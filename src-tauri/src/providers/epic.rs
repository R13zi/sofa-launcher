//! Epic Games: manifestos JSON (*.item) em ProgramData.

use super::{Game, Launch, UpdateState};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Item {
    display_name: String,
    install_location: String,
    catalog_namespace: String,
    catalog_item_id: String,
    app_name: String,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(default, rename = "bIsIncompleteInstall")]
    incomplete: bool,
    #[serde(default)]
    app_version_string: String,
    #[serde(default)]
    install_size: u64,
}

fn program_data() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()))
}

/// Tamanho instalado lido do `.item`.
pub fn read_size(manifest: &str) -> Option<u64> {
    let item: Item = serde_json::from_str(&std::fs::read_to_string(manifest).ok()?).ok()?;
    Some(item.install_size)
}

// ---- Desinstalar ----

const LAUNCHER_EXE: &str = "EpicGamesLauncher.exe";

/// Tira um jogo da lista de instalados do launcher, mantendo o resto do arquivo como está.
pub fn remove_installed_entry(text: &str, app_name: &str) -> Option<String> {
    let mut doc: serde_json::Value = serde_json::from_str(text).ok()?;
    let list = doc.get_mut("InstallationList")?.as_array_mut()?;
    list.retain(|e| e.get("AppName").and_then(|v| v.as_str()) != Some(app_name));
    serde_json::to_string_pretty(&doc).ok()
}

/// Pasta segura para apagar: absoluta e pelo menos dois níveis abaixo da raiz do disco.
pub fn safe_install_dir(path: &str) -> bool {
    let p = std::path::Path::new(path);
    p.is_absolute() && p.components().filter(|c| matches!(c, std::path::Component::Normal(_))).count() >= 2
}

/// Apaga o jogo, o `.item` e a entrada na lista do launcher. A Epic fica fechada durante a remoção.
pub fn uninstall(manifest_path: &std::path::Path) -> Result<(), String> {
    let item: Item = serde_json::from_str(&std::fs::read_to_string(manifest_path).map_err(|e| e.to_string())?)
        .map_err(|_| "Manifesto da Epic inválido.".to_string())?;
    if !safe_install_dir(&item.install_location) {
        return Err(format!("Pasta de instalação inesperada: {}", item.install_location));
    }
    let launcher = crate::session::process_path(LAUNCHER_EXE);
    if launcher.is_some() {
        crate::session::kill_processes(LAUNCHER_EXE);
        crate::session::wait_exit(LAUNCHER_EXE, std::time::Duration::from_secs(15));
    }
    let result = (|| {
        let dir = std::path::Path::new(&item.install_location);
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| format!("Não foi possível apagar {}: {e}", dir.display()))?;
        }
        std::fs::remove_file(manifest_path).map_err(|e| format!("Não foi possível apagar o manifesto: {e}"))?;
        let dat = program_data().join(r"Epic\UnrealEngineLauncher\LauncherInstalled.dat");
        if let Ok(text) = std::fs::read_to_string(&dat) {
            if let Some(updated) = remove_installed_entry(&text, &item.app_name) {
                std::fs::write(&dat, updated).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })();
    if let Some(exe) = launcher {
        let _ = std::process::Command::new(exe).arg("-silent").spawn();
    }
    result
}

// ---- Jogos da conta (cache local do launcher) ----

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Owned {
    pub app_name: String,
    pub title: String,
    pub namespace: String,
    pub item_id: String,
    pub art: Option<String>,
}

