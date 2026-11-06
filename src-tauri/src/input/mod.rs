//! Leitura de controles via SDL3 em uma thread própria.

pub mod logic;

use crate::overlay;
use logic::{Action, InputState, Output};
use sdl3_sys::everything::*;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    Launcher = 0,
    /// Só a combinação é tratada; o resto fica para o jogo.
    InGame = 1,
    Desktop = 2,
}

static MODE: AtomicU8 = AtomicU8::new(Mode::Launcher as u8);
static SWAP_CONFIRM: AtomicBool = AtomicBool::new(false);

pub fn set_mode(mode: Mode) {
    MODE.store(mode as u8, Ordering::Relaxed);
}

pub fn mode() -> Mode {
    match MODE.load(Ordering::Relaxed) {
        1 => Mode::InGame,
        2 => Mode::Desktop,
        _ => Mode::Launcher,
    }
}

pub fn set_swap(value: bool) {
    SWAP_CONFIRM.store(value, Ordering::Relaxed);
}

/// No modo desktop com o teclado fechado, o analógico move o cursor em vez de navegar.
fn cursor_mode() -> bool {
    mode() == Mode::Desktop && !overlay::KEYBOARD_OPEN.load(Ordering::Relaxed)
}

#[derive(Clone, Serialize)]
struct InputEvent {
    action: Action,
}

#[derive(Clone, Serialize)]
pub struct ControllerInfo {
    /// "xbox", "nintendo" ou "generic": define os ícones de botão mostrados na tela.
    pub family: &'static str,
    pub name: String,
}

const STICK_ON: i16 = 16000;
const STICK_OFF: i16 = 10000;
const CURSOR_DEADZONE: f32 = 0.18;
/// Pixels por iteração do laço (cerca de 120 por segundo) com o analógico no máximo.
const CURSOR_SPEED: f32 = 16.0;
const SCROLL_EVERY_TICKS: u32 = 8;
const WHEEL_STEP: f32 = 120.0;

/// Curva quadrática: precisão perto do centro e velocidade no fim do curso.
pub fn cursor_delta(value: i16) -> i32 {
    let v = value as f32 / 32767.0;
    if v.abs() < CURSOR_DEADZONE {
        return 0;
    }
    let scaled = (v.abs() - CURSOR_DEADZONE) / (1.0 - CURSOR_DEADZONE);
    (scaled * scaled * CURSOR_SPEED * v.signum()).round() as i32
}

/// Estado do analógico esquerdo tratado como direcional, com histerese.
#[derive(Default)]
struct Stick {
    x: Option<Action>,
    y: Option<Action>,
}

fn stick_direction(value: i16, current: Option<Action>, neg: Action, pos: Action) -> Option<Action> {
    match current {
        Some(a) if a == neg && value < -STICK_OFF => Some(a),
        Some(a) if a == pos && value > STICK_OFF => Some(a),
        _ if value <= -STICK_ON => Some(neg),
        _ if value >= STICK_ON => Some(pos),
        _ => None,
    }
}

/// Traduz o botão físico pela etiqueta: "A" confirma tanto no Xbox (posição sul)
/// quanto no Pro Controller (posição leste).
unsafe fn map_button(gamepad: *mut SDL_Gamepad, button: u8) -> Option<Action> {
    let button = SDL_GamepadButton(button as _);
    match button {
        SDL_GAMEPAD_BUTTON_DPAD_UP => return Some(Action::Up),
        SDL_GAMEPAD_BUTTON_DPAD_DOWN => return Some(Action::Down),
        SDL_GAMEPAD_BUTTON_DPAD_LEFT => return Some(Action::Left),
        SDL_GAMEPAD_BUTTON_DPAD_RIGHT => return Some(Action::Right),
        SDL_GAMEPAD_BUTTON_LEFT_SHOULDER => return Some(Action::Lb),
        SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER => return Some(Action::Rb),
        SDL_GAMEPAD_BUTTON_START => return Some(Action::Menu),
        SDL_GAMEPAD_BUTTON_BACK => return Some(Action::View),
        SDL_GAMEPAD_BUTTON_LEFT_STICK => return Some(Action::L3),
        SDL_GAMEPAD_BUTTON_RIGHT_STICK => return Some(Action::R3),
        _ => {}
    }
    let swap = SWAP_CONFIRM.load(Ordering::Relaxed);
    match SDL_GetGamepadButtonLabel(gamepad, button) {
        SDL_GAMEPAD_BUTTON_LABEL_A => Some(if swap { Action::Back } else { Action::Confirm }),
        SDL_GAMEPAD_BUTTON_LABEL_B => Some(if swap { Action::Confirm } else { Action::Back }),
        SDL_GAMEPAD_BUTTON_LABEL_X => Some(Action::X),
        SDL_GAMEPAD_BUTTON_LABEL_Y => Some(Action::Y),
        _ => None,
    }
}

