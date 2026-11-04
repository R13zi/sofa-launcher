//! Lógica de entrada sem dependência de SDL, para poder ser testada sem controle físico.

use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    X,
    Y,
    Lb,
    Rb,
    Menu,
    View,
    /// Cliques dos analógicos: só servem para a combinação e nunca chegam à interface.
    L3,
    R3,
}

impl Action {
    fn repeats(self) -> bool {
        matches!(self, Action::Up | Action::Down | Action::Left | Action::Right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Press(Action),
    Combo,
}

const REPEAT_DELAY: Duration = Duration::from_millis(350);
const REPEAT_RATE: Duration = Duration::from_millis(90);
pub const COMBO_HOLD: Duration = Duration::from_millis(1500);

/// Transforma pressionar/soltar em ações, com repetição de direcionais e a combinação
/// L3 + R3 segurados.
#[derive(Default)]
pub struct InputState {
    repeat: Option<(Action, Instant)>,
    l3: bool,
    r3: bool,
    combo_since: Option<Instant>,
    combo_fired: bool,
}

impl InputState {
    pub fn press(&mut self, action: Action, now: Instant) -> Vec<Output> {
        match action {
            Action::L3 | Action::R3 => {
                if action == Action::L3 {
                    self.l3 = true;
                } else {
                    self.r3 = true;
                }
                if self.l3 && self.r3 {
                    self.combo_since = Some(now);
                }
                vec![]
            }
            _ => {
                if action.repeats() {
                    self.repeat = Some((action, now + REPEAT_DELAY));
                }
                vec![Output::Press(action)]
            }
        }
    }

    pub fn release(&mut self, action: Action) -> Vec<Output> {
        match action {
            Action::L3 | Action::R3 => {
                if action == Action::L3 {
                    self.l3 = false;
                } else {
                    self.r3 = false;
                }
                self.combo_since = None;
                self.combo_fired = false;
            }
            _ => {
                if matches!(self.repeat, Some((held, _)) if held == action) {
                    self.repeat = None;
                }
            }
        }
        vec![]
    }

    pub fn tick(&mut self, now: Instant) -> Vec<Output> {
        let mut out = vec![];
        if let Some((action, next)) = self.repeat {
            if now >= next {
                self.repeat = Some((action, now + REPEAT_RATE));
                out.push(Output::Press(action));
            }
        }
        if let Some(since) = self.combo_since {
            if !self.combo_fired && now.duration_since(since) >= COMBO_HOLD {
                self.combo_fired = true;
                out.push(Output::Combo);
            }
        }
        out
    }

    /// Esquece o que está pressionado (ex.: controle desconectado no meio de uma repetição).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direcional_repete_depois_do_atraso() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        assert_eq!(s.press(Action::Down, t0), vec![Output::Press(Action::Down)]);
        assert!(s.tick(t0 + Duration::from_millis(100)).is_empty());
        assert_eq!(s.tick(t0 + REPEAT_DELAY), vec![Output::Press(Action::Down)]);
        s.release(Action::Down);
        assert!(s.tick(t0 + Duration::from_secs(5)).is_empty());
    }

    #[test]
    fn confirmar_nao_repete() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        s.press(Action::Confirm, t0);
        assert!(s.tick(t0 + Duration::from_secs(5)).is_empty());
    }

    #[test]
    fn start_e_select_disparam_ao_apertar_e_nao_formam_combinacao() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        assert_eq!(s.press(Action::Menu, t0), vec![Output::Press(Action::Menu)]);
        assert_eq!(s.press(Action::View, t0), vec![Output::Press(Action::View)]);
        assert!(s.tick(t0 + COMBO_HOLD * 2).is_empty());
        assert!(s.release(Action::Menu).is_empty());
        assert!(s.release(Action::View).is_empty());
    }

    #[test]
    fn combinacao_dispara_uma_vez_e_nao_vaza_toques() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        assert!(s.press(Action::L3, t0).is_empty());
        assert!(s.press(Action::R3, t0).is_empty());
        assert!(s.tick(t0 + Duration::from_millis(1000)).is_empty());
        assert_eq!(s.tick(t0 + COMBO_HOLD), vec![Output::Combo]);
        assert!(s.tick(t0 + COMBO_HOLD * 2).is_empty());
        assert!(s.release(Action::L3).is_empty());
        assert!(s.release(Action::R3).is_empty());
    }

    #[test]
    fn combinacao_volta_a_funcionar_depois_de_soltar() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        s.press(Action::L3, t0);
        s.press(Action::R3, t0);
        assert_eq!(s.tick(t0 + COMBO_HOLD), vec![Output::Combo]);
        s.release(Action::L3);
        s.release(Action::R3);
        let t1 = t0 + COMBO_HOLD * 2;
        s.press(Action::L3, t1);
        s.press(Action::R3, t1);
        assert_eq!(s.tick(t1 + COMBO_HOLD), vec![Output::Combo]);
    }

    #[test]
    fn soltar_antes_do_tempo_cancela_a_combinacao() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        s.press(Action::L3, t0);
        s.press(Action::R3, t0);
        s.release(Action::R3);
        assert!(s.tick(t0 + COMBO_HOLD * 2).is_empty());
    }

    #[test]
    fn um_analogico_sozinho_nao_dispara() {
        let t0 = Instant::now();
        let mut s = InputState::default();
        s.press(Action::L3, t0);
        assert!(s.tick(t0 + COMBO_HOLD * 2).is_empty());
    }
}
