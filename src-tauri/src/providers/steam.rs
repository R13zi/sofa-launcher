//! Steam: registro → libraryfolders.vdf → appmanifest_*.acf.

use super::{Game, Launch};
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
    // Bit 4 de StateFlags: totalmente instalado.
    let flags: u32 = state.str("StateFlags")?.parse().ok()?;
    if flags & 4 == 0 {
        return None;
    }
    Some(Manifest {
        appid: state.str("appid")?.to_string(),
        name: state.str("name")?.to_string(),
        installdir: state.str("installdir")?.to_string(),
    })
}

fn steam_root() -> Option<PathBuf> {
    let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    Some(PathBuf::from(path.replace('/', "\\")))
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

pub fn scan() -> Vec<Game> {
    let Some(root) = steam_root() else { return vec![] };
    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    let mut libraries = std::fs::read_to_string(vdf).map(|t| library_paths(&t)).unwrap_or_default();
    if libraries.is_empty() {
        libraries.push(root.to_string_lossy().into_owned());
    }

    let mut games = vec![];
    for library in libraries {
        let steamapps = Path::new(&library).join("steamapps");
        let Ok(entries) = std::fs::read_dir(&steamapps) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
            let Some(m) = parse_manifest(&text).filter(is_game) else { continue };
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
    fn ignora_download_incompleto() {
        assert!(parse_manifest(&manifest(1026, "70", "Half-Life")).is_none());
    }

    #[test]
    fn filtra_o_que_nao_e_jogo() {
        for (id, name) in [("228980", "Steamworks Common Redistributables"), ("431960", "Wallpaper Engine"), ("999", "Proton 9.0")] {
            let m = parse_manifest(&manifest(4, id, name)).unwrap();
            assert!(!is_game(&m), "{name}");
        }
        assert!(is_game(&parse_manifest(&manifest(4, "730", "Counter-Strike 2")).unwrap()));
    }
}
