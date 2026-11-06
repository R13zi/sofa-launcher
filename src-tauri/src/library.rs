//! Configuração e dados do usuário em %APPDATA%\Sofa, com gravação atômica.

use crate::providers::{self, emulator::Emulator, manual::ManualGame, Game};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub favorite: bool,
    pub hidden: bool,
    pub last_played: u64,
    pub playtime_secs: u64,
    pub art: Option<String>,
    /// Banner escolhido nos detalhes do jogo.
    pub hero: Option<String>,
    /// Arquivo passado ao jogo ao iniciar, como a ISO de um decomp.
    pub launch_file: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub initialized: bool,
    /// O início automático já foi ligado uma vez; a partir daí vale a escolha do usuário.
    pub autostart_defaulted: bool,
    /// Desliga as transições entre telas.
    pub reduce_animations: bool,
    /// Para onde os jogos importados são copiados. Vazio usa o padrão.
    pub library_dir: Option<String>,
    pub swap_confirm: bool,
    pub emulators: Vec<Emulator>,
    pub manual: Vec<ManualGame>,
    pub meta: HashMap<String, Meta>,
}

#[derive(Serialize)]
pub struct GameView {
    pub id: String,
    pub title: String,
    pub source: String,
    pub art: Option<String>,
    pub hero: Option<String>,
    /// O banner escolhido nos detalhes, para oferecer "Remover" só quando houver um.
    pub custom_hero: Option<String>,
    pub favorite: bool,
    pub hidden: bool,
    pub last_played: u64,
    pub playtime_secs: u64,
    pub launch_file: Option<String>,
    /// Estado de atualização da Steam ou versão da Epic, lido na última varredura.
    pub update: Option<crate::providers::UpdateState>,
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    Path::new(&base).join("Sofa")
}

