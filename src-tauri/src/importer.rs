//! Importa jogos de `<unidade>:\sofa\dump` para a biblioteca local.
//! Só essa pasta é lida, nada no disco externo é alterado e nenhum arquivo importado é executado.

use crate::library::Store;
use crate::providers::emulator::{clean_title, parse_sfo, ps3_layout};
use crate::providers::manual::ManualGame;
use crate::session::path_in_dir;
use crate::{art, input, AppState};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetLogicalDrives};

const PART_SUFFIX: &str = ".sofa-part";
/// Pasta de arquivos que os jogos pedem depois, como ISOs e assets de decomps.
const ISO_CATEGORY: &str = "iso";
const DISC_IMAGE_EXTS: &[&str] = &[
    "iso", "rvz", "gcm", "gcz", "wbfs", "ciso", "wia", "wad", "dol", "bin", "cue", "img", "chd", "cso", "nds", "3ds", "cia", "n64",
    "z64", "v64", "nsp", "xci", "wua", "wud", "wux", "xiso",
];
const MAX_MAIN_FILES: usize = 8;
/// Pastas de jogos são montadas aqui e só entram na biblioteca quando completas.
const STAGING_DIR: &str = ".sofa-staging";
const CHUNK: usize = 1024 * 1024;
const DETECT_EVERY: Duration = Duration::from_secs(3);
const PROGRESS_EVERY: Duration = Duration::from_millis(200);
/// Nomes de instaladores e programas auxiliares, que não devem ser o executável do jogo.
const NON_GAME_EXES: &[&str] = &[
    "setup", "install", "unins", "crash", "redist", "vc_", "dxsetup", "dxwebsetup", "directx", "dotnet", "prereq",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    New,
    NeedsExe,
    Done,
    Conflict,
    Blocked,
}

#[derive(Clone, Debug)]
struct FileEntry {
    rel: PathBuf,
    size: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImportItem {
    pub id: String,
    pub title: String,
    pub category: String,
    pub status: Status,
    pub problem: Option<String>,
    pub size: u64,
    pub pending_bytes: u64,
    pub exe: Option<String>,
    pub exe_options: Vec<String>,
    #[serde(skip)]
    args: Vec<String>,
    #[serde(skip)]
    cwd: Option<String>,
    #[serde(skip)]
    src_root: PathBuf,
    #[serde(skip)]
    dest_root: PathBuf,
    #[serde(skip)]
    staging: Option<PathBuf>,
    #[serde(skip)]
    files: Vec<FileEntry>,
    #[serde(skip)]
    dirs: Vec<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExePick {
    Chosen { exe: String, title: Option<String>, args: Vec<String>, cwd: Option<String> },
    Choose(Vec<String>),
    OnlyInstallers,
    NoExe,
    Invalid(String),
}

#[derive(Deserialize)]
struct SofaJson {
    title: Option<String>,
    exe: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
}

/// Aceita só caminhos relativos que não saem da pasta do jogo.
fn safe_rel(text: &str) -> Option<PathBuf> {
    let path = PathBuf::from(text.replace('/', "\\"));
    let safe = path.components().next().is_some() && path.components().all(|c| matches!(c, Component::Normal(_)));
    safe.then_some(path)
}

fn is_exe(path: &str) -> bool {
    path.to_lowercase().ends_with(".exe")
}

fn is_non_game_exe(path: &str) -> bool {
    let lower = path.to_lowercase();
    let name = lower.rsplit('\\').next().unwrap_or(&lower);
    NON_GAME_EXES.iter().any(|word| name.contains(word))
        || lower.split('\\').any(|part| matches!(part, "redist" | "_commonredist" | "__installer" | "directx"))
}

/// Decide qual `.exe` iniciar, recebendo os arquivos relativos à pasta do jogo.
pub fn pick_exe(files: &[String], sofa_json: Option<&str>) -> ExePick {
    if let Some(text) = sofa_json {
        let parsed: SofaJson = match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
            Ok(parsed) => parsed,
            Err(e) => return ExePick::Invalid(format!("sofa.json inválido: {e}")),
        };
        let Some(exe_path) = safe_rel(&parsed.exe) else {
            return ExePick::Invalid("sofa.json aponta para fora da pasta do jogo.".into());
        };
        let exe = exe_path.to_string_lossy().into_owned();
        if !is_exe(&exe) || !files.iter().any(|f| f.eq_ignore_ascii_case(&exe)) {
            return ExePick::Invalid(format!("sofa.json indica {exe}, que não existe nesta pasta."));
        }
        let cwd = match parsed.cwd.as_deref().map(safe_rel) {
            Some(None) => return ExePick::Invalid("O cwd do sofa.json aponta para fora da pasta do jogo.".into()),
            Some(Some(path)) => Some(path.to_string_lossy().into_owned()),
            None => None,
        };
        return ExePick::Chosen { exe, title: parsed.title, args: parsed.args, cwd };
    }
    let exes: Vec<&String> = files.iter().filter(|f| is_exe(f)).collect();
    if exes.is_empty() {
        return ExePick::NoExe;
    }
    let mut games: Vec<String> = exes.into_iter().filter(|f| !is_non_game_exe(f)).cloned().collect();
    games.sort();
    match games.len() {
        0 => ExePick::OnlyInstallers,
        1 => ExePick::Chosen { exe: games.remove(0), title: None, args: vec![], cwd: None },
        _ => ExePick::Choose(games),
    }
}

/// Não segue atalhos nem links: nada fora da pasta de origem é lido.
fn walk_tree(root: &Path, rel: &Path, files: &mut Vec<FileEntry>, dirs: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root.join(rel))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let entry_rel = rel.join(entry.file_name());
        if kind.is_dir() {
            dirs.push(entry_rel.clone());
            walk_tree(root, &entry_rel, files, dirs)?;
        } else if kind.is_file() {
            let lower = entry.file_name().to_string_lossy().to_lowercase();
            if !lower.ends_with(".lnk") && !lower.ends_with(PART_SUFFIX) {
                files.push(FileEntry { rel: entry_rel, size: entry.metadata()?.len() });
            }
        }
    }
    Ok(())
}

fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).map(|m| m.is_dir() && !m.file_type().is_symlink()).unwrap_or(false)
}

/// A única pasta que o importador lê em cada disco.
pub fn dump_dir(root: &Path) -> Option<PathBuf> {
    let sofa = root.join("sofa");
    let dump = sofa.join("dump");
    (real_dir(&sofa) && real_dir(&dump)).then_some(dump)
}

