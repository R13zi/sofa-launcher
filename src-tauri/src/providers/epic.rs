//! Epic Games: manifestos JSON (*.item) em ProgramData.

use super::{Game, Launch};
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
    })
}

pub fn scan() -> Vec<Game> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    let dir = std::path::Path::new(&program_data).join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| parse_item(&t))
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
    fn ignora_incompletos_e_nao_jogos() {
        assert!(parse_item(&item(r#""games""#, true)).is_none());
        assert!(parse_item(&item(r#""software""#, false)).is_none());
        assert!(parse_item("nao e json").is_none());
    }
}