/// Diagnóstico opcional: com a variável SOFA_DEBUG definida, registra eventos em sofa.log.
pub fn debug_log(message: &str) {
    if std::env::var_os("SOFA_DEBUG").is_none() {
        return;
    }
    use std::io::Write;
    let _ = std::fs::create_dir_all(data_dir());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(data_dir().join("sofa.log")) {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let _ = writeln!(file, "{millis} {message}");
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Store {
    fn path() -> PathBuf {
        data_dir().join("data.json")
    }

    pub fn load() -> Store {
        let mut store = load_from(&Self::path());
        if !store.initialized {
            store.detect_defaults();
            store.initialized = true;
            let _ = store.save();
        }
        store
    }

    pub fn library_dir(&self) -> String {
        self.library_dir.clone().unwrap_or_else(|| {
            let profile = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
            Path::new(&profile).join("Games").join("Sofa").to_string_lossy().into_owned()
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        save_to(self, &Self::path())
    }

    /// Na primeira execução, procura emuladores e o Dusklight nas pastas pessoais.
    fn detect_defaults(&mut self) {
        let Ok(profile) = std::env::var("USERPROFILE") else { return };
        let mut found: HashMap<String, String> = HashMap::new();
        for sub in ["Documents", "Desktop", "Downloads", "Games"] {
            find_exes(&Path::new(&profile).join(sub), 4, &mut found);
        }
        for (exe, kind) in [("cemu.exe", "cemu"), ("dolphin.exe", "dolphin"), ("rpcs3.exe", "rpcs3")] {
            if let Some(path) = found.get(exe) {
                let mut emu = providers::emulator::preset(kind, path);
                emu.rom_dirs = default_rom_dirs(path);
                self.emulators.push(emu);
            }
        }
        if let Some(path) = found.get("dusklight.exe") {
            self.manual.push(ManualGame { id: "dusklight".into(), title: "Dusklight".into(), path: path.clone(), args: vec![], cwd: None });
        }
    }

    /// Banner: o escolhido, depois o importado ou baixado (`local_hero`), depois o da Steam.
    pub fn view(&self, game: &Game, cached_art: Option<String>, local_hero: Option<String>) -> GameView {
        let meta = self.meta.get(&game.id).cloned().unwrap_or_default();
        GameView {
            id: game.id.clone(),
            title: game.title.clone(),
            source: game.source.clone(),
            art: meta.art.or(cached_art).or_else(|| game.art.clone()),
            hero: meta.hero.or(local_hero).or_else(|| game.hero.clone()),
            custom_hero: self.meta.get(&game.id).and_then(|m| m.hero.clone()),
            favorite: meta.favorite,
            hidden: meta.hidden,
            last_played: meta.last_played,
            playtime_secs: meta.playtime_secs,
            launch_file: meta.launch_file,
            update: game.update.clone(),
        }
    }
}

const KNOWN_EXES: &[&str] = &["cemu.exe", "dolphin.exe", "rpcs3.exe", "dusklight.exe"];

/// Pastas de jogos com nomes comuns ao lado do emulador.
fn default_rom_dirs(exe: &str) -> Vec<String> {
    let Some(parent) = Path::new(exe).parent() else { return vec![] };
    ["jogos", "games", "roms"]
        .iter()
        .map(|name| parent.join(name))
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

fn find_exes(dir: &Path, depth: u8, found: &mut HashMap<String, String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            if depth > 0 {
                find_exes(&path, depth - 1, found);
            }
        } else {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if KNOWN_EXES.contains(&name.as_str()) {
                found.entry(name).or_insert_with(|| path.to_string_lossy().into_owned());
            }
        }
    }
}

fn load_from(path: &Path) -> Store {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Grava em um arquivo temporário e renomeia, para uma queda não corromper os dados.
fn save_to(store: &Store, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(store)?)?;
    std::fs::rename(tmp, path)
}

pub fn scan_all(store: &Store) -> Vec<Game> {
    let mut games = providers::steam::scan();
    games.extend(providers::epic::scan());
    for emu in &store.emulators {
        games.extend(providers::emulator::scan(emu));
    }
    games.extend(store.manual.iter().map(providers::manual::to_game));
    games.sort_by_key(|g| g.title.to_lowercase());
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_e_le_de_volta() {
        let dir = std::env::temp_dir().join(format!("sofa-store-{}", std::process::id()));
        let path = dir.join("data.json");
        let mut store = Store::default();
        store.meta.insert("steam:70".into(), Meta { favorite: true, playtime_secs: 90, ..Default::default() });
        save_to(&store, &path).unwrap();
        let back = load_from(&path);
        assert!(back.meta["steam:70"].favorite);
        assert_eq!(back.meta["steam:70"].playtime_secs, 90);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Diagnóstico com os dados reais da máquina; não grava nada.
    /// `cargo test varredura_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn varredura_real() {
        let mut store = Store::default();
        store.detect_defaults();
        for emu in &store.emulators {
            println!("EMULADOR {} => {} | pastas: {:?}", emu.name, emu.exe, emu.rom_dirs);
        }
        for game in scan_all(&store) {
            println!("[{}] {} | capa: {} | {:?}", game.source, game.title, game.art.is_some(), game.launch);
        }
    }

    #[test]
    fn prioridade_do_banner() {
        let game = Game {
            id: "steam:70".into(),
            title: "Half-Life".into(),
            source: "steam".into(),
            launch: crate::providers::Launch::Uri { uri: String::new(), watch_dir: String::new() },
            art: Some("capa-steam.jpg".into()),
            hero: Some("steam.jpg".into()),
            manifest: None,
            update: None,
        };
        let mut store = Store::default();
        assert_eq!(store.view(&game, None, None).hero.as_deref(), Some("steam.jpg"));
        assert_eq!(store.view(&game, None, Some("local.png".into())).hero.as_deref(), Some("local.png"));
        store.meta.insert(game.id.clone(), Meta { hero: Some("meu.png".into()), ..Default::default() });
        let view = store.view(&game, None, Some("local.png".into()));
        assert_eq!(view.hero.as_deref(), Some("meu.png"));
        assert_eq!(view.custom_hero.as_deref(), Some("meu.png"));

        let sem_banner = Game { hero: None, ..game };
        let store = Store::default();
        let view = store.view(&sem_banner, None, None);
        assert_eq!(view.hero, None, "sem banner, a interface usa a capa desfocada");
        assert_eq!(view.art.as_deref(), Some("capa-steam.jpg"));
    }

    #[test]
    fn arquivo_corrompido_nao_derruba() {
        let dir = std::env::temp_dir().join(format!("sofa-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.json");
        std::fs::write(&path, "{ nao e json").unwrap();
        assert!(load_from(&path).emulators.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