fn blocked(category: &str, name: &str, title: String, problem: String) -> ImportItem {
    ImportItem {
        id: format!("{category}/{name}").to_lowercase(),
        title,
        category: category.to_string(),
        status: Status::Blocked,
        problem: Some(problem),
        size: 0,
        pending_bytes: 0,
        exe: None,
        exe_options: vec![],
        args: vec![],
        cwd: None,
        src_root: PathBuf::new(),
        dest_root: PathBuf::new(),
        staging: None,
        files: vec![],
        dirs: vec![],
    }
}

fn entries_of(dir: &Path) -> Vec<(String, PathBuf, std::fs::FileType)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    entries
        .flatten()
        .filter_map(|e| {
            let kind = e.file_type().ok()?;
            (!kind.is_symlink()).then(|| (e.file_name().to_string_lossy().into_owned(), e.path(), kind))
        })
        .collect()
}

fn stem(name: &str) -> String {
    Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_string())
}

fn has_ext(name: &Path, exts: &[String]) -> bool {
    let ext = name.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    exts.contains(&ext)
}

/// Monta um item de pasta inteira, lendo seus arquivos.
fn dir_item(category: &str, name: &str, title: String, src: &Path, library: &Path) -> ImportItem {
    let mut files = vec![];
    let mut dirs = vec![];
    if let Err(e) = walk_tree(src, Path::new(""), &mut files, &mut dirs) {
        return blocked(category, name, title, format!("Não foi possível ler a pasta: {e}"));
    }
    let mut item = blocked(category, name, title, String::new());
    item.problem = None;
    item.status = Status::New;
    item.src_root = src.to_path_buf();
    item.dest_root = library.join(category).join(name);
    item.staging = Some(library.join(STAGING_DIR).join(category).join(name));
    item.size = files.iter().map(|f| f.size).sum();
    item.files = files;
    item.dirs = dirs;
    item
}

fn file_item(category: &str, name: &str, src: &Path, library: &Path) -> ImportItem {
    let mut item = blocked(category, name, clean_title(&stem(name)), String::new());
    item.problem = None;
    item.status = Status::New;
    item.src_root = src.parent().unwrap_or(Path::new("")).to_path_buf();
    item.dest_root = library.join(category);
    let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
    item.size = size;
    item.files = vec![FileEntry { rel: PathBuf::from(name), size }];
    item
}

impl ImportItem {
    /// Onde os arquivos estão sendo gravados: na pasta final, se já existir, ou na provisória.
    fn base(&self) -> &Path {
        match &self.staging {
            Some(staging) if !self.dest_root.exists() => staging,
            _ => &self.dest_root,
        }
    }

    fn writes_to_final(&self) -> bool {
        self.base() == self.dest_root
    }
}

fn is_registered(item: &ImportItem, store: &Store) -> bool {
    if item.category == ISO_CATEGORY || item.category == art::COVERS_DIR {
        // Não há cadastro: estar copiado já basta.
        true
    } else if item.category == "pc" {
        let dest = item.dest_root.to_string_lossy();
        store.manual.iter().any(|m| path_in_dir(&m.path, &dest))
    } else {
        let root = item.dest_root.parent().filter(|_| item.staging.is_some()).unwrap_or(&item.dest_root).to_string_lossy().into_owned();
        store.emulators.iter().any(|e| e.id == item.category && e.rom_dirs.iter().any(|d| d.eq_ignore_ascii_case(&root)))
    }
}

/// Compara com o que já existe e define o estado final do item.
fn finish_item(mut item: ImportItem, store: &Store) -> ImportItem {
    if item.status == Status::Blocked {
        return item;
    }
    let to_final = item.writes_to_final();
    let base = item.base().to_path_buf();
    let mut pending = 0;
    let mut conflict = None;
    for file in &item.files {
        match std::fs::metadata(base.join(&file.rel)) {
            Ok(meta) if meta.len() == file.size => {}
            Ok(_) if to_final => {
                conflict = Some(file.rel.to_string_lossy().into_owned());
                break;
            }
            _ => pending += file.size,
        }
    }
    item.pending_bytes = pending;
    if let Some(name) = conflict {
        item.status = Status::Conflict;
        item.problem = Some(format!("{name} já existe na biblioteca com outro tamanho. Nada foi sobrescrito."));
    } else if pending == 0 && to_final && is_registered(&item, store) {
        item.status = Status::Done;
    } else if !item.exe_options.is_empty() {
        item.status = Status::NeedsExe;
    }
    item
}

fn plan_pc(dir: &Path, library: &Path) -> Vec<ImportItem> {
    let mut items = vec![];
    for (name, path, kind) in entries_of(dir) {
        if !kind.is_dir() {
            if is_exe(&name) {
                items.push(blocked(
                    "pc",
                    &name,
                    clean_title(&stem(&name)),
                    format!("{name} está solto em pc\\. Coloque-o em uma pasta com todos os arquivos do jogo."),
                ));
            }
            continue;
        }
        let mut item = dir_item("pc", &name, clean_title(&name), &path, library);
        if item.status == Status::Blocked {
            items.push(item);
            continue;
        }
        let rels: Vec<String> = item.files.iter().map(|f| f.rel.to_string_lossy().into_owned()).collect();
        let json = std::fs::read_to_string(path.join("sofa.json")).ok();
        match pick_exe(&rels, json.as_deref()) {
            ExePick::Chosen { exe, title, args, cwd } => {
                if let Some(title) = title {
                    item.title = title;
                }
                item.exe = Some(exe);
                item.args = args;
                item.cwd = cwd;
            }
            ExePick::Choose(options) => item.exe_options = options,
            ExePick::OnlyInstallers => {
                item.status = Status::Blocked;
                item.problem = Some("Só encontrei instaladores. O jogo precisa estar pronto para rodar, e instaladores nunca são executados.".into());
            }
            ExePick::NoExe => {
                item.status = Status::Blocked;
                item.problem = Some("Nenhum .exe encontrado nesta pasta.".into());
            }
            ExePick::Invalid(message) => {
                item.status = Status::Blocked;
                item.problem = Some(message);
            }
        }
        items.push(item);
    }
    items
}

