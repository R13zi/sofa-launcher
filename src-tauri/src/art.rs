//! Capas: cache local e download pelo SteamGridDB.

use crate::library::data_dir;
use crate::providers::emulator::clean_title;
use crate::AppState;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const API: &str = "https://www.steamgriddb.com/api/v2";
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

fn art_dir() -> PathBuf {
    data_dir().join("art")
}

/// IDs de emulador contêm caminhos, então o nome do arquivo usa um hash estável.
pub fn file_stem(game_id: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in game_id.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn cached(game_id: &str) -> Option<String> {
    let stem = file_stem(game_id);
    ["jpg", "png", "webp"].iter().map(|ext| art_dir().join(format!("{stem}.{ext}"))).find(|p| p.exists()).map(|p| p.to_string_lossy().into_owned())
}

const HERO_SUFFIX: &str = "-hero";

/// Banner baixado: fica ao lado da capa, com outro nome.
pub fn cached_hero(game_id: &str) -> Option<String> {
    let stem = file_stem(game_id);
    ["jpg", "png", "webp"]
        .iter()
        .map(|ext| art_dir().join(format!("{stem}{HERO_SUFFIX}.{ext}")))
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Pasta da biblioteca com as capas importadas de `sofa\dump\capas`.
pub const COVERS_DIR: &str = "capas";

pub fn is_cover_image(path: &Path) -> bool {
    let ext = path.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp")
}

/// Forma de comparar nomes: ignora maiúsculas, acentos, tags como "(USA)" e símbolos.
/// O Windows não aceita `:` em nomes de arquivo, então "Batman™: Arkham" precisa casar com "Batman Arkham".
pub fn cover_key(title: &str) -> String {
    clean_title(title)
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn images_in(dir: &Path) -> Vec<PathBuf> {
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                collect(&entry.path(), out);
            } else if is_cover_image(&entry.path()) {
                out.push(entry.path());
            }
        }
    }
    let mut images = vec![];
    collect(dir, &mut images);
    images.sort();
    images
}

fn stem_is(path: &Path, names: &[&str]) -> bool {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    names.contains(&stem.as_str())
}

const BANNER_NAMES: &[&str] = &["banner", "hero"];
const COVER_NAMES: &[&str] = &["capa", "cover"];

/// Escolhe a capa de uma pasta: a chamada capa ou cover, senão a primeira em ordem alfabética.
/// Um banner nunca vira capa, nem sozinho na pasta.
pub fn best_cover_in(dir: &Path) -> Option<PathBuf> {
    let mut images: Vec<PathBuf> = images_in(dir).into_iter().filter(|p| !stem_is(p, BANNER_NAMES)).collect();
    match images.iter().position(|p| stem_is(p, COVER_NAMES)) {
        Some(index) => Some(images.swap_remove(index)),
        None => images.into_iter().next(),
    }
}

/// O banner de uma pasta: só a imagem chamada banner ou hero.
pub fn best_banner_in(dir: &Path) -> Option<PathBuf> {
    images_in(dir).into_iter().find(|p| stem_is(p, BANNER_NAMES))
}

/// Capas de `<biblioteca>\capas`, pela chave do nome. Só lê essa pasta.
pub fn imported_covers(library: &Path) -> HashMap<String, String> {
    let mut covers = HashMap::new();
    let Ok(entries) = std::fs::read_dir(library.join(COVERS_DIR)) else { return covers };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        let (name, image) = if kind.is_dir() {
            (entry.file_name().to_string_lossy().into_owned(), best_cover_in(&path))
        } else if is_cover_image(&path) {
            (path.file_stem().unwrap_or_default().to_string_lossy().into_owned(), Some(path.clone()))
        } else {
            continue;
        };
        let key = cover_key(&name);
        if let (false, Some(image)) = (key.is_empty(), image) {
            covers.insert(key, image.to_string_lossy().into_owned());
        }
    }
    covers
}

/// Banners de `<biblioteca>\capas\<jogo>`, pela chave do nome da pasta.
pub fn imported_banners(library: &Path) -> HashMap<String, String> {
    let mut banners = HashMap::new();
    let Ok(entries) = std::fs::read_dir(library.join(COVERS_DIR)) else { return banners };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() || !kind.is_dir() {
            continue;
        }
        let key = cover_key(&entry.file_name().to_string_lossy());
        if let (false, Some(image)) = (key.is_empty(), best_banner_in(&entry.path())) {
            banners.insert(key, image.to_string_lossy().into_owned());
        }
    }
    banners
}

/// Capa local do jogo: a importada por nome ou a baixada do SteamGridDB.
pub fn local_cover(game_id: &str, title: &str, covers: &HashMap<String, String>) -> Option<String> {
    covers.get(&cover_key(title)).cloned().or_else(|| cached(game_id))
}

