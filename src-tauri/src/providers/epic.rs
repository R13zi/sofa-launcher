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
}

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
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    let dir = std::path::Path::new(&program_data).join(r"Epic\EpicGamesLauncher\Data\Manifests");
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
    fn ignora_incompletos_e_nao_jogos() {
        assert!(parse_item(&item(r#""games""#, true)).is_none());
        assert!(parse_item(&item(r#""software""#, false)).is_none());
        assert!(parse_item("nao e json").is_none());
    }
}
