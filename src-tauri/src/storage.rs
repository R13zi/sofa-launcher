//! Armazenamento: espaço de cada disco, tamanho de cada jogo e desinstalação.

use crate::providers::{epic, steam, Game, Launch};
use crate::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW};

const DRIVE_FIXED: u32 = 3;

#[derive(Serialize, Clone, Debug)]
pub struct GameSize {
    pub id: String,
    pub title: String,
    pub source: String,
    pub art: Option<String>,
    pub size: u64,
    /// O Sofa sabe apagar este jogo.
    pub removable: bool,
}

#[derive(Serialize)]
pub struct DriveView {
    pub root: String,
    pub total: u64,
    pub free: u64,
    pub games: Vec<GameSize>,
}

/// Espaço livre e total de um disco ou pasta.
pub fn disk_space(path: &Path) -> Option<(u64, u64)> {
    let (mut free, mut total) = (0u64, 0u64);
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(path.as_os_str()), Some(&mut free), Some(&mut total), None).ok()? };
    Some((free, total))
}

/// Discos fixos (HDs e SSDs internos), sem pendrives nem unidades de rede.
pub fn fixed_drives() -> Vec<String> {
    ('A'..='Z')
        .map(|l| format!("{l}:\\"))
        .filter(|root| unsafe { GetDriveTypeW(&HSTRING::from(root.as_str())) } == DRIVE_FIXED)
        .collect()
}

/// Raiz do disco de um caminho, como `C:\`.
pub fn drive_of(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        .then(|| format!("{}:\\", (bytes[0] as char).to_ascii_uppercase()))
}

/// Pasta de um item da biblioteca do Sofa: `<biblioteca>\<categoria>\<item>`. Fora da
/// biblioteca, ou na própria categoria, não há nada que o Sofa possa apagar.
pub fn library_item(path: &Path, library: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let key = |c: &Component| c.as_os_str().to_string_lossy().to_lowercase();
    let lib: Vec<Component> = library.components().collect();
    let parts: Vec<Component> = path.components().collect();
    if lib.len() < 2 || parts.len() < lib.len() + 2 || lib.iter().zip(&parts).any(|(a, b)| key(a) != key(b)) {
        return None;
    }
    let (Component::Normal(category), Component::Normal(item)) = (parts[lib.len()], parts[lib.len() + 1]) else {
        return None;
    };
    if category.to_string_lossy().starts_with('.') {
        return None;
    }
    Some(library.join(category).join(item))
}

pub fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path).map(|entries| entries.flatten().map(|e| dir_size(&e.path())).sum()).unwrap_or(0)
}

/// Arquivo do jogo de emulador, guardado no id: `emu:<emulador>:<caminho>`.
fn rom_path(game: &Game) -> Option<PathBuf> {
    game.id.strip_prefix("emu:")?.split_once(':').map(|(_, rom)| PathBuf::from(rom))
}

/// O que apagar e onde o jogo está.
enum Location {
    Steam { manifest: String, dir: String },
    Epic { manifest: String, dir: String },
    /// Pasta (ou arquivo) dentro da biblioteca do Sofa.
    Library(PathBuf),
    /// Fora da biblioteca: só dá para tirar da lista.
    External(PathBuf),
}

fn locate(game: &Game, library: &Path) -> Option<Location> {
    let watch_dir = match &game.launch {
        Launch::Uri { watch_dir, .. } => Some(watch_dir.clone()),
        Launch::Exec { .. } => None,
    };
    match game.source.as_str() {
        "steam" => Some(Location::Steam { manifest: game.manifest.clone()?, dir: watch_dir? }),
        "epic" => Some(Location::Epic { manifest: game.manifest.clone()?, dir: watch_dir? }),
        "manual" => {
            let Launch::Exec { path, .. } = &game.launch else { return None };
            let exe = Path::new(path);
            Some(library_item(exe, library).map(Location::Library).unwrap_or_else(|| Location::External(exe.to_path_buf())))
        }
        _ => {
            let rom = rom_path(game)?;
            Some(library_item(&rom, library).map(Location::Library).unwrap_or(Location::External(rom)))
        }
    }
}

fn measure(game: &Game, library: &Path) -> Option<(String, GameSize)> {
    let location = locate(game, library)?;
    let (where_, size, removable) = match &location {
        Location::Steam { manifest, dir } => (dir.clone(), steam::read_size(manifest).unwrap_or(0), true),
        Location::Epic { manifest, dir } => (dir.clone(), epic::read_size(manifest).unwrap_or(0), true),
        Location::Library(path) => (path.to_string_lossy().into_owned(), dir_size(path), true),
        Location::External(path) => (path.to_string_lossy().into_owned(), 0, false),
    };
    let drive = drive_of(&where_)?;
    Some((drive, GameSize { id: game.id.clone(), title: game.title.clone(), source: game.source.clone(), art: game.art.clone(), size, removable }))
}

