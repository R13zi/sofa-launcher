pub mod emulator;
pub mod epic;
pub mod manual;
pub mod steam;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Launch {
    /// O launcher da loja inicia o jogo; a sessão acompanha os processos em `watch_dir`.
    Uri { uri: String, watch_dir: String },
    Exec { path: String, args: Vec<String>, cwd: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub source: String,
    pub launch: Launch,
    pub art: Option<String>,
    /// Arte grande para o fundo das telas, quando a origem tiver uma.
    pub hero: Option<String>,
}