/// Banner local do jogo: o importado por nome ou o baixado do SteamGridDB.
pub fn local_hero(game_id: &str, title: &str, banners: &HashMap<String, String>) -> Option<String> {
    banners.get(&cover_key(title)).cloned().or_else(|| cached_hero(game_id))
}

/// Capas e banners locais de uma vez, para montar a visão de vários jogos.
pub struct LocalArt {
    covers: HashMap<String, String>,
    banners: HashMap<String, String>,
}

impl LocalArt {
    pub fn load(library: &Path) -> Self {
        LocalArt { covers: imported_covers(library), banners: imported_banners(library) }
    }
    pub fn cover(&self, game_id: &str, title: &str) -> Option<String> {
        local_cover(game_id, title, &self.covers)
    }
    pub fn hero(&self, game_id: &str, title: &str) -> Option<String> {
        local_hero(game_id, title, &self.banners)
    }
}

fn get_json(url: &str, key: &str) -> Result<serde_json::Value, String> {
    ureq::get(url)
        .set("Authorization", &format!("Bearer {key}"))
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())
}

fn first_grid_url(doc: &serde_json::Value) -> Option<String> {
    doc["data"].as_array()?.first()?["url"].as_str().map(String::from)
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ArtKind {
    Cover,
    Hero,
}

impl ArtKind {
    /// Caminho da API. Capas pedem o formato vertical; banners vêm em qualquer tamanho.
    fn path(self, platform: &str, id: &str) -> String {
        match self {
            ArtKind::Cover => format!("{API}/grids/{platform}/{id}?dimensions=600x900,660x930"),
            ArtKind::Hero => format!("{API}/heroes/{platform}/{id}"),
        }
    }
    fn file_suffix(self) -> &'static str {
        match self {
            ArtKind::Cover => "",
            ArtKind::Hero => HERO_SUFFIX,
        }
    }
}

fn find_url(kind: ArtKind, game_id: &str, title: &str, key: &str) -> Result<Option<String>, String> {
    if let Some(appid) = game_id.strip_prefix("steam:") {
        if let Some(url) = first_grid_url(&get_json(&kind.path("steam", appid), key)?) {
            return Ok(Some(url));
        }
    }
    let search = get_json(&format!("{API}/search/autocomplete/{}", percent_encode(title)), key)?;
    let Some(id) = search["data"].as_array().and_then(|a| a.first()).and_then(|g| g["id"].as_u64()) else {
        return Ok(None);
    };
    Ok(first_grid_url(&get_json(&kind.path("game", &id.to_string()), key)?))
}

fn download(url: &str, game_id: &str, kind: ArtKind) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("URL de imagem insegura".into());
    }
    let lower = url.to_lowercase();
    let ext = if lower.ends_with(".png") { "png" } else if lower.ends_with(".webp") { "webp" } else { "jpg" };
    let response = ureq::get(url).timeout(std::time::Duration::from_secs(60)).call().map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    std::io::Read::read_to_end(&mut std::io::Read::take(response.into_reader(), MAX_IMAGE_BYTES), &mut bytes).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(art_dir()).map_err(|e| e.to_string())?;
    std::fs::write(art_dir().join(format!("{}{}.{ext}", file_stem(game_id), kind.file_suffix())), bytes).map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
struct ArtProgress {
    done: usize,
    total: usize,
    found: usize,
    error: Option<String>,
    finished: bool,
}

