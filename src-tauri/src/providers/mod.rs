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
    /// Arquivo da loja que descreve a instalação (`.acf` da Steam, `.item` da Epic), relido para
    /// acompanhar atualizações.
    pub manifest: Option<String>,
    /// Estado de atualização no momento da varredura. Só Steam e Epic têm.
    pub update: Option<UpdateState>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateState {
    /// Steam: nada pendente.
    UpToDate,
    /// Steam: há atualização, ainda não baixando.
    Pending,
    /// Steam: baixando ou instalando a atualização.
    Downloading { done: u64, total: u64 },
    /// Steam: primeira instalação, ainda sem versão jogável.
    Installing { done: u64, total: u64 },
    /// Epic: só a versão instalada é conhecida; quem verifica é o launcher dela.
    Version { version: String },
}
