//! Steam: registro → libraryfolders.vdf → appmanifest_*.acf.

use super::{Game, Launch, UpdateState};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum Vdf {
    Str(String),
    Map(Vec<(String, Vdf)>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Map(items) => items.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Vdf::Str(s)) => Some(s),
            _ => None,
        }
    }
}

/// Leitor do formato de texto KeyValues da Valve.
pub fn parse_vdf(text: &str) -> Vdf {
    let mut chars = text.chars().peekable();
    Vdf::Map(parse_map(&mut chars))
}

fn parse_map(chars: &mut std::iter::Peekable<std::str::Chars>) -> Vec<(String, Vdf)> {
    let mut items = vec![];
    let mut key: Option<String> = None;
    while let Some(&c) = chars.peek() {
        match c {
            '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        c => s.push(c),
                    }
                }
                match key.take() {
                    Some(k) => items.push((k, Vdf::Str(s))),
                    None => key = Some(s),
                }
            }
            '{' => {
                chars.next();
                let inner = parse_map(chars);
                if let Some(k) = key.take() {
                    items.push((k, Vdf::Map(inner)));
                }
            }
            '}' => {
                chars.next();
                return items;
            }
            '/' => {
                while let Some(c) = chars.next() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {
                chars.next();
            }
        }
    }
    items
}

pub fn library_paths(vdf_text: &str) -> Vec<String> {
    let doc = parse_vdf(vdf_text);
    let mut out = vec![];
    if let Some(Vdf::Map(items)) = doc.get("libraryfolders") {
        for (_, entry) in items {
            if let Some(path) = entry.str("path") {
                out.push(path.to_string());
            }
        }
    }
    out
}

pub struct Manifest {
    pub appid: String,
    pub name: String,
    pub installdir: String,
    pub update: UpdateState,
    /// Tamanho instalado, informado pela Steam.
    pub size: u64,
}

// Bits de StateFlags. O 64 (jogo aberto) e o 512 (atualização pausada) não mudam o estado aqui:
// pausada continua como pendente.
const UPDATE_REQUIRED: u32 = 2;
const FULLY_INSTALLED: u32 = 4;
const UPDATE_RUNNING: u32 = 256;
const UPDATE_STARTED: u32 = 1024;

fn update_state(flags: u32, done: u64, total: u64) -> UpdateState {
    if flags & (UPDATE_RUNNING | UPDATE_STARTED) != 0 && flags & 512 == 0 {
        UpdateState::Downloading { done, total }
    } else if flags & UPDATE_REQUIRED != 0 {
        UpdateState::Pending
    } else {
        UpdateState::UpToDate
    }
}

/// Ferramentas que a Steam instala como "apps", mas não são jogos.
const NON_GAME_IDS: &[&str] = &["228980", "431960", "250820", "1070560", "1391110", "1628350", "1493710", "1887720", "2180100"];

pub fn is_game(m: &Manifest) -> bool {
    let name = m.name.to_lowercase();
    !NON_GAME_IDS.contains(&m.appid.as_str())
        && !name.contains("redistributable")
        && !name.starts_with("proton ")
        && !name.starts_with("steam linux runtime")
        && !name.starts_with("steamvr")
}

pub fn parse_manifest(text: &str) -> Option<Manifest> {
    let doc = parse_vdf(text);
    let state = doc.get("AppState")?;
    let flags: u32 = state.str("StateFlags")?.parse().ok()?;
    // Durante uma atualização a Steam desliga o bit de "totalmente instalado". O que separa
    // isso de uma primeira instalação é já existir uma versão instalada (`buildid`).
    let installed_before = state.str("buildid").is_some_and(|b| !b.is_empty() && b != "0");
    let busy = flags & (UPDATE_REQUIRED | UPDATE_RUNNING | UPDATE_STARTED) != 0;
    let bytes = |key: &str| state.str(key).and_then(|v| v.parse().ok()).unwrap_or(0);
    let (done, total) = (bytes("BytesDownloaded"), bytes("BytesToDownload"));
    let update = if flags & FULLY_INSTALLED != 0 || (busy && installed_before) {
        update_state(flags, done, total)
    } else if busy {
        // Primeira instalação: aparece na biblioteca com o progresso, ainda sem poder jogar.
        UpdateState::Installing { done, total }
    } else {
        return None;
    };
    Some(Manifest {
        appid: state.str("appid")?.to_string(),
        name: state.str("name")?.to_string(),
        installdir: state.str("installdir")?.to_string(),
        update,
        size: bytes("SizeOnDisk"),
    })
}