/// Baixa em segundo plano as capas e os banners que faltam, cada um só se faltar.
pub fn fetch_missing(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let Some(key) = crate::credentials::load() else {
            let _ = app.emit("art-progress", ArtProgress { done: 0, total: 0, found: 0, error: Some("Nenhuma chave configurada.".into()), finished: true });
            return;
        };
        let missing: Vec<(String, String, ArtKind)> = {
            let store = state.store.lock().unwrap();
            let local = LocalArt::load(Path::new(&store.library_dir()));
            let games = state.games.lock().unwrap();
            let mut list = vec![];
            for g in games.iter() {
                let view = store.view(g, local.cover(&g.id, &g.title), local.hero(&g.id, &g.title));
                if view.art.is_none() {
                    list.push((g.id.clone(), g.title.clone(), ArtKind::Cover));
                }
                if view.hero.is_none() {
                    list.push((g.id.clone(), g.title.clone(), ArtKind::Hero));
                }
            }
            list
        };
        let total = missing.len();
        let mut found = 0;
        let mut last_error = None;
        for (done, (id, title, kind)) in missing.into_iter().enumerate() {
            match find_url(kind, &id, &title, &key).and_then(|url| url.map(|u| download(&u, &id, kind)).transpose()) {
                Ok(Some(())) => {
                    found += 1;
                    let _ = app.emit("library-changed", ());
                }
                Ok(None) => {}
                Err(e) => last_error = Some(e),
            }
            let _ = app.emit("art-progress", ArtProgress { done: done + 1, total, found, error: last_error.clone(), finished: false });
        }
        let _ = app.emit("art-progress", ArtProgress { done: total, total, found, error: last_error, finished: true });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_library(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sofa-capas-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(COVERS_DIR)).unwrap();
        dir
    }

    fn write(path: PathBuf) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn chave_ignora_o_que_nao_cabe_em_nome_de_arquivo() {
        assert_eq!(cover_key("Batman™: Arkham Origins"), cover_key("Batman Arkham Origins"));
        assert_eq!(cover_key("LEGO® Batman™: The Videogame"), cover_key("lego batman the videogame"));
        assert_eq!(cover_key("Pokémon"), cover_key("POKEMON"));
        assert_eq!(cover_key("Zelda (USA) [v1]"), cover_key("zelda"));
        assert_ne!(cover_key("Half-Life"), cover_key("Half-Life 2"));
    }

    #[test]
    fn encontra_capas_soltas_e_em_pastas() {
        let library = temp_library("achar");
        let dir = library.join(COVERS_DIR);
        write(dir.join("Dusklight.png"));
        write(dir.join("Batman Arkham Origins").join("arte").join("a.jpg"));
        write(dir.join("Com Capa").join("a.jpg"));
        write(dir.join("Com Capa").join("capa.webp"));
        write(dir.join("Sem Imagem").join("leia.txt"));
        write(dir.join("solto.txt"));
        let covers = imported_covers(&library);
        assert!(covers[&cover_key("DUSKLIGHT")].ends_with("Dusklight.png"));
        assert!(covers[&cover_key("Batman™: Arkham Origins")].ends_with("a.jpg"));
        assert!(covers[&cover_key("Com Capa")].ends_with("capa.webp"));
        assert_eq!(covers.len(), 3);
        std::fs::remove_dir_all(library).unwrap();
    }

    #[test]
    fn capa_e_banner_ficam_cada_um_no_seu_papel() {
        let library = temp_library("papeis");
        let dir = library.join(COVERS_DIR);
        write(dir.join("Dusklight").join("capa.png"));
        write(dir.join("Dusklight").join("banner.png"));
        write(dir.join("So Banner").join("banner.jpg"));
        write(dir.join("Com Hero").join("hero.webp"));
        write(dir.join("Com Hero").join("a.jpg"));
        write(dir.join("Sem Nome").join("foto.jpg"));
        write(dir.join("Solto.png"));

        let covers = imported_covers(&library);
        let banners = imported_banners(&library);
        assert!(covers[&cover_key("Dusklight")].ends_with("capa.png"));
        assert!(banners[&cover_key("Dusklight")].ends_with("banner.png"));
        assert!(!covers.contains_key(&cover_key("So Banner")), "banner sozinho não vira capa");
        assert!(banners[&cover_key("So Banner")].ends_with("banner.jpg"));
        assert!(covers[&cover_key("Com Hero")].ends_with("a.jpg"));
        assert!(banners[&cover_key("Com Hero")].ends_with("hero.webp"));
        assert!(covers.contains_key(&cover_key("Sem Nome")));
        assert!(!banners.contains_key(&cover_key("Sem Nome")), "imagem qualquer não vira banner");
        assert!(!banners.contains_key(&cover_key("Solto")));
        std::fs::remove_dir_all(library).unwrap();
    }

    #[test]
    fn caminhos_e_nomes_de_capa_e_banner_sao_diferentes() {
        assert!(ArtKind::Cover.path("steam", "70").contains("/grids/steam/70?dimensions="));
        assert!(ArtKind::Hero.path("steam", "70").ends_with("/heroes/steam/70"));
        assert!(ArtKind::Hero.path("game", "9").ends_with("/heroes/game/9"));
        assert_eq!(ArtKind::Cover.file_suffix(), "");
        assert_eq!(ArtKind::Hero.file_suffix(), HERO_SUFFIX);
    }

    #[test]
    fn sem_pasta_de_capas_nao_ha_capas() {
        let dir = std::env::temp_dir().join(format!("sofa-sem-capas-{}", std::process::id()));
        assert!(imported_covers(&dir).is_empty());
    }

    #[test]
    fn nome_de_arquivo_e_estavel_e_seguro() {
        let a = file_stem(r"emu:cemu:d:\jogos\mario kart.wua");
        assert_eq!(a, file_stem(r"emu:cemu:d:\jogos\mario kart.wua"));
        assert_ne!(a, file_stem("steam:70"));
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn codifica_titulo_para_url() {
        assert_eq!(percent_encode("Demon's Souls/é"), "Demon%27s%20Souls%2F%C3%A9");
    }

    #[test]
    fn extrai_primeira_capa() {
        let doc = serde_json::json!({"data":[{"url":"https://x/a.png"},{"url":"https://x/b.png"}]});
        assert_eq!(first_grid_url(&doc).as_deref(), Some("https://x/a.png"));
        assert_eq!(first_grid_url(&serde_json::json!({"data":[]})), None);
    }
}