unsafe fn describe(gamepad: *mut SDL_Gamepad) -> ControllerInfo {
    let family = match SDL_GetGamepadType(gamepad) {
        SDL_GAMEPAD_TYPE_XBOX360 | SDL_GAMEPAD_TYPE_XBOXONE => "xbox",
        SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_PRO
        | SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_JOYCON_LEFT
        | SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_JOYCON_RIGHT
        | SDL_GAMEPAD_TYPE_NINTENDO_SWITCH_JOYCON_PAIR => "nintendo",
        _ => "generic",
    };
    let raw = SDL_GetGamepadName(gamepad);
    let name = if raw.is_null() {
        "Controle".to_string()
    } else {
        std::ffi::CStr::from_ptr(raw).to_string_lossy().into_owned()
    };
    ControllerInfo { family, name }
}

/// Bateria do controle em uso; `None` sem controle ou quando ele não informa.
unsafe fn read_battery(pad: Option<*mut SDL_Gamepad>) -> Option<crate::battery::Battery> {
    use crate::battery::{Battery, PowerState};
    let pad = pad?;
    let mut percent: std::ffi::c_int = -1;
    let state = SDL_GetGamepadPowerInfo(pad, &mut percent);
    if let Some(battery) = crate::battery::from_sdl(state.0, percent) {
        return Some(battery);
    }
    // Controles do Xbox por Bluetooth não informam a bateria ao SDL; o Windows sabe.
    let state = match crate::battery::bluetooth_gamepad_percent() {
        Some(p) => Battery { state: PowerState::Battery, percent: Some(p) },
        None => Battery { state: PowerState::Unknown, percent: None },
    };
    Some(state)
}

const BATTERY_EVERY: Duration = Duration::from_secs(30);

fn dispatch(app: &AppHandle, outputs: Vec<Output>) {
    for output in outputs {
        match output {
            Output::Press(action) => match mode() {
                Mode::Launcher => {
                    crate::library::debug_log(&format!("controle {action:?}"));
                    let _ = app.emit_to("main", "input", InputEvent { action });
                }
                Mode::InGame => {}
                Mode::Desktop => overlay::on_action(app, action),
            },
            Output::Combo => trigger_combo(app),
        }
    }
}