fn plan_emulator(category: &str, exts: &[String], kind_rpcs3: bool, dir: &Path, library: &Path) -> Vec<ImportItem> {
    let exts: Vec<String> = exts.iter().map(|e| e.trim_start_matches('.').to_lowercase()).collect();
    let mut items = vec![];
    for (name, path, kind) in entries_of(dir) {
        if kind_rpcs3 {
            if !kind.is_dir() {
                continue;
            }
            match ps3_layout(&path) {
                Some((_, sfo)) => {
                    let title = std::fs::read(sfo).ok().and_then(|d| parse_sfo(&d, "TITLE")).unwrap_or_else(|| clean_title(&name));
                    items.push(dir_item(category, &name, title, &path, library));
                }
                None => items.push(blocked(category, &name, clean_title(&name), "Não parece um jogo de PS3: falta USRDIR\\EBOOT.BIN.".into())),
            }
        } else if kind.is_dir() {
            let item = dir_item(category, &name, clean_title(&name), &path, library);
            if item.status == Status::Blocked || item.files.iter().any(|f| has_ext(&f.rel, &exts)) {
                items.push(item);
            } else {
                items.push(blocked(category, &name, clean_title(&name), "Nenhum arquivo de jogo com as extensões deste emulador.".into()));
            }
        } else if has_ext(Path::new(&name), &exts) {
            items.push(file_item(category, &name, &path, library));
        }
    }
    items
}

/// `iso\{jogo}`: arquivos que um jogo pede depois (ISO ou pastas de assets). Vão inteiros,
/// sem filtro de extensão, e nunca viram jogos, mesmo que contenham um `.exe`.
fn plan_iso(dir: &Path, library: &Path) -> Vec<ImportItem> {
    let mut items = vec![];
    for (name, path, kind) in entries_of(dir) {
        if kind.is_dir() {
            let item = dir_item(ISO_CATEGORY, &name, name.clone(), &path, library);
            if item.status != Status::Blocked && item.files.is_empty() {
                items.push(blocked(ISO_CATEGORY, &name, name.clone(), "Esta pasta está vazia.".into()));
            } else {
                items.push(item);
            }
        } else {
            items.push(file_item(ISO_CATEGORY, &name, &path, library));
        }
    }
    items
}