/// Relê o manifesto para acompanhar uma atualização.
pub fn read_update(manifest: &str) -> Option<UpdateState> {
    parse_manifest(&std::fs::read_to_string(manifest).ok()?).map(|m| m.update)
}

/// A Steam só descobre atualizações aberta. Se estiver fechada, abre sem mostrar janela.
pub fn ensure_running() -> Result<bool, String> {
    if crate::session::process_running("steam.exe") {
        return Ok(false);
    }
    let root = steam_root().ok_or("A Steam não está instalada.")?;
    std::process::Command::new(root.join("steam.exe"))
        .arg("-silent")
        .spawn()
        .map_err(|e| format!("Não foi possível abrir a Steam: {e}"))?;
    Ok(true)
}

fn steam_root() -> Option<PathBuf> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    Some(PathBuf::from(path.replace('/', "\\")))
}

/// Pastas de biblioteca da Steam (cada uma com uma `steamapps`).
pub fn libraries() -> Vec<PathBuf> {
    let Some(root) = steam_root() else { return vec![] };
    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    let mut libraries: Vec<PathBuf> =
        std::fs::read_to_string(vdf).map(|t| library_paths(&t)).unwrap_or_default().into_iter().map(PathBuf::from).collect();
    if libraries.is_empty() {
        libraries.push(root);
    }
    libraries
}

// ---- Fechar e reabrir a Steam ----

const STEAM_EXE: &str = "steam.exe";

/// Fecha a Steam, faz `work` e reabre em segundo plano. Mexer nos manifestos com ela aberta
/// não funciona: ela reescreve os arquivos a partir do que tem na memória.
fn with_steam_closed<T>(reopen: bool, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let root = steam_root().ok_or("A Steam não está instalada.")?;
    let exe = root.join(STEAM_EXE);
    let was_running = crate::session::process_running(STEAM_EXE);
    if was_running {
        std::process::Command::new(&exe).arg("-shutdown").spawn().map_err(|e| format!("Não foi possível fechar a Steam: {e}"))?;
        if !crate::session::wait_exit(STEAM_EXE, std::time::Duration::from_secs(60)) {
            return Err("A Steam não fechou a tempo. Tente de novo em instantes.".into());
        }
    }
    let result = work();
    if reopen || was_running {
        let _ = std::process::Command::new(&exe).arg("-silent").spawn();
    }
    result
}

// ---- Desinstalar ----

/// Nome de pasta seguro: sem separadores nem `..`, para nunca apagar fora de `common`.
fn safe_dir_name(name: &str) -> bool {
    !name.trim().is_empty() && !name.contains(['\\', '/', ':']) && name.trim_matches('.').len() == name.len() && name != ".."
}

/// Apaga o jogo, o manifesto e o conteúdo da Oficina. A Steam fica fechada durante a remoção.
pub fn uninstall(manifest_path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(manifest_path).map_err(|e| format!("Não foi possível ler o manifesto: {e}"))?;
    let doc = parse_vdf(&text);
    let state = doc.get("AppState").ok_or("Manifesto inválido.")?;
    let appid = state.str("appid").ok_or("Manifesto sem appid.")?.to_string();
    let installdir = state.str("installdir").unwrap_or_default().to_string();
    let steamapps = manifest_path.parent().ok_or("Manifesto fora de uma biblioteca.")?.to_path_buf();
    if !appid.chars().all(|c| c.is_ascii_digit()) {
        return Err("Manifesto inválido.".into());
    }
    with_steam_closed(false, || {
        if safe_dir_name(&installdir) {
            let dir = steamapps.join("common").join(&installdir);
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("Não foi possível apagar {}: {e}", dir.display()))?;
            }
        }
        let workshop = steamapps.join("workshop");
        let _ = std::fs::remove_dir_all(workshop.join("content").join(&appid));
        let _ = std::fs::remove_file(workshop.join(format!("appworkshop_{appid}.acf")));
        std::fs::remove_file(manifest_path).map_err(|e| format!("Não foi possível apagar o manifesto: {e}"))
    })
}

// ---- Instalar ----