/// Lê o `catcache.bin` (JSON em base64): só jogos para Windows, sem DLCs.
pub fn parse_catalog(base64_text: &str) -> Vec<Owned> {
    let Some(bytes) = decode_base64(base64_text.trim()) else { return vec![] };
    let Ok(items) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) else { return vec![] };
    let mut out: Vec<Owned> = items
        .iter()
        .filter_map(|item| {
            let paths: Vec<&str> =
                item.get("categories")?.as_array()?.iter().filter_map(|c| c.get("path")?.as_str()).collect();
            if !paths.contains(&"games") || paths.iter().any(|p| p.contains("addons")) {
                return None;
            }
            let dlc = item.get("mainGameItem").and_then(|m| m.get("id")).and_then(|v| v.as_str()).is_some_and(|id| !id.is_empty());
            if dlc {
                return None;
            }
            let release = item.get("releaseInfo")?.as_array()?.iter().find(|r| {
                r.get("platform").and_then(|p| p.as_array()).is_none_or(|p| p.iter().any(|x| x.as_str() == Some("Windows")))
            })?;
            let image = |kind: &str| {
                item.get("keyImages")?.as_array()?.iter().find(|i| i.get("type").and_then(|t| t.as_str()) == Some(kind))?
                    .get("url")?.as_str().map(String::from)
            };
            Some(Owned {
                app_name: release.get("appId")?.as_str()?.to_string(),
                title: item.get("title")?.as_str()?.to_string(),
                namespace: item.get("namespace")?.as_str()?.to_string(),
                item_id: item.get("id")?.as_str()?.to_string(),
                art: image("DieselGameBoxTall").or_else(|| image("DieselGameBox")),
            })
        })
        .collect();
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out.dedup_by(|a, b| a.app_name == b.app_name);
    out
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for &c in text.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

pub fn owned_games() -> Vec<Owned> {
    let path = program_data().join(r"Epic\EpicGamesLauncher\Data\Catalog\catcache.bin");
    std::fs::read_to_string(path).map(|t| parse_catalog(&t)).unwrap_or_default()
}

/// Abre a janela de instalação da Epic para o jogo.
pub fn install_uri(owned: &Owned) -> String {
    format!("com.epicgames.launcher://apps/{}%3A{}%3A{}?action=install", owned.namespace, owned.item_id, owned.app_name)
}

pub const LIBRARY_URI: &str = "com.epicgames.launcher://store/library";

fn version_state(version: &str) -> Option<UpdateState> {
    (!version.is_empty()).then(|| UpdateState::Version { version: version.to_string() })
}

/// Relê o manifesto: a versão muda quando a Epic termina uma atualização.
pub fn read_update(manifest: &str) -> Option<UpdateState> {
    let item: Item = serde_json::from_str(&std::fs::read_to_string(manifest).ok()?).ok()?;
    version_state(&item.app_version_string)
}

/// Pede ao launcher da Epic que procure atualizações do jogo, sem abrir o jogo.
pub fn update_check_uri(launch_uri: &str) -> String {
    launch_uri.replace("action=launch", "action=updatecheck")
}

pub fn parse_item(text: &str) -> Option<Game> {
    let item: Item = serde_json::from_str(text).ok()?;
    let is_game = item.app_categories.is_empty() || item.app_categories.iter().any(|c| c == "games");
    if item.incomplete || !is_game {
        return None;
    }
    Some(Game {
        id: format!("epic:{}", item.app_name),
        title: item.display_name,
        source: "epic".into(),
        launch: Launch::Uri {
            uri: format!(
                "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
                item.catalog_namespace, item.catalog_item_id, item.app_name
            ),
            watch_dir: item.install_location,
        },
        art: None,
        hero: None,
        manifest: None,
        update: version_state(&item.app_version_string),
    })
}