/// Abre o menu do Sofa. Vem do L3 + R3 ou do atalho global de teclado.
pub fn trigger_combo(app: &AppHandle) {
    match mode() {
        Mode::Desktop => overlay::exit(app),
        Mode::InGame => {
            crate::session::show_launcher(app, true);
            crate::commands::notify_combo(app);
        }
        Mode::Launcher => crate::commands::notify_combo(app),
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("sdl-input".into())
        .spawn(move || unsafe { run(app) })
        .expect("não foi possível iniciar a thread de entrada");
}

unsafe fn run(app: AppHandle) {
    // Sem isto a combinação não seria lida com o jogo em primeiro plano.
    SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
    if !SDL_Init(SDL_INIT_GAMEPAD) {
        let err = std::ffi::CStr::from_ptr(SDL_GetError()).to_string_lossy();
        eprintln!("SDL_Init falhou: {err}");
        let _ = app.emit("input-error", err.into_owned());
        return;
    }

    let mut state = InputState::default();
    let mut pads: HashMap<SDL_JoystickID, *mut SDL_Gamepad> = HashMap::new();
    let mut stick = Stick::default();
    let mut event: SDL_Event = std::mem::zeroed();
    let mut ticks: u32 = 0;
    let mut last_pad: Option<SDL_JoystickID> = None;
    let mut battery_at: Option<Instant> = None;
    let mut battery_warned = crate::battery::NOT_WARNED;

    loop {
        let now = Instant::now();
        while SDL_PollEvent(&mut event) {
            let kind = SDL_EventType(event.r#type);
            match kind {
                SDL_EVENT_GAMEPAD_ADDED => {
                    let which = event.gdevice.which;
                    let pad = SDL_OpenGamepad(which);
                    if !pad.is_null() {
                        pads.insert(which, pad);
                        last_pad = Some(which);
                        let _ = app.emit("controller", describe(pad));
                        battery_at = None;
                    }
                }
                SDL_EVENT_GAMEPAD_REMOVED => {
                    let which = event.gdevice.which;
                    if let Some(pad) = pads.remove(&which) {
                        SDL_CloseGamepad(pad);
                    }
                    if last_pad == Some(which) {
                        last_pad = pads.keys().next().copied();
                        battery_at = None;
                    }
                    state.reset();
                    stick = Stick::default();
                }
                SDL_EVENT_GAMEPAD_BUTTON_DOWN | SDL_EVENT_GAMEPAD_BUTTON_UP => {
                    let b = event.gbutton;
                    let Some(&pad) = pads.get(&b.which) else { continue };
                    // O ícone na tela acompanha o último controle usado.
                    if kind == SDL_EVENT_GAMEPAD_BUTTON_DOWN {
                        last_pad = Some(b.which);
                        let _ = app.emit("controller", describe(pad));
                    }
                    if let Some(action) = map_button(pad, b.button) {
                        let out = if kind == SDL_EVENT_GAMEPAD_BUTTON_DOWN {
                            state.press(action, now)
                        } else {
                            state.release(action)
                        };
                        dispatch(&app, out);
                    }
                }
                SDL_EVENT_GAMEPAD_AXIS_MOTION => {
                    if cursor_mode() {
                        continue;
                    }
                    let a = event.gaxis;
                    let axis = SDL_GamepadAxis(a.axis as _);
                    let (slot, neg, pos) = match axis {
                        SDL_GAMEPAD_AXIS_LEFTX => (&mut stick.x, Action::Left, Action::Right),
                        SDL_GAMEPAD_AXIS_LEFTY => (&mut stick.y, Action::Up, Action::Down),
                        _ => continue,
                    };
                    let next = stick_direction(a.value, *slot, neg, pos);
                    if next != *slot {
                        if let Some(old) = *slot {
                            dispatch(&app, state.release(old));
                        }
                        if let Some(new) = next {
                            dispatch(&app, state.press(new, now));
                        }
                        *slot = next;
                    }
                }
                _ => {}
            }
        }
        dispatch(&app, state.tick(now));

        if battery_at.is_none_or(|at| now.duration_since(at) >= BATTERY_EVERY) {
            battery_at = Some(now);
            let mut battery = read_battery(last_pad.and_then(|id| pads.get(&id).copied()));
            // Teste do aviso sem esperar a bateria acabar: um número no arquivo %APPDATA%\Sofa\fake-battery.txt,
            // relido a cada leitura. Apagar o arquivo volta para a bateria real em até 30 s.
            let fake = std::fs::read_to_string(crate::library::data_dir().join("fake-battery.txt")).ok();
            if let Some(p) = fake.and_then(|v| v.trim().parse::<u8>().ok()) {
                battery = Some(crate::battery::Battery { state: crate::battery::PowerState::Battery, percent: Some(p.min(100)) });
            }
            crate::battery::update(&app, battery, &mut battery_warned);
        }

        ticks = ticks.wrapping_add(1);
        if cursor_mode() {
            if let Some(&pad) = last_pad.and_then(|id| pads.get(&id)) {
                let dx = cursor_delta(SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_LEFTX));
                let dy = cursor_delta(SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_LEFTY));
                if dx != 0 || dy != 0 {
                    overlay::move_cursor(dx, dy);
                }
                if ticks % SCROLL_EVERY_TICKS == 0 {
                    let wheel = SDL_GetGamepadAxis(pad, SDL_GAMEPAD_AXIS_RIGHTY) as f32 / 32767.0;
                    if wheel.abs() > 0.4 {
                        overlay::scroll((-wheel * WHEEL_STEP) as i32);
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnóstico com o controle real: `cargo test bateria_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bateria_real() {
        unsafe {
            SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
            assert!(SDL_Init(SDL_INIT_GAMEPAD));
            let mut event: SDL_Event = std::mem::zeroed();
            let mut pads = vec![];
            let start = Instant::now();
            let mut last_print = Instant::now() - Duration::from_secs(5);
            while start.elapsed() < Duration::from_secs(12) {
                while SDL_PollEvent(&mut event) {
                    let kind = SDL_EventType(event.r#type);
                    if kind == SDL_EVENT_GAMEPAD_ADDED {
                        let pad = SDL_OpenGamepad(event.gdevice.which);
                        println!("ADICIONADO {}", describe(pad).name);
                        pads.push(pad);
                    } else if kind == SDL_EVENT_JOYSTICK_BATTERY_UPDATED {
                        println!("EVENTO BATERIA estado={} pct={}", event.jbattery.state.0, event.jbattery.percent);
                    }
                }
                if last_print.elapsed() >= Duration::from_secs(2) {
                    last_print = Instant::now();
                    for &pad in &pads {
                        let mut pct = -1;
                        let state = SDL_GetGamepadPowerInfo(pad, &mut pct);
                        let joy = SDL_GetGamepadJoystick(pad);
                        let conn = SDL_GetJoystickConnectionState(joy).0;
                        println!("{} | estado={} pct={} conexao={}", describe(pad).name, state.0, pct, conn);
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }

    #[test]
    fn cursor_para_na_zona_morta() {
        assert_eq!(cursor_delta(0), 0);
        assert_eq!(cursor_delta(4000), 0);
        assert_eq!(cursor_delta(-4000), 0);
    }

    #[test]
    fn cursor_acelera_e_respeita_o_sinal() {
        assert_eq!(cursor_delta(32767), CURSOR_SPEED as i32);
        assert_eq!(cursor_delta(-32767), -(CURSOR_SPEED as i32));
        assert!(cursor_delta(16000).abs() < cursor_delta(30000).abs());
    }

    #[test]
    fn analogico_tem_histerese() {
        let up = Some(Action::Up);
        assert_eq!(stick_direction(-17000, None, Action::Up, Action::Down), up);
        assert_eq!(stick_direction(-12000, up, Action::Up, Action::Down), up);
        assert_eq!(stick_direction(-12000, None, Action::Up, Action::Down), None);
        assert_eq!(stick_direction(-5000, up, Action::Up, Action::Down), None);
    }
}