/// Nome de pasta a partir do título, sem caracteres que o Windows recusa.
pub fn install_dir_name(title: &str) -> String {
    let cleaned: String = title.chars().filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control()).collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim().to_string();
    if cleaned.is_empty() { "Jogo".into() } else { cleaned }
}

/// Manifesto mínimo de um jogo a instalar: com a Steam reaberta, ela começa o download.
pub fn install_manifest(appid: &str, name: &str) -> String {
    let name = name.replace('"', "'");
    let dir = install_dir_name(&name);
    format!(
        "\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"Universe\"\t\t\"1\"\n\t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"1026\"\n\t\"installdir\"\t\t\"{dir}\"\n\t\"LastUpdated\"\t\t\"0\"\n\t\"UpdateResult\"\t\t\"0\"\n\t\"SizeOnDisk\"\t\t\"0\"\n\t\"buildid\"\t\t\"0\"\n\t\"LastOwner\"\t\t\"0\"\n\t\"BytesToDownload\"\t\t\"0\"\n\t\"BytesDownloaded\"\t\t\"0\"\n\t\"AutoUpdateBehavior\"\t\t\"0\"\n\t\"AllowOtherDownloadsWhileRunning\"\t\t\"0\"\n\t\"ScheduledAutoUpdate\"\t\t\"0\"\n}}\n"
    )
}

/// Se já existe um manifesto deste jogo em alguma biblioteca.
pub fn has_manifest(appid: &str) -> bool {
    libraries().iter().any(|l| l.join("steamapps").join(format!("appmanifest_{appid}.acf")).exists())
}

/// Pasta criada como biblioteca nova num disco que ainda não tem uma.
pub const NEW_LIBRARY_DIR: &str = "SteamLibrary";

/// Acrescenta uma biblioteca ao `libraryfolders.vdf`, com o próximo número livre.
pub fn add_library_entry(vdf_text: &str, path: &str, content_id: u64) -> Result<String, String> {
    let doc = parse_vdf(vdf_text);
    let Some(Vdf::Map(items)) = doc.get("libraryfolders") else {
        return Err("Arquivo de bibliotecas da Steam inválido.".into());
    };
    let next = items.iter().filter_map(|(k, _)| k.parse::<u32>().ok()).max().map_or(0, |n| n + 1);
    let end = vdf_text.rfind('}').ok_or("Arquivo de bibliotecas da Steam inválido.")?;
    let path = path.replace('\\', "\\\\");
    let entry = format!(
        "\t\"{next}\"\n\t{{\n\t\t\"path\"\t\t\"{path}\"\n\t\t\"label\"\t\t\"\"\n\t\t\"contentid\"\t\t\"{content_id}\"\n\t\t\"totalsize\"\t\t\"0\"\n\t\t\"update_clean_bytes_tally\"\t\t\"0\"\n\t\t\"time_last_update_verified\"\t\t\"0\"\n\t\t\"apps\"\n\t\t{{\n\t\t}}\n\t}}\n"
    );
    Ok(format!("{}{entry}{}", &vdf_text[..end], &vdf_text[end..]))
}

/// Cria `<disco>\SteamLibrary` e a registra na Steam (com ela fechada).
fn create_library(library: &Path) -> Result<(), String> {
    let root = steam_root().ok_or("A Steam não está instalada.")?;
    let vdf_path = root.join("steamapps").join("libraryfolders.vdf");
    let text = std::fs::read_to_string(&vdf_path).map_err(|e| format!("Não foi possível ler as bibliotecas da Steam: {e}"))?;
    let content_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64 | 1);
    std::fs::create_dir_all(library.join("steamapps")).map_err(|e| format!("Não foi possível criar {}: {e}", library.display()))?;
    std::fs::write(
        library.join("libraryfolder.vdf"),
        format!("\"libraryfolder\"\n{{\n\t\"contentid\"\t\t\"{content_id}\"\n\t\"label\"\t\t\"\"\n}}\n"),
    )
    .map_err(|e| e.to_string())?;
    let updated = add_library_entry(&text, &library.to_string_lossy(), content_id)?;
    std::fs::write(&vdf_path, updated).map_err(|e| format!("Não foi possível registrar a biblioteca na Steam: {e}"))
}

