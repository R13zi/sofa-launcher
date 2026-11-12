//! Vigia os downloads da Steam e da Epic em segundo plano e avisa quando um termina.

use crate::providers::UpdateState;
use crate::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_secs(5);

/// Um download que aparece na loja: `active` quando está baixando (não na fila nem pausado).
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub id: String,
    pub title: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Done {
    pub id: String,
    pub title: String,
}

fn current() -> Vec<Seen> {
    let mut out: Vec<Seen> = crate::providers::steam::downloads()
        .into_iter()
        .map(|(_, m)| Seen {
            id: format!("steam:{}", m.appid),
            active: matches!(m.update, UpdateState::Downloading { .. } | UpdateState::Installing { .. }),
            title: m.name,
        })
        .collect();
    out.extend(crate::providers::epic::downloads().into_iter().map(|d| Seen {
        id: format!("epic:{}", d.app_name),
        title: d.title,
        active: true,
    }));
    out
}

/// Baixando antes e fora da lista agora. Pausar ou voltar para a fila não conta: continua na lista.
pub fn vanished(previous: &[Seen], now: &[Seen]) -> Vec<Done> {
    previous
        .iter()
        .filter(|p| p.active && !now.iter().any(|n| n.id == p.id))
        .map(|p| Done { id: p.id.clone(), title: p.title.clone() })
        .collect()
}

pub fn spawn(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut previous = current();
        loop {
            std::thread::sleep(POLL);
            let now = current();
            let gone = vanished(&previous, &now);
            previous = now;
            if gone.is_empty() {
                continue;
            }
            crate::commands::rescan(&state);
            let _ = app.emit("library-changed", ());
            // Sumiu da lista e não está na biblioteca: foi cancelado ou desinstalado, não concluído.
            let installed: Vec<Done> = {
                let games = state.games.lock().unwrap();
                gone.into_iter().filter(|d| games.iter().any(|g| g.id == d.id)).collect()
            };
            for done in installed {
                crate::library::debug_log(&format!("download concluído: {}", done.id));
                notify(&app, done);
            }
        }
    });
}

fn notify(app: &AppHandle, done: Done) {
    crate::battery::play_chime(true);
    let _ = app.emit("download-done", &done);
    if crate::input::mode() != crate::input::Mode::Launcher {
        // Com jogo aberto ou no modo desktop, o launcher não está à vista.
        let app = app.clone();
        let hash = format!("done-{}", url_encode(&done.title));
        std::thread::spawn(move || crate::battery::show_toast(&app, &hash));
    }
}

/// Para levar o nome do jogo no endereço da janela do aviso.
fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(id: &str, active: bool) -> Seen {
        Seen { id: id.into(), title: format!("Jogo {id}"), active }
    }

    #[test]
    fn concluido_e_o_que_baixava_e_sumiu() {
        let before = [seen("steam:1", true), seen("steam:2", false), seen("epic:a", true)];
        let after = [seen("epic:a", true)];
        assert_eq!(vanished(&before, &after), vec![Done { id: "steam:1".into(), title: "Jogo steam:1".into() }]);
    }

    #[test]
    fn pausar_nao_conta_como_concluido() {
        assert!(vanished(&[seen("steam:1", true)], &[seen("steam:1", false)]).is_empty());
    }

    #[test]
    fn codifica_o_nome_para_o_endereco() {
        assert_eq!(url_encode("Ação: 2"), "A%C3%A7%C3%A3o%3A%202");
    }
}