/// `capas`: uma imagem solta com o nome do jogo ou uma pasta com o nome dele.
/// Outros arquivos soltos são ignorados, como acontece com as pastas de emulador.
fn plan_covers(dir: &Path, library: &Path) -> Vec<ImportItem> {
    let mut items = vec![];
    for (name, path, kind) in entries_of(dir) {
        if kind.is_dir() {
            if art::best_cover_in(&path).is_some() {
                items.push(dir_item(art::COVERS_DIR, &name, name.clone(), &path, library));
            } else {
                items.push(blocked(art::COVERS_DIR, &name, name.clone(), "Nenhuma imagem .jpg, .png ou .webp nesta pasta.".into()));
            }
        } else if art::is_cover_image(&path) {
            let mut item = file_item(art::COVERS_DIR, &name, &path, library);
            item.title = stem(&name);
            items.push(item);
        }
    }
    items
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct GameFiles {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub file_count: usize,
    /// Arquivos que parecem a imagem do jogo, para copiar o caminho direto.
    pub main_files: Vec<String>,
}

fn is_disc_image(path: &Path) -> bool {
    let ext = path.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    DISC_IMAGE_EXTS.contains(&ext.as_str())
}

/// Lista o que está em `<biblioteca>\iso`. Só lê essa pasta.
pub fn list_game_files(library: &Path) -> Vec<GameFiles> {
    let dir = library.join(ISO_CATEGORY);
    let mut out = vec![];
    for (name, path, kind) in entries_of(&dir) {
        if name.ends_with(PART_SUFFIX) {
            continue;
        }
        let shown = path.to_string_lossy().into_owned();
        if kind.is_dir() {
            let (mut files, mut dirs) = (vec![], vec![]);
            if walk_tree(&path, Path::new(""), &mut files, &mut dirs).is_err() {
                continue;
            }
            let mut main_files: Vec<String> = files
                .iter()
                .filter(|f| is_disc_image(&f.rel))
                .map(|f| path.join(&f.rel).to_string_lossy().into_owned())
                .collect();
            main_files.sort();
            main_files.truncate(MAX_MAIN_FILES);
            out.push(GameFiles {
                name,
                path: shown,
                size: files.iter().map(|f| f.size).sum(),
                file_count: files.len(),
                main_files,
            });
        } else {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push(GameFiles { name: stem(&name), path: shown.clone(), size, file_count: 1, main_files: vec![shown] });
        }
    }
    out.sort_by_key(|f| f.name.to_lowercase());
    out
}

/// O arquivo de `<biblioteca>\iso` com o mesmo nome do jogo: a imagem, se houver só uma,
/// ou a pasta, se só houver assets. Com várias imagens não dá para adivinhar, então devolve nada.
pub fn auto_file_for(title: &str, library: &Path) -> Option<String> {
    let wanted = title.trim().to_lowercase();
    let entry = list_game_files(library)
        .into_iter()
        .find(|f| f.name.trim().to_lowercase() == wanted || clean_title(&f.name).to_lowercase() == wanted)?;
    match entry.main_files.len() {
        0 => Some(entry.path),
        1 => entry.main_files.into_iter().next(),
        _ => None,
    }
}

/// Lista o que há no dump e o estado de cada item. Só lê; não altera nada.
pub fn plan_import(dump: &Path, store: &Store, library: &Path) -> Vec<ImportItem> {
    let mut items = vec![];
    for (name, path, kind) in entries_of(dump) {
        if !kind.is_dir() {
            continue;
        }
        let lower = name.to_lowercase();
        if lower == "pc" {
            items.extend(plan_pc(&path, library));
        } else if lower == ISO_CATEGORY {
            items.extend(plan_iso(&path, library));
        } else if lower == art::COVERS_DIR || lower == "capa" {
            items.extend(plan_covers(&path, library));
        } else if let Some(emu) = store.emulators.iter().find(|e| e.id.eq_ignore_ascii_case(&name)) {
            items.extend(plan_emulator(&emu.id, &emu.exts, emu.kind == "rpcs3", &path, library));
        } else {
            for (child, _, _) in entries_of(&path) {
                items.push(blocked(
                    &lower,
                    &child,
                    clean_title(&stem(&child)),
                    format!("Precisa do emulador {name}. Cadastre-o em Menu → Emuladores."),
                ));
            }
        }
    }
    let mut items: Vec<ImportItem> = items.into_iter().map(|i| finish_item(i, store)).collect();
    items.sort_by_key(|i| i.title.to_lowercase());
    items
}

#[derive(Debug, PartialEq, Eq)]
pub enum CopyError {
    Cancelled,
    Failed(String),
}

fn failed(context: &Path, e: impl std::fmt::Display) -> CopyError {
    CopyError::Failed(format!("{}: {e}", context.display()))
}

/// Grava em um arquivo provisório e renomeia, para nunca deixar um arquivo final incompleto.
fn copy_file(src: &Path, dest: &Path, size: u64, cancel: &AtomicBool, on_bytes: &mut dyn FnMut(u64)) -> Result<(), CopyError> {
    let mut part_name = dest.file_name().unwrap_or_default().to_os_string();
    part_name.push(PART_SUFFIX);
    let part = dest.with_file_name(part_name);
    let result = (|| {
        let mut reader = std::fs::File::open(src).map_err(|e| failed(src, e))?;
        let mut writer = std::fs::File::create(&part).map_err(|e| failed(&part, e))?;
        let mut buffer = vec![0u8; CHUNK];
        let mut written = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(CopyError::Cancelled);
            }
            let read = reader.read(&mut buffer).map_err(|e| failed(src, e))?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read]).map_err(|e| failed(&part, e))?;
            written += read as u64;
            on_bytes(read as u64);
        }
        writer.sync_all().map_err(|e| failed(&part, e))?;
        if written != size {
            return Err(failed(src, "o tamanho mudou durante a cópia"));
        }
        Ok(())
    })();
    match result {
        Ok(()) => std::fs::rename(&part, dest).map_err(|e| failed(dest, e)),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// Apaga restos de importações interrompidas.
fn remove_stale_parts(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            remove_stale_parts(&entry.path());
        } else if entry.file_name().to_string_lossy().ends_with(PART_SUFFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Copia um item. A origem é só lida; arquivos finais existentes nunca são sobrescritos.
fn copy_item(item: &ImportItem, cancel: &AtomicBool, on_bytes: &mut dyn FnMut(u64)) -> Result<(), CopyError> {
    let to_final = item.writes_to_final();
    let base = item.base().to_path_buf();
    std::fs::create_dir_all(&base).map_err(|e| failed(&base, e))?;
    if item.staging.is_some() {
        remove_stale_parts(&base);
    } else if let Some(file) = item.files.first() {
        let mut part = file.rel.as_os_str().to_os_string();
        part.push(PART_SUFFIX);
        let _ = std::fs::remove_file(base.join(part));
    }
    for dir in &item.dirs {
        let path = base.join(dir);
        std::fs::create_dir_all(&path).map_err(|e| failed(&path, e))?;
    }
    for file in &item.files {
        let dest = base.join(&file.rel);
        if let Ok(meta) = std::fs::metadata(&dest) {
            if meta.len() == file.size {
                continue;
            }
            if to_final {
                return Err(CopyError::Failed(format!("{} já existe com outro tamanho.", dest.display())));
            }
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| failed(parent, e))?;
        }
        copy_file(&item.src_root.join(&file.rel), &dest, file.size, cancel, on_bytes)?;
    }
    // Só agora a pasta entra no lugar final, onde a biblioteca consegue vê-la.
    if !to_final {
        if let Some(parent) = item.dest_root.parent() {
            std::fs::create_dir_all(parent).map_err(|e| failed(parent, e))?;
        }
        std::fs::rename(&base, &item.dest_root).map_err(|e| failed(&item.dest_root, e))?;
    }
    Ok(())
}

/// Coloca o jogo copiado na biblioteca. O `.exe` cadastrado é o da cópia, nunca o do disco externo.
fn register(store: &mut Store, item: &ImportItem, chosen_exe: Option<&str>) -> Result<(), String> {
    if item.category == ISO_CATEGORY || item.category == art::COVERS_DIR {
        return Ok(());
    }
    if item.category == "pc" {
        let exe = chosen_exe.or(item.exe.as_deref()).ok_or("Nenhum executável escolhido.")?;
        let path = item.dest_root.join(exe).to_string_lossy().into_owned();
        let cwd = item.cwd.as_ref().map(|c| item.dest_root.join(c).to_string_lossy().into_owned());
        if !store.manual.iter().any(|m| m.path.eq_ignore_ascii_case(&path)) {
            store.manual.push(ManualGame {
                id: art::file_stem(&item.id),
                title: item.title.clone(),
                path,
                args: item.args.clone(),
                cwd,
            });
        }
    } else {
        let root = item.dest_root.parent().filter(|_| item.staging.is_some()).unwrap_or(&item.dest_root).to_string_lossy().into_owned();
        let emu = store.emulators.iter_mut().find(|e| e.id == item.category).ok_or("Emulador não encontrado.")?;
        if !emu.rom_dirs.iter().any(|d| d.eq_ignore_ascii_case(&root)) {
            emu.rom_dirs.push(root);
        }
    }
    Ok(())
}

fn free_bytes(path: &Path) -> Option<u64> {
    let mut existing = path;
    while !existing.exists() {
        existing = existing.parent()?;
    }
    let mut available = 0u64;
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(existing.as_os_str()), Some(&mut available), None, None) }.ok()?;
    Some(available)
}

fn drive_roots() -> Vec<PathBuf> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8).filter(|i| mask & (1 << i) != 0).map(|i| PathBuf::from(format!("{}:\\", (b'A' + i) as char))).collect()
}

fn counts_as_new(item: &ImportItem) -> bool {
    matches!(item.status, Status::New | Status::NeedsExe)
}

#[derive(Serialize, Clone)]
pub struct DrivePlan {
    pub drive: String,
    pub items: Vec<ImportItem>,
    pub free_bytes: Option<u64>,
    pub library_dir: String,
}

fn plan_for(state: &AppState, root: &Path) -> Option<DrivePlan> {
    let dump = dump_dir(root)?;
    // Uma cópia, para ler um disco lento sem travar o restante do launcher.
    let store = state.store.lock().unwrap().clone();
    let library = PathBuf::from(store.library_dir());
    let items = plan_import(&dump, &store, &library);
    Some(DrivePlan {
        drive: root.to_string_lossy().into_owned(),
        items,
        free_bytes: free_bytes(&library),
        library_dir: library.to_string_lossy().into_owned(),
    })
}

pub fn scan_drives(state: &AppState) -> Vec<DrivePlan> {
    drive_roots().iter().filter_map(|root| plan_for(state, root)).collect()
}