/// Biblioteca nova permitida: `<disco fixo>\SteamLibrary`, num disco que ainda não tem biblioteca.
fn is_new_library(library: &Path) -> bool {
    let text = library.to_string_lossy();
    let Some(drive) = crate::storage::drive_of(&text) else { return false };
    let expected = Path::new(&drive).join(NEW_LIBRARY_DIR);
    text.eq_ignore_ascii_case(&expected.to_string_lossy())
        && crate::storage::fixed_drives().contains(&drive)
        && !libraries().iter().any(|l| crate::storage::drive_of(&l.to_string_lossy()).as_deref() == Some(drive.as_str()))
}

/// Tamanho pedido pela loja, lido do texto de requisitos ("Storage: 60 GB available space").
/// Usa o maior entre o mínimo e o recomendado.
pub fn parse_required_size(requirements_html: &str) -> Option<u64> {
    let mut text = String::new();
    let mut in_tag = false;
    for c in requirements_html.chars() {
        match c {
            '<' => {
                in_tag = true;
                text.push('\n');
            }
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let keys = ["storage", "hard drive", "hard disk", "hdd", "disk space", "available space"];
    let mut best: Option<u64> = None;
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for (i, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if !keys.iter().any(|k| lower.contains(k)) {
            continue;
        }
        // O valor costuma estar na mesma linha ou na seguinte ("Storage:" em negrito, depois o número).
        let candidate = std::iter::once(lower.clone()).chain(lines.get(i + 1).map(|l| l.to_lowercase()));
        for l in candidate {
            if let Some(size) = first_size(&l) {
                best = Some(best.map_or(size, |b| b.max(size)));
                break;
            }
        }
    }
    best
}

fn first_size(text: &str) -> Option<u64> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',') {
                i += 1;
            }
            let number: f64 = chars[start..i].iter().collect::<String>().replace(',', ".").parse().ok()?;
            let rest: String = chars[i..].iter().collect::<String>().trim_start().to_string();
            let unit = if rest.starts_with("tb") {
                1u64 << 40
            } else if rest.starts_with("gb") {
                1 << 30
            } else if rest.starts_with("mb") {
                1 << 20
            } else {
                continue;
            };
            return Some((number * unit as f64) as u64);
        }
        i += 1;
    }
    None
}

/// Tamanho aproximado do jogo pela página da loja; `None` se a loja não informa.
pub fn store_size(appid: &str) -> Option<u64> {
    let json: serde_json::Value = ureq::get("https://store.steampowered.com/api/appdetails")
        .query("appids", appid)
        .query("l", "english")
        .query("filters", "basic")
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let req = json.get(appid)?.get("data")?.get("pc_requirements")?;
    let html = ["minimum", "recommended"].iter().filter_map(|k| req.get(k)?.as_str()).collect::<Vec<_>>().join("\n");
    parse_required_size(&html)
}

pub fn install(appid: &str, name: &str, library: &Path) -> Result<(), String> {
    if !appid.chars().all(|c| c.is_ascii_digit()) || appid.is_empty() {
        return Err("Jogo inválido.".into());
    }
    if has_manifest(appid) {
        return Err(format!("{name} já está instalado ou sendo instalado."));
    }
    let new_library = !libraries().iter().any(|l| l == library);
    if new_library && !is_new_library(library) {
        return Err("Essa pasta não é uma biblioteca da Steam.".into());
    }
    let steamapps = library.join("steamapps");
    with_steam_closed(true, || {
        if new_library {
            create_library(library)?;
        }
        std::fs::create_dir_all(&steamapps).map_err(|e| e.to_string())?;
        std::fs::write(steamapps.join(format!("appmanifest_{appid}.acf")), install_manifest(appid, name))
            .map_err(|e| format!("Não foi possível preparar a instalação: {e}"))
    })
}

// ---- Jogos da conta ----

/// A conta usada por último neste PC, em `config\loginusers.vdf`.
pub fn parse_steam_id(loginusers: &str) -> Option<String> {
    let doc = parse_vdf(loginusers);
    let Some(Vdf::Map(users)) = doc.get("users") else { return None };
    let recent = users.iter().find(|(_, u)| u.str("MostRecent") == Some("1"));
    recent.or(users.first()).map(|(id, _)| id.clone())
}