pub fn scan() -> Vec<Game> {
    let dir = program_data().join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| {
            let mut game = parse_item(&std::fs::read_to_string(e.path()).ok()?)?;
            game.manifest = Some(e.path().to_string_lossy().into_owned());
            Some(game)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(categories: &str, incomplete: bool) -> String {
        format!(
            r#"{{"DisplayName":"Jogo Exemplo","InstallLocation":"C:\\Epic\\Jogo","CatalogNamespace":"ns1","CatalogItemId":"item1","AppName":"App1","AppCategories":[{categories}],"bIsIncompleteInstall":{incomplete},"LaunchExecutable":"Jogo.exe"}}"#
        )
    }

    #[test]
    fn monta_a_uri_de_inicio() {
        let g = parse_item(&item(r#""public","games""#, false)).unwrap();
        assert_eq!(g.id, "epic:App1");
        match g.launch {
            Launch::Uri { uri, watch_dir } => {
                assert_eq!(uri, "com.epicgames.launcher://apps/ns1%3Aitem1%3AApp1?action=launch&silent=true");
                assert_eq!(watch_dir, r"C:\Epic\Jogo");
            }
            _ => panic!("esperava Uri"),
        }
    }

    #[test]
    fn le_a_versao_e_monta_a_verificacao() {
        let text = item(r#""games""#, false).replace(r#""LaunchExecutable""#, r#""AppVersionString":"1.2.3","LaunchExecutable""#);
        let g = parse_item(&text).unwrap();
        assert_eq!(g.update, Some(UpdateState::Version { version: "1.2.3".into() }));
        assert_eq!(parse_item(&item(r#""games""#, false)).unwrap().update, None);
        assert_eq!(
            update_check_uri("com.epicgames.launcher://apps/ns1%3Aitem1%3AApp1?action=launch&silent=true"),
            "com.epicgames.launcher://apps/ns1%3Aitem1%3AApp1?action=updatecheck&silent=true"
        );
    }

    #[test]
    fn remove_so_o_jogo_da_lista_de_instalados() {
        let text = r#"{"InstallationList":[{"InstallLocation":"C:\\A","AppName":"App1"},{"InstallLocation":"D:\\B","AppName":"App2"}],"Outro":1}"#;
        let out: serde_json::Value = serde_json::from_str(&remove_installed_entry(text, "App1").unwrap()).unwrap();
        let list = out["InstallationList"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["AppName"], "App2");
        assert_eq!(out["Outro"], 1);
        assert_eq!(remove_installed_entry("nao e json", "App1"), None);
    }

    #[test]
    fn so_apaga_pastas_seguras() {
        assert!(safe_install_dir(r"C:\Program Files\Epic Games\Control"));
        assert!(safe_install_dir(r"D:\Jogos\Control"));
        for bad in [r"C:\", r"D:\Jogos", "", r"Jogos\Control"] {
            assert!(!safe_install_dir(bad), "{bad}");
        }
    }

    #[test]
    fn le_jogos_do_cache_do_launcher() {
        let catalog = serde_json::json!([
            {"id": "cat1", "namespace": "calluna", "title": "Control", "categories": [{"path": "games"}, {"path": "applications"}],
             "mainGameItem": {"namespace": "", "id": ""},
             "releaseInfo": [{"appId": "Calluna", "platform": ["Windows"]}],
             "keyImages": [{"type": "DieselGameBox", "url": "https://cdn1.epicgames.com/wide.jpg"}, {"type": "DieselGameBoxTall", "url": "https://cdn1.epicgames.com/tall.jpg"}]},
            {"id": "dlc1", "namespace": "calluna", "title": "Control DLC", "categories": [{"path": "games"}],
             "mainGameItem": {"namespace": "calluna", "id": "cat1"}, "releaseInfo": [{"appId": "CallunaDlc"}]},
            {"id": "ue", "namespace": "ue", "title": "Unreal Engine", "categories": [{"path": "engines"}], "releaseInfo": [{"appId": "UE_5.4"}]},
            {"id": "mac", "namespace": "m", "title": "Só Mac", "categories": [{"path": "games"}], "releaseInfo": [{"appId": "Mac", "platform": ["Mac"]}]}
        ]);
        let encoded = encode_base64(catalog.to_string().as_bytes());
        let owned = parse_catalog(&encoded);
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].app_name, "Calluna");
        assert_eq!(owned[0].art.as_deref(), Some("https://cdn1.epicgames.com/tall.jpg"));
        assert_eq!(install_uri(&owned[0]), "com.epicgames.launcher://apps/calluna%3Acat1%3ACalluna?action=install");
        assert!(parse_catalog("!!!").is_empty());
    }

    fn encode_base64(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |acc, (i, &b)| acc | (b as u32) << (16 - 8 * i));
            for i in 0..4 {
                out.push(if i <= chunk.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
            }
        }
        out
    }

    #[test]
    fn ignora_incompletos_e_nao_jogos() {
        assert!(parse_item(&item(r#""games""#, true)).is_none());
        assert!(parse_item(&item(r#""software""#, false)).is_none());
        assert!(parse_item("nao e json").is_none());
    }
}