#[derive(Serialize, Clone, Default)]
struct Progress {
    done_bytes: u64,
    total_bytes: u64,
    bytes_per_sec: u64,
    current: String,
    item_index: usize,
    item_count: usize,
    finished: bool,
    cancelled: bool,
    imported: Vec<String>,
    errors: Vec<String>,
}

pub fn cancel(state: &AppState) {
    state.import_cancel.store(true, Ordering::Relaxed);
}

/// Revalida tudo no momento da importação, sem confiar no que a interface mostrou antes.
pub fn start(
    app: AppHandle,
    state: Arc<AppState>,
    drive: String,
    ids: Vec<String>,
    choices: HashMap<String, String>,
) -> Result<(), String> {
    if state.session.lock().unwrap().is_some() {
        return Err("Feche o jogo antes de importar.".into());
    }
    let plan = plan_for(&state, Path::new(&drive)).ok_or("Não encontrei a pasta sofa\\dump neste disco.")?;
    let wanted: HashSet<&String> = ids.iter().collect();
    let items: Vec<ImportItem> = plan.items.into_iter().filter(|i| wanted.contains(&i.id) && counts_as_new(i)).collect();
    if items.is_empty() {
        return Err("Não há nada novo para importar.".into());
    }
    for item in &items {
        if item.status == Status::NeedsExe {
            let choice = choices.get(&item.id).ok_or_else(|| format!("Escolha o executável de {}.", item.title))?;
            if !item.exe_options.contains(choice) {
                return Err(format!("Executável inválido para {}.", item.title));
            }
        }
    }
    let total: u64 = items.iter().map(|i| i.pending_bytes).sum();
    if let Some(free) = plan.free_bytes {
        if total > free {
            return Err(format!(
                "Espaço insuficiente em {}: faltam {} MB.",
                plan.library_dir,
                (total - free) / (1024 * 1024)
            ));
        }
    }
    if state.importing.swap(true, Ordering::SeqCst) {
        return Err("Já existe uma importação em andamento.".into());
    }
    state.import_cancel.store(false, Ordering::Relaxed);

    std::thread::spawn(move || {
        let started = Instant::now();
        let mut progress = Progress { total_bytes: total, item_count: items.len(), ..Default::default() };
        let mut last_emit = Instant::now() - PROGRESS_EVERY;
        for (index, item) in items.iter().enumerate() {
            progress.item_index = index + 1;
            progress.current = item.title.clone();
            let _ = app.emit("import-progress", progress.clone());
            let result = {
                let progress = &mut progress;
                let app = &app;
                let last_emit = &mut last_emit;
                copy_item(item, &state.import_cancel, &mut |bytes| {
                    progress.done_bytes += bytes;
                    if last_emit.elapsed() >= PROGRESS_EVERY {
                        *last_emit = Instant::now();
                        let elapsed = started.elapsed().as_secs_f64().max(0.001);
                        progress.bytes_per_sec = (progress.done_bytes as f64 / elapsed) as u64;
                        let _ = app.emit("import-progress", progress.clone());
                    }
                })
            };
            match result {
                Ok(()) => {
                    let mut store = state.store.lock().unwrap();
                    let outcome = register(&mut store, item, choices.get(&item.id).map(String::as_str))
                        .and_then(|()| store.save().map_err(|e| e.to_string()));
                    match outcome {
                        Ok(()) => progress.imported.push(item.title.clone()),
                        Err(e) => progress.errors.push(format!("{}: {e}", item.title)),
                    }
                }
                Err(CopyError::Cancelled) => {
                    progress.cancelled = true;
                    break;
                }
                Err(CopyError::Failed(e)) => progress.errors.push(format!("{}: {e}", item.title)),
            }
        }
        crate::commands::rescan(&state);
        state.importing.store(false, Ordering::SeqCst);
        progress.finished = true;
        let _ = app.emit("library-changed", ());
        let _ = app.emit("import-progress", progress);
        if crate::credentials::load().is_some() {
            art::fetch_missing(app.clone(), state.clone());
        }
    });
    Ok(())
}

#[derive(Serialize, Clone)]
struct Found {
    drive: String,
    count: usize,
}