pub fn steam_id() -> Option<String> {
    let root = steam_root()?;
    parse_steam_id(&std::fs::read_to_string(root.join("config").join("loginusers.vdf")).ok()?)
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Owned {
    pub appid: String,
    pub name: String,
}

pub fn parse_owned(json: &serde_json::Value) -> Option<Vec<Owned>> {
    let games = json.get("response")?.get("games")?.as_array()?;
    Some(
        games
            .iter()
            .filter_map(|g| Some(Owned { appid: g.get("appid")?.as_u64()?.to_string(), name: g.get("name")?.as_str()?.to_string() }))
            .collect(),
    )
}

/// Jogos da conta pela API oficial. Precisa da chave e de os detalhes de jogos do perfil não
/// estarem privados.
pub fn owned_games(key: &str, steam_id: &str) -> Result<Vec<Owned>, String> {
    let response = ureq::get("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/")
        .query("key", key)
        .query("steamid", steam_id)
        .query("include_appinfo", "1")
        .query("include_played_free_games", "1")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(401 | 403, _) => "A Steam recusou a chave da API. Confira em Configurações.".to_string(),
            other => format!("Não foi possível falar com a Steam: {other}"),
        })?;
    let json: serde_json::Value = response.into_json().map_err(|e| e.to_string())?;
    parse_owned(&json).ok_or_else(|| {
        "A Steam não devolveu a lista de jogos. Deixe os detalhes de jogos do perfil como públicos nas configurações de privacidade da Steam.".to_string()
    })
}

/// Capa local, se a Steam já tiver guardado.
pub fn cached_art(appid: &str) -> Option<String> {
    find_art(&steam_root()?, appid)
}

