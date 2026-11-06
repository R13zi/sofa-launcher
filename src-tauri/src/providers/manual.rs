//! Executáveis adicionados à mão, como o Dusklight.

use super::{Game, Launch};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManualGame {
    pub id: String,
    pub title: String,
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Pasta de trabalho, quando não for a do próprio `.exe`.
    #[serde(default)]
    pub cwd: Option<String>,
}

/// Em `args`, marca onde entra o caminho do arquivo do jogo (ISO ou pasta de assets).
pub const FILE_PLACEHOLDER: &str = "{iso}";

pub fn has_placeholder(args: &[String]) -> bool {
    args.iter().any(|a| a.contains(FILE_PLACEHOLDER))
}

/// Monta os argumentos finais. O arquivo escolhido tem prioridade sobre o encontrado pelo nome.
/// Sem `{iso}`, o escolhido vai no fim, e o automático não é usado: o jogo não pediu por ele.
/// Com `{iso}` e sem arquivo, o argumento é retirado, e o jogo abre a própria tela de seleção.
pub fn resolve_args(args: &[String], chosen: Option<&str>, auto: Option<&str>) -> Vec<String> {
    if has_placeholder(args) {
        return match chosen.or(auto) {
            Some(file) => args.iter().map(|a| a.replace(FILE_PLACEHOLDER, file)).collect(),
            None => args.iter().filter(|a| !a.contains(FILE_PLACEHOLDER)).cloned().collect(),
        };
    }
    let mut out = args.to_vec();
    out.extend(chosen.map(String::from));
    out
}

/// A pasta de trabalho é a do `.exe`: vários jogos, como o Dusklight, leem arquivos relativos a ela.
pub fn cwd_of(path: &str) -> String {
    Path::new(path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn to_game(m: &ManualGame) -> Game {
    Game {
        id: format!("manual:{}", m.id),
        title: m.title.clone(),
        source: "manual".into(),
        launch: Launch::Exec {
            path: m.path.clone(),
            args: m.args.clone(),
            cwd: m.cwd.clone().unwrap_or_else(|| cwd_of(&m.path)),
        },
        art: None,
        hero: None,
        manifest: None,
        update: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn arquivo_escolhido_vai_no_fim_sem_marcador() {
        assert_eq!(resolve_args(&strs(&["-w"]), Some(r"D:\a b.iso"), None), strs(&["-w", r"D:\a b.iso"]));
        assert_eq!(resolve_args(&[], None, None), Vec::<String>::new());
    }

    #[test]
    fn automatico_so_vale_com_marcador() {
        assert_eq!(resolve_args(&strs(&["-w"]), None, Some(r"D:\auto.iso")), strs(&["-w"]));
        assert_eq!(resolve_args(&strs(&["{iso}"]), None, Some(r"D:\auto.iso")), strs(&[r"D:\auto.iso"]));
    }

    #[test]
    fn marcador_prefere_o_escolhido_e_aceita_prefixo() {
        assert_eq!(
            resolve_args(&strs(&["--dvd={iso}", "-f"]), Some(r"D:\meu.iso"), Some(r"D:\auto.iso")),
            strs(&[r"--dvd=D:\meu.iso", "-f"])
        );
    }

    #[test]
    fn marcador_sem_arquivo_sai_dos_argumentos() {
        assert_eq!(resolve_args(&strs(&["-f", "{iso}"]), None, None), strs(&["-f"]));
    }

    #[test]
    fn pasta_de_trabalho_e_a_do_exe() {
        assert_eq!(cwd_of(r"C:\Jogos\Dusklight\dusklight.exe"), r"C:\Jogos\Dusklight");
    }
}