/// Discos fixos com os jogos de cada um, do maior para o menor.
pub fn overview(state: &AppState) -> Vec<DriveView> {
    let library = PathBuf::from(state.store.lock().unwrap().library_dir());
    let games: Vec<Game> = state.games.lock().unwrap().clone();
    let mut drives: Vec<DriveView> = fixed_drives()
        .into_iter()
        .filter_map(|root| {
            let (free, total) = disk_space(Path::new(&root))?;
            Some(DriveView { root, total, free, games: vec![] })
        })
        .collect();
    for game in &games {
        let Some((drive, size)) = measure(game, &library) else { continue };
        if size.size == 0 && !size.removable {
            continue;
        }
        if let Some(view) = drives.iter_mut().find(|d| d.root == drive) {
            view.games.push(size);
        }
    }
    for d in &mut drives {
        d.games.sort_by(|a, b| b.size.cmp(&a.size));
    }
    drives
}

/// Tamanho de um jogo, para a confirmação de desinstalar nos detalhes.
pub fn game_size(state: &AppState, id: &str) -> Option<GameSize> {
    let library = PathBuf::from(state.store.lock().unwrap().library_dir());
    let game = state.games.lock().unwrap().iter().find(|g| g.id == id).cloned()?;
    measure(&game, &library).map(|(_, size)| size)
}

pub fn uninstall(state: &AppState, id: &str) -> Result<(), String> {
    if state.session.lock().unwrap().is_some() {
        return Err("Feche o jogo antes de desinstalar.".into());
    }
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Aguarde a importação terminar antes de desinstalar.".into());
    }
    let library = PathBuf::from(state.store.lock().unwrap().library_dir());
    let game = state.games.lock().unwrap().iter().find(|g| g.id == id).cloned().ok_or("Jogo não encontrado.")?;
    match locate(&game, &library).ok_or("Não foi possível descobrir onde este jogo está instalado.")? {
        Location::Steam { manifest, .. } => steam::uninstall(Path::new(&manifest))?,
        Location::Epic { manifest, .. } => epic::uninstall(Path::new(&manifest))?,
        Location::Library(path) => {
            let result = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
            result.map_err(|e| format!("Não foi possível apagar {}: {e}", path.display()))?;
        }
        Location::External(_) => {
            return Err("Este jogo está fora da biblioteca do Sofa; o Sofa não apaga arquivos de outras pastas.".into())
        }
    }
    if let Some(manual_id) = id.strip_prefix("manual:") {
        let mut store = state.store.lock().unwrap();
        store.manual.retain(|m| m.id != manual_id);
        store.save().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_da_biblioteca_e_a_pasta_do_jogo() {
        let lib = Path::new(r"C:\Users\Eu\Games\Sofa");
        assert_eq!(
            library_item(Path::new(r"C:\Users\Eu\Games\Sofa\pc\Dusklight\bin\jogo.exe"), lib),
            Some(PathBuf::from(r"C:\Users\Eu\Games\Sofa\pc\Dusklight"))
        );
        assert_eq!(
            library_item(Path::new(r"c:\users\eu\games\sofa\cemu\Mario Kart 8.wua"), lib),
            Some(PathBuf::from(r"C:\Users\Eu\Games\Sofa\cemu\Mario Kart 8.wua"))
        );
    }

    #[test]
    fn nunca_apaga_fora_ou_acima_de_um_item() {
        let lib = Path::new(r"C:\Users\Eu\Games\Sofa");
        for path in [
            r"C:\Users\Eu\Desktop\jogo.exe",
            r"C:\Users\Eu\Games\Sofa",
            r"C:\Users\Eu\Games\Sofa\pc",
            r"C:\Users\Eu\Games\SofaOutra\pc\x\a.exe",
            r"C:\Users\Eu\Games\Sofa\.sofa-staging\pc\x",
            r"C:\Users\Eu\Games\Sofa\..\x\y",
        ] {
            assert_eq!(library_item(Path::new(path), lib), None, "{path}");
        }
        assert_eq!(library_item(Path::new(r"C:\x\y"), Path::new("")), None);
    }

    #[test]
    fn raiz_do_disco() {
        assert_eq!(drive_of(r"d:\SteamLibrary\steamapps").as_deref(), Some(r"D:\"));
        assert_eq!(drive_of(r"\\servidor\pasta"), None);
    }

    #[test]
    fn rom_vem_do_id() {
        let game = Game {
            id: r"emu:cemu:c:\jogos\mk8.wua".into(),
            title: "MK8".into(),
            source: "cemu".into(),
            launch: Launch::Exec { path: "cemu.exe".into(), args: vec![], cwd: String::new() },
            art: None,
            hero: None,
            manifest: None,
            update: None,
        };
        assert_eq!(rom_path(&game), Some(PathBuf::from(r"c:\jogos\mk8.wua")));
    }

    #[test]
    fn soma_o_tamanho_da_pasta() {
        let dir = std::env::temp_dir().join(format!("sofa-size-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.bin"), [0u8; 100]).unwrap();
        std::fs::write(dir.join(r"sub\b.bin"), [0u8; 50]).unwrap();
        assert_eq!(dir_size(&dir), 150);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