/// Procura uma imagem no cache do jogo, incluindo as subpastas com hash do formato novo.
fn walk(dir: &Path, depth: u8, prefix: &str) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().collect();
    for e in &entries {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if name.starts_with(prefix) && name.ends_with(".jpg") {
            return Some(e.path());
        }
    }
    if depth > 0 {
        for e in &entries {
            if e.path().is_dir() {
                if let Some(found) = walk(&e.path(), depth - 1, prefix) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Arte grande do jogo. O ponto no prefixo deixa de fora a versão desfocada, `library_hero_blur.jpg`.
fn find_hero(root: &Path, appid: &str) -> Option<String> {
    let cache = root.join("appcache").join("librarycache");
    let legacy = cache.join(format!("{appid}_library_hero.jpg"));
    if legacy.exists() {
        return Some(legacy.to_string_lossy().into_owned());
    }
    walk(&cache.join(appid), 2, "library_hero.").map(|p| p.to_string_lossy().into_owned())
}

/// Procura a capa no cache local, nos formatos antigo e novo (pasta por jogo).
fn find_art(root: &Path, appid: &str) -> Option<String> {
    let cache = root.join("appcache").join("librarycache");
    let legacy = cache.join(format!("{appid}_library_600x900.jpg"));
    if legacy.exists() {
        return Some(legacy.to_string_lossy().into_owned());
    }
    let dir = cache.join(appid);
    // Da capa vertical para os formatos horizontais, que são cortados no cartão.
    ["library_600x900", "library_capsule", "library_header", "header"]
        .iter()
        .find_map(|prefix| walk(&dir, 2, prefix))
        .map(|p| p.to_string_lossy().into_owned())
}

/// Tamanho instalado lido do manifesto.
pub fn read_size(manifest: &str) -> Option<u64> {
    parse_manifest(&std::fs::read_to_string(manifest).ok()?).map(|m| m.size)
}

pub fn scan() -> Vec<Game> {
    let Some(root) = steam_root() else { return vec![] };
    let mut games = vec![];
    for library in libraries() {
        let steamapps = library.join("steamapps");
        let Ok(entries) = std::fs::read_dir(&steamapps) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
            let Some(m) = parse_manifest(&text).filter(is_game) else { continue };
            let manifest = Some(entry.path().to_string_lossy().into_owned());
            games.push(Game {
                id: format!("steam:{}", m.appid),
                title: m.name,
                source: "steam".into(),
                launch: Launch::Uri {
                    uri: format!("steam://rungameid/{}", m.appid),
                    watch_dir: steamapps.join("common").join(&m.installdir).to_string_lossy().into_owned(),
                },
                art: find_art(&root, &m.appid),
                hero: find_hero(&root, &m.appid),
                manifest,
                update: Some(m.update),
            });
        }
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "70" "123" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;

    #[test]
    fn arte_grande_ignora_a_versao_desfocada() {
        let root = std::env::temp_dir().join(format!("sofa-hero-{}", std::process::id()));
        let game = root.join(r"appcache\librarycache\98");
        std::fs::create_dir_all(game.join("abc123")).unwrap();
        std::fs::write(game.join("library_hero_blur.jpg"), b"").unwrap();
        assert_eq!(find_hero(&root, "98"), None);
        std::fs::write(game.join(r"abc123\library_hero.jpg"), b"").unwrap();
        assert!(find_hero(&root, "98").unwrap().ends_with(r"abc123\library_hero.jpg"));
        assert_eq!(find_hero(&root, "sem-pasta"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn capa_prefere_vertical_e_aceita_pasta_com_hash() {
        let root = std::env::temp_dir().join(format!("sofa-art-{}", std::process::id()));
        let game = root.join(r"appcache\librarycache\99");
        std::fs::create_dir_all(game.join("abc123")).unwrap();
        std::fs::write(game.join("header.jpg"), b"").unwrap();
        assert!(find_art(&root, "99").unwrap().ends_with("header.jpg"));
        std::fs::write(game.join(r"abc123\library_capsule.jpg"), b"").unwrap();
        assert!(find_art(&root, "99").unwrap().ends_with("library_capsule.jpg"));
        std::fs::write(game.join("library_600x900.jpg"), b"").unwrap();
        assert!(find_art(&root, "99").unwrap().ends_with("library_600x900.jpg"));
        assert_eq!(find_art(&root, "sem-pasta"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn le_bibliotecas_com_barras_escapadas() {
        assert_eq!(library_paths(LIBS), vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]);
    }

    fn manifest(flags: u32, id: &str, name: &str) -> String {
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"Half-Life\"\n\t\"UserConfig\"\n\t{{\n\t\t\"language\"\t\t\"brazilian\"\n\t}}\n}}\n"
        )
    }

    #[test]
    fn le_manifesto_instalado() {
        let m = parse_manifest(&manifest(4, "70", "Half-Life")).unwrap();
        assert_eq!((m.appid.as_str(), m.name.as_str(), m.installdir.as_str()), ("70", "Half-Life", "Half-Life"));
    }

    #[test]
    fn primeira_instalacao_aparece_instalando() {
        let m = parse_manifest(&manifest(1026, "70", "Half-Life")).unwrap();
        assert_eq!(m.update, UpdateState::Installing { done: 0, total: 0 });
        // Sem nada em andamento e sem instalação completa, fica de fora.
        assert!(parse_manifest(&manifest(0, "70", "Half-Life")).is_none());
    }

    #[test]
    fn manifesto_de_instalacao_e_lido_como_instalando() {
        let text = install_manifest("620", "Portal 2");
        let m = parse_manifest(&text).unwrap();
        assert_eq!((m.appid.as_str(), m.name.as_str(), m.installdir.as_str()), ("620", "Portal 2", "Portal 2"));
        assert_eq!(m.update, UpdateState::Installing { done: 0, total: 0 });
    }

    #[test]
    fn nome_de_pasta_sem_caracteres_proibidos() {
        assert_eq!(install_dir_name("Batman™: Arkham Origins"), "Batman™ Arkham Origins");
        assert_eq!(install_dir_name("Jogo?."), "Jogo");
        assert_eq!(install_dir_name("///"), "Jogo");
        assert!(safe_dir_name("Half-Life"));
        for bad in ["", "..", "a\\b", "C:", "x/y", "."] {
            assert!(!safe_dir_name(bad), "{bad}");
        }
    }

    #[test]
    fn le_tamanho_instalado() {
        let text = manifest(4, "70", "Half-Life").replacen("\"installdir\"", "\"SizeOnDisk\"\t\t\"123456\"\n\t\"installdir\"", 1);
        assert_eq!(parse_manifest(&text).unwrap().size, 123456);
    }

    #[test]
    fn conta_mais_recente() {
        let text = r#""users"
{
	"76561190000000001" { "AccountName" "outro" "MostRecent" "0" }
	"76561190000000002" { "AccountName" "eu" "MostRecent" "1" }
}"#;
        assert_eq!(parse_steam_id(text).as_deref(), Some("76561190000000002"));
        assert_eq!(parse_steam_id(r#""users" { "765" { "MostRecent" "0" } }"#).as_deref(), Some("765"));
        assert_eq!(parse_steam_id("nada"), None);
    }

    #[test]
    fn le_jogos_da_conta() {
        let json = serde_json::json!({"response": {"game_count": 2, "games": [
            {"appid": 620, "name": "Portal 2", "playtime_forever": 0},
            {"appid": 70, "name": "Half-Life"}
        ]}});
        let owned = parse_owned(&json).unwrap();
        assert_eq!(owned[0], Owned { appid: "620".into(), name: "Portal 2".into() });
        assert_eq!(owned.len(), 2);
        // Perfil privado: a resposta vem vazia.
        assert_eq!(parse_owned(&serde_json::json!({"response": {}})), None);
    }

    fn manifest_with_build(flags: u32, buildid: &str, done: u64, total: u64) -> String {
        manifest(flags, "70", "Half-Life").replacen(
            "\"installdir\"",
            &format!("\"buildid\"\t\t\"{buildid}\"\n\t\"BytesDownloaded\"\t\t\"{done}\"\n\t\"BytesToDownload\"\t\t\"{total}\"\n\t\"installdir\""),
            1,
        )
    }

    #[test]
    fn le_estado_de_atualizacao() {
        let state = |flags, done, total| parse_manifest(&manifest_with_build(flags, "123", done, total)).map(|m| m.update);
        assert_eq!(state(4, 0, 0), Some(UpdateState::UpToDate));
        assert_eq!(state(6, 0, 0), Some(UpdateState::Pending));
        // Atualizando, o jogo continua na biblioteca, com o progresso.
        assert_eq!(state(1026, 500, 2000), Some(UpdateState::Downloading { done: 500, total: 2000 }));
        assert_eq!(state(1028, 0, 0), Some(UpdateState::Downloading { done: 0, total: 0 }));
        assert_eq!(state(4 | 2 | 256, 7, 9), Some(UpdateState::Downloading { done: 7, total: 9 }));
        // Pausada continua pendente; jogo aberto (64) não muda nada.
        assert_eq!(state(4 | 2 | 1024 | 512, 7, 9), Some(UpdateState::Pending));
        assert_eq!(state(68, 0, 0), Some(UpdateState::UpToDate));
        // Primeira instalação: sem versão anterior, aparece como instalando.
        assert_eq!(
            parse_manifest(&manifest_with_build(1026, "0", 1, 2)).map(|m| m.update),
            Some(UpdateState::Installing { done: 1, total: 2 })
        );
    }

    #[test]
    fn filtra_o_que_nao_e_jogo() {
        for (id, name) in [("228980", "Steamworks Common Redistributables"), ("431960", "Wallpaper Engine"), ("999", "Proton 9.0")] {
            let m = parse_manifest(&manifest(4, id, name)).unwrap();
            assert!(!is_game(&m), "{name}");
        }
        assert!(is_game(&parse_manifest(&manifest(4, "730", "Counter-Strike 2")).unwrap()));
    }

    #[test]
    fn tamanho_pelos_requisitos_da_loja() {
        let min = "<strong>Minimum:</strong><br><ul class=\"bb_ul\"><li><strong>Memory:</strong> 8 GB RAM<br></li><li><strong>Storage:</strong> 10 GB available space</li></ul>";
        let rec = "<strong>Recommended:</strong><ul><li><strong>Hard Drive:</strong> 25 GB available space</li></ul>";
        assert_eq!(parse_required_size(&format!("{min}\n{rec}")), Some(25 << 30));
        assert_eq!(parse_required_size("<li><strong>Storage:</strong> 500 MB available space</li>"), Some(500 << 20));
        assert_eq!(parse_required_size("<li><strong>Storage:</strong> 1,5 GB</li>"), Some((1.5 * (1u64 << 30) as f64) as u64));
        assert_eq!(parse_required_size("<li><strong>Memory:</strong> 8 GB RAM</li>"), None);
    }

    #[test]
    fn registra_biblioteca_nova() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"c:\\\\program files (x86)\\\\steam\"\n\t\t\"apps\"\n\t\t{\n\t\t\t\"10\"\t\t\"1\"\n\t\t}\n\t}\n}\n";
        let out = add_library_entry(vdf, r"D:\SteamLibrary", 42).unwrap();
        assert_eq!(library_paths(&out), vec![r"c:\program files (x86)\steam".to_string(), r"D:\SteamLibrary".to_string()]);
        let doc = parse_vdf(&out);
        assert_eq!(doc.get("libraryfolders").and_then(|l| l.get("1")).and_then(|e| e.str("contentid")), Some("42"));
        assert!(add_library_entry("lixo", r"D:\x", 1).is_err());
    }
}