/// Avisa uma vez por conexão quando um disco com jogos novos aparece. Nunca durante um jogo.
pub fn spawn_detector(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut notified: HashSet<PathBuf> = HashSet::new();
        loop {
            std::thread::sleep(DETECT_EVERY);
            let present: Vec<PathBuf> = drive_roots().into_iter().filter(|r| dump_dir(r).is_some()).collect();
            notified.retain(|root| present.contains(root));
            let idle = input::mode() == input::Mode::Launcher
                && state.session.lock().unwrap().is_none()
                && !state.importing.load(Ordering::SeqCst);
            if !idle {
                continue;
            }
            for root in present {
                if !notified.insert(root.clone()) {
                    continue;
                }
                let Some(plan) = plan_for(&state, &root) else { continue };
                let count = plan.items.iter().filter(|i| counts_as_new(i)).count();
                if count > 0 {
                    let _ = app.emit_to("main", "import-found", Found { drive: plan.drive, count });
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::emulator::preset;

    struct Temp(PathBuf);
    impl Temp {
        fn new(tag: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("sofa-import-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }
        fn write(&self, rel: &str, data: &[u8]) {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, data).unwrap();
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store_with_cemu() -> Store {
        let mut store = Store::default();
        store.emulators.push(preset("cemu", r"C:\nao-existe\Cemu.exe"));
        store
    }

    fn item<'a>(items: &'a [ImportItem], title: &str) -> &'a ImportItem {
        items.iter().find(|i| i.title == title).unwrap_or_else(|| panic!("sem item {title}: {items:?}"))
    }

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    /// Copia e registra, como a importação real faz.
    fn import(item: &ImportItem, store: &mut Store, choice: Option<&str>) {
        copy_item(item, &no_cancel(), &mut |_| {}).unwrap();
        register(store, item, choice).unwrap();
    }

    fn strs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exe_unico_ignorando_auxiliares_e_instaladores() {
        let files = strs(&["dusklight.exe", "crashpad_handler.exe", "unins000.exe", "res\\a.dat", "redist\\vc_x64.exe"]);
        assert_eq!(
            pick_exe(&files, None),
            ExePick::Chosen { exe: "dusklight.exe".into(), title: None, args: vec![], cwd: None }
        );
    }

    #[test]
    fn exe_em_subpasta_e_encontrado() {
        let files = strs(&["bin\\x64\\jogo.exe", "dados\\a.pak"]);
        assert!(matches!(pick_exe(&files, None), ExePick::Chosen { exe, .. } if exe == "bin\\x64\\jogo.exe"));
    }

    #[test]
    fn varios_exes_pedem_escolha() {
        let files = strs(&["a.exe", "b.exe", "setup.exe"]);
        assert_eq!(pick_exe(&files, None), ExePick::Choose(strs(&["a.exe", "b.exe"])));
    }

    #[test]
    fn so_instalador_ou_sem_exe_nao_importa() {
        assert_eq!(pick_exe(&strs(&["setup.exe", "a.bin"]), None), ExePick::OnlyInstallers);
        assert_eq!(pick_exe(&strs(&["a.bin"]), None), ExePick::NoExe);
    }

    #[test]
    fn sofa_json_define_exe_titulo_e_cwd() {
        let files = strs(&["bin\\jogo.exe", "outro.exe"]);
        let json = r#"{"title":"Meu Jogo","exe":"bin/jogo.exe","args":["-w"],"cwd":"bin"}"#;
        assert_eq!(
            pick_exe(&files, Some(json)),
            ExePick::Chosen { exe: "bin\\jogo.exe".into(), title: Some("Meu Jogo".into()), args: strs(&["-w"]), cwd: Some("bin".into()) }
        );
    }

    #[test]
    fn sofa_json_nao_sai_da_pasta_do_jogo() {
        let files = strs(&["jogo.exe"]);
        for json in [
            r#"{"exe":"..\\..\\Windows\\notepad.exe"}"#,
            r#"{"exe":"C:\\Windows\\notepad.exe"}"#,
            r#"{"exe":"sub/../../fora.exe"}"#,
            r#"{"exe":"jogo.exe","cwd":"..\\.."}"#,
            r#"{"exe":"inexistente.exe"}"#,
            r#"{"exe":"jogo.txt"}"#,
            "nao e json",
        ] {
            assert!(matches!(pick_exe(&files, Some(json)), ExePick::Invalid(_)), "{json}");
        }
    }

    #[test]
    fn so_le_a_pasta_sofa_dump() {
        let disk = Temp::new("disco");
        assert_eq!(dump_dir(&disk.0), None);
        disk.write("sofa/outra/a.exe", b"x");
        assert_eq!(dump_dir(&disk.0), None);
        std::fs::create_dir_all(disk.0.join("sofa/dump")).unwrap();
        assert_eq!(dump_dir(&disk.0), Some(disk.0.join("sofa").join("dump")));
    }

    #[test]
    fn classifica_por_pasta() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("cemu/Mario Kart (USA).wua", b"rom");
        dump.write("cemu/leia.txt", b"x");
        dump.write("cemu/Zelda/code/game.rpx", b"rpx");
        dump.write("cemu/SoFotos/a.png", b"x");
        dump.write("dolphin/Wind Waker.rvz", b"rom");
        dump.write("pc/Dusklight/dusklight.exe", b"exe");
        dump.write("pc/solto.exe", b"exe");
        dump.write("pc/Instalar/setup.exe", b"exe");
        let items = plan_import(&dump.0, &store_with_cemu(), &library.0);

        assert_eq!(item(&items, "Mario Kart").status, Status::New);
        assert_eq!(item(&items, "Zelda").status, Status::New);
        assert_eq!(item(&items, "SoFotos").status, Status::Blocked);
        assert!(!items.iter().any(|i| i.title.contains("leia")));
        let dolphin = item(&items, "Wind Waker");
        assert_eq!(dolphin.status, Status::Blocked);
        assert!(dolphin.problem.as_ref().unwrap().contains("dolphin"));
        assert_eq!(item(&items, "Dusklight").exe.as_deref(), Some("dusklight.exe"));
        assert!(item(&items, "solto").problem.as_ref().unwrap().contains("solto"));
        assert_eq!(item(&items, "Instalar").status, Status::Blocked);
    }

    #[test]
    fn jogo_de_pc_vai_inteiro_e_aponta_para_a_copia() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("pc/Jogo/bin/jogo.exe", b"exe");
        dump.write("pc/Jogo/bin/lib.dll", b"dll");
        dump.write("pc/Jogo/dados/mapas/a.pak", b"pak");
        std::fs::create_dir_all(dump.0.join("pc/Jogo/saves")).unwrap();
        let mut store = Store::default();
        let items = plan_import(&dump.0, &store, &library.0);
        import(item(&items, "Jogo"), &mut store, None);

        let dest = library.0.join("pc").join("Jogo");
        for rel in ["bin/jogo.exe", "bin/lib.dll", "dados/mapas/a.pak"] {
            assert!(dest.join(rel).is_file(), "{rel}");
        }
        assert!(dest.join("saves").is_dir(), "pastas vazias também são copiadas");
        assert_eq!(store.manual.len(), 1);
        assert_eq!(store.manual[0].path, dest.join("bin\\jogo.exe").to_string_lossy());
        assert!(!store.manual[0].path.starts_with(&*dump.0.to_string_lossy()));
        assert!(!library.0.join(STAGING_DIR).join("pc").join("Jogo").exists());
    }

    #[test]
    fn escolha_de_exe_e_respeitada() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("pc/Jogo/a.exe", b"a");
        dump.write("pc/Jogo/b.exe", b"b");
        let mut store = Store::default();
        let items = plan_import(&dump.0, &store, &library.0);
        let game = item(&items, "Jogo");
        assert_eq!(game.status, Status::NeedsExe);
        import(game, &mut store, Some("b.exe"));
        assert!(store.manual[0].path.ends_with("b.exe"));
    }

    #[test]
    fn rom_entra_nas_pastas_do_emulador_e_e_encontrada_pela_varredura() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("cemu/Mario.wua", b"rom");
        dump.write("cemu/Zelda/code/game.rpx", b"rpx");
        let mut store = store_with_cemu();
        let items = plan_import(&dump.0, &store, &library.0);
        import(item(&items, "Mario"), &mut store, None);
        import(item(&items, "Zelda"), &mut store, None);

        let root = library.0.join("cemu").to_string_lossy().into_owned();
        assert_eq!(store.emulators[0].rom_dirs, vec![root]);
        let mut titles: Vec<String> = crate::providers::emulator::scan(&store.emulators[0]).into_iter().map(|g| g.title).collect();
        titles.sort();
        assert_eq!(titles, vec!["Mario", "Zelda"]);
    }

    #[test]
    fn segunda_conexao_nao_tem_nada_novo() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("cemu/Mario.wua", b"rom");
        dump.write("pc/Jogo/jogo.exe", b"exe");
        let mut store = store_with_cemu();
        for i in plan_import(&dump.0, &store, &library.0) {
            import(&i, &mut store, None);
        }
        let again = plan_import(&dump.0, &store, &library.0);
        assert!(again.iter().all(|i| i.status == Status::Done), "{again:?}");
    }

    #[test]
    fn copia_completa_mas_sem_cadastro_ainda_conta_como_nova() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("pc/Jogo/jogo.exe", b"exe");
        let store = Store::default();
        let items = plan_import(&dump.0, &store, &library.0);
        copy_item(item(&items, "Jogo"), &no_cancel(), &mut |_| {}).unwrap();
        let again = plan_import(&dump.0, &store, &library.0);
        assert_eq!(item(&again, "Jogo").status, Status::New);
        assert_eq!(item(&again, "Jogo").pending_bytes, 0);
    }

    #[test]
    fn conflito_nao_sobrescreve_nada() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("cemu/Mario.wua", b"novo tamanho");
        library.write("cemu/Mario.wua", b"meu");
        let store = store_with_cemu();
        let items = plan_import(&dump.0, &store, &library.0);
        let mario = item(&items, "Mario");
        assert_eq!(mario.status, Status::Conflict);
        assert!(copy_item(mario, &no_cancel(), &mut |_| {}).is_err());
        assert_eq!(std::fs::read(library.0.join("cemu").join("Mario.wua")).unwrap(), b"meu");
    }

    #[test]
    fn cancelar_nao_deixa_arquivo_final_incompleto() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("cemu/Mario.wua", &vec![7u8; CHUNK * 3]);
        let store = store_with_cemu();
        let items = plan_import(&dump.0, &store, &library.0);
        let cancel = no_cancel();
        let result = copy_item(item(&items, "Mario"), &cancel, &mut |_| cancel.store(true, Ordering::Relaxed));
        assert_eq!(result, Err(CopyError::Cancelled));
        let dest = library.0.join("cemu");
        assert!(!dest.join("Mario.wua").exists());
        assert!(!dest.join(format!("Mario.wua{PART_SUFFIX}")).exists());
    }

    #[test]
    fn pasta_cancelada_nao_aparece_na_biblioteca_e_retoma_do_ponto() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("pc/Jogo/a.bin", &vec![1u8; 1024]);
        dump.write("pc/Jogo/jogo.exe", &vec![2u8; CHUNK * 2]);
        let store = Store::default();
        let items = plan_import(&dump.0, &store, &library.0);
        let game = item(&items, "Jogo");
        let cancel = no_cancel();
        let mut copied = 0;
        let result = copy_item(game, &cancel, &mut |bytes| {
            copied += bytes;
            // Já no segundo arquivo: o primeiro terminou e não deve ser copiado de novo.
            if copied > 1024 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(result, Err(CopyError::Cancelled));
        assert!(!library.0.join("pc").join("Jogo").exists(), "nada na pasta final");

        // Sobra de uma queda anterior é removida, e o que já terminou não é copiado de novo.
        let staging = library.0.join(STAGING_DIR).join("pc").join("Jogo");
        std::fs::write(staging.join(format!("velho.bin{PART_SUFFIX}")), b"lixo").unwrap();
        let again = plan_import(&dump.0, &store, &library.0);
        let game = item(&again, "Jogo");
        assert!(game.pending_bytes < game.size);
        let mut copied_now = 0;
        copy_item(game, &no_cancel(), &mut |b| copied_now += b).unwrap();
        assert_eq!(copied_now, game.pending_bytes);
        let dest = library.0.join("pc").join("Jogo");
        assert_eq!(std::fs::metadata(dest.join("jogo.exe")).unwrap().len(), (CHUNK * 2) as u64);
        assert!(!dest.join(format!("velho.bin{PART_SUFFIX}")).exists());
    }

    #[test]
    fn origem_fica_identica() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("cemu/Mario.wua", b"rom");
        dump.write("pc/Jogo/jogo.exe", b"exe");
        fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
            let (mut files, mut dirs) = (vec![], vec![]);
            walk_tree(root, Path::new(""), &mut files, &mut dirs).unwrap();
            let mut out: Vec<_> = files.iter().map(|f| (f.rel.clone(), std::fs::read(root.join(&f.rel)).unwrap())).collect();
            out.sort();
            out
        }
        let before = snapshot(&dump.0);
        let mut store = store_with_cemu();
        for i in plan_import(&dump.0, &store, &library.0) {
            import(&i, &mut store, None);
        }
        assert_eq!(snapshot(&dump.0), before);
    }

    #[test]
    fn pasta_iso_vai_inteira_e_nao_vira_jogo() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("iso/Twilight Princess/jogo.iso", b"iso");
        dump.write("iso/Mario 64/assets/sons/a.wav", b"som");
        dump.write("iso/Mario 64/assets/texturas/fundo/b.png", b"tex");
        dump.write("iso/Mario 64/ferramenta.exe", b"exe");
        dump.write("iso/Mario 64/sem_extensao", b"x");
        std::fs::create_dir_all(dump.0.join("iso/Mario 64/saves")).unwrap();
        let mut store = store_with_cemu();
        for i in plan_import(&dump.0, &store, &library.0) {
            assert_eq!(i.status, Status::New, "{i:?}");
            import(&i, &mut store, None);
        }

        let dest = library.0.join("iso").join("Mario 64");
        for rel in ["assets/sons/a.wav", "assets/texturas/fundo/b.png", "ferramenta.exe", "sem_extensao"] {
            assert!(dest.join(rel).is_file(), "{rel}");
        }
        assert!(dest.join("saves").is_dir());
        assert!(library.0.join("iso").join("Twilight Princess").join("jogo.iso").is_file());
        assert!(store.manual.is_empty(), "arquivos de jogo não entram na biblioteca");
        assert!(store.emulators[0].rom_dirs.is_empty());
        let again = plan_import(&dump.0, &store, &library.0);
        assert!(again.iter().all(|i| i.status == Status::Done), "{again:?}");
    }

    #[test]
    fn pasta_iso_nao_exige_exe_e_mantem_o_nome() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("ISO/Zelda (USA) [v1]/a.dat", b"x");
        let items = plan_import(&dump.0, &Store::default(), &library.0);
        let zelda = item(&items, "Zelda (USA) [v1]");
        assert_eq!(zelda.status, Status::New);
        assert_eq!(zelda.exe, None);
        assert!(zelda.exe_options.is_empty());
    }

    #[test]
    fn pasta_iso_vazia_nao_e_importada_e_arquivo_solto_e() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        std::fs::create_dir_all(dump.0.join("iso/Vazia")).unwrap();
        dump.write("iso/Solto.iso", b"iso");
        let mut store = Store::default();
        let items = plan_import(&dump.0, &store, &library.0);
        assert_eq!(item(&items, "Vazia").status, Status::Blocked);
        import(item(&items, "Solto"), &mut store, None);
        assert!(library.0.join("iso").join("Solto.iso").is_file());
    }

    #[test]
    fn pasta_iso_nao_sobrescreve() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("iso/Jogo/a.iso", b"novo tamanho");
        library.write("iso/Jogo/a.iso", b"meu");
        let items = plan_import(&dump.0, &Store::default(), &library.0);
        assert_eq!(item(&items, "Jogo").status, Status::Conflict);
    }

    #[test]
    fn lista_arquivos_de_jogos_com_imagens_em_subpastas() {
        let library = Temp::new("lib");
        library.write("iso/Com Imagem/disco/jogo.ISO", b"12345");
        library.write("iso/Com Imagem/leia.txt", b"123");
        library.write("iso/So Assets/a/b/c.png", b"1");
        library.write("iso/Solto.iso", b"12");
        library.write("pc/Outro/nao_listar.iso", b"1");
        let list = list_game_files(&library.0);
        assert_eq!(list.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["Com Imagem", "So Assets", "Solto"]);
        assert_eq!((list[0].size, list[0].file_count), (8, 2));
        assert_eq!(list[0].main_files.len(), 1);
        assert!(list[0].main_files[0].ends_with("disco\\jogo.ISO"));
        assert!(list[1].main_files.is_empty());
        assert_eq!(list[2].main_files.len(), 1);
    }

    #[test]
    fn capas_sao_importadas_e_ligadas_pelo_nome() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("capas/Dusklight.png", b"png");
        dump.write("capas/Batman Arkham Origins/arte.jpg", b"jpg");
        dump.write("capas/Sem Imagem/leia.txt", b"x");
        dump.write("capas/leia.txt", b"x");
        let mut store = store_with_cemu();
        let items = plan_import(&dump.0, &store, &library.0);
        assert_eq!(items.len(), 3, "{items:?}");
        assert_eq!(item(&items, "Sem Imagem").status, Status::Blocked);
        import(item(&items, "Dusklight"), &mut store, None);
        import(item(&items, "Batman Arkham Origins"), &mut store, None);

        assert!(store.manual.is_empty(), "capa não vira jogo");
        assert!(store.emulators[0].rom_dirs.is_empty());
        let covers = art::imported_covers(&library.0);
        assert!(art::local_cover("manual:x", "Dusklight", &covers).unwrap().ends_with("Dusklight.png"));
        assert!(art::local_cover("steam:209000", "Batman™: Arkham Origins", &covers).unwrap().ends_with("arte.jpg"));

        let again = plan_import(&dump.0, &store, &library.0);
        assert_eq!(item(&again, "Dusklight").status, Status::Done);
        assert_eq!(item(&again, "Batman Arkham Origins").status, Status::Done);
    }

    #[test]
    fn capa_diferente_com_mesmo_nome_nao_sobrescreve() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("capas/Dusklight.png", b"nova e maior");
        library.write("capas/Dusklight.png", b"minha");
        let items = plan_import(&dump.0, &Store::default(), &library.0);
        assert_eq!(item(&items, "Dusklight").status, Status::Conflict);
        assert!(copy_item(item(&items, "Dusklight"), &no_cancel(), &mut |_| {}).is_err());
        assert_eq!(std::fs::read(library.0.join("capas").join("Dusklight.png")).unwrap(), b"minha");
    }

    #[test]
    fn arquivo_automatico_pelo_nome_do_jogo() {
        let library = Temp::new("lib");
        library.write("iso/Dusklight/disco/jogo.iso", b"x");
        library.write("iso/Dusklight/leia.txt", b"x");
        library.write("iso/So Assets/a/b.png", b"x");
        library.write("iso/Duas/a.iso", b"x");
        library.write("iso/Duas/b.iso", b"x");
        library.write("iso/Zelda (USA)/z.iso", b"x");

        assert!(auto_file_for("DUSKLIGHT", &library.0).unwrap().ends_with("disco\\jogo.iso"));
        assert!(auto_file_for("So Assets", &library.0).unwrap().ends_with("So Assets"));
        assert_eq!(auto_file_for("Duas", &library.0), None, "com várias imagens, não adivinha");
        assert!(auto_file_for("Zelda", &library.0).unwrap().ends_with("z.iso"));
        assert_eq!(auto_file_for("Dusk", &library.0), None, "nome parecido não conta");
    }

    #[test]
    fn sofa_json_preserva_o_marcador_de_arquivo() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("pc/Dusklight/dusklight.exe", b"exe");
        dump.write("pc/Dusklight/sofa.json", br#"{"exe":"dusklight.exe","args":["{iso}"]}"#);
        let mut store = Store::default();
        let items = plan_import(&dump.0, &store, &library.0);
        import(item(&items, "Dusklight"), &mut store, None);
        assert_eq!(store.manual[0].args, vec!["{iso}".to_string()]);
    }

    #[test]
    fn sem_pasta_iso_a_lista_fica_vazia() {
        let library = Temp::new("lib");
        assert!(list_game_files(&library.0).is_empty());
    }

    #[test]
    fn jogo_de_ps3_usa_titulo_e_exige_eboot() {
        let dump = Temp::new("dump");
        let library = Temp::new("lib");
        dump.write("rpcs3/Valido/PS3_GAME/USRDIR/EBOOT.BIN", b"x");
        dump.write("rpcs3/Valido/PS3_GAME/PARAM.SFO", b"sem titulo");
        dump.write("rpcs3/Invalido/a.bin", b"x");
        let mut store = Store::default();
        store.emulators.push(preset("rpcs3", r"C:\nao-existe\rpcs3.exe"));
        let items = plan_import(&dump.0, &store, &library.0);
        assert_eq!(item(&items, "Valido").status, Status::New);
        assert_eq!(item(&items, "Invalido").status, Status::Blocked);
    }
}
