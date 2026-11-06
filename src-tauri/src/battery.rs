//! Bateria do controle: estado mostrado nos menus e aviso de bateria baixa.

use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerState {
    Battery,
    Charging,
    Full,
    Wired,
    /// Controle conectado, mas sem leitura de bateria.
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Battery {
    pub state: PowerState,
    /// De 0 a 100; ausente quando o controle não informa.
    pub percent: Option<u8>,
}

/// Último valor lido, para os menus mostrarem ao abrir.
pub static CURRENT: Mutex<Option<Battery>> = Mutex::new(None);

/// Limiares do aviso, do mais grave para o menos.
const THRESHOLDS: [u8; 2] = [10, 20];
/// Acima disto o aviso é rearmado.
const REARM_ABOVE: u8 = 25;
/// Sem aviso dado.
pub const NOT_WARNED: u8 = 100;

/// Traduz o estado do SDL: 1 = na bateria, 2 = sem bateria (cabo), 3 = carregando, 4 = carregado.
pub fn from_sdl(state: i32, percent: i32) -> Option<Battery> {
    let state = match state {
        1 => PowerState::Battery,
        2 => PowerState::Wired,
        3 => PowerState::Charging,
        4 => PowerState::Full,
        _ => return None,
    };
    let percent = (0..=100).contains(&percent).then_some(percent as u8);
    Some(Battery { state, percent })
}

/// Nomes de controles, para separar dos fones e teclados Bluetooth.
pub fn looks_like_gamepad(name: &str) -> bool {
    let name = name.to_lowercase();
    ["controller", "gamepad", "dualsense", "dualshock", "joy-con", "xbox"].iter().any(|k| name.contains(k))
}

/// Bateria de um controle Bluetooth, lida das propriedades do dispositivo no Windows (a mesma
/// porcentagem que aparece em Configurações → Bluetooth). Serve para controles, como os do Xbox,
/// em que o SDL não informa a bateria.
pub fn bluetooth_gamepad_percent() -> Option<u8> {
    use windows::core::GUID;
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::Devices::Properties::{DEVPKEY_Device_FriendlyName, DEVPROPTYPE};
    use windows::Win32::Foundation::DEVPROPKEY;

    const BATTERY: DEVPROPKEY = DEVPROPKEY { fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5), pid: 2 };
    unsafe {
        let set = SetupDiGetClassDevsW(Some(&GUID_DEVCLASS_BLUETOOTH), None, None, DIGCF_PRESENT).ok()?;
        let mut found = None;
        let mut index = 0;
        loop {
            let mut data = SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
            if SetupDiEnumDeviceInfo(set, index, &mut data).is_err() {
                break;
            }
            index += 1;
            let mut kind = DEVPROPTYPE::default();
            let mut name_buf = [0u8; 512];
            let mut size = 0u32;
            if SetupDiGetDevicePropertyW(set, &data, &DEVPKEY_Device_FriendlyName, &mut kind, Some(&mut name_buf), Some(&mut size), 0).is_err() {
                continue;
            }
            let units: Vec<u16> = name_buf[..size as usize].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let name = String::from_utf16_lossy(&units).trim_end_matches('\0').to_string();
            if !looks_like_gamepad(&name) {
                continue;
            }
            let mut value = [0u8; 1];
            if SetupDiGetDevicePropertyW(set, &data, &BATTERY, &mut kind, Some(&mut value), None, 0).is_ok() && value[0] <= 100 {
                found = Some(value[0]);
                break;
            }
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
        found
    }
}

/// Decide se avisa agora. `warned` guarda o limiar já avisado, para cada um aparecer uma vez só.
pub fn next_alert(warned: u8, battery: Option<Battery>) -> (Option<u8>, u8) {
    let Some(Battery { state: PowerState::Battery, percent: Some(percent) }) = battery else {
        return (None, NOT_WARNED);
    };
    if percent > REARM_ABOVE {
        return (None, NOT_WARNED);
    }
    for t in THRESHOLDS {
        if percent <= t {
            return if warned > t { (Some(t), t) } else { (None, warned) };
        }
    }
    (None, warned)
}

/// Guarda e envia o valor novo quando ele muda; avisa quando a bateria fica baixa.
pub fn update(app: &AppHandle, battery: Option<Battery>, warned: &mut u8) {
    let changed = {
        let mut current = CURRENT.lock().unwrap();
        let changed = *current != battery;
        *current = battery;
        changed
    };
    if changed {
        let _ = app.emit("battery", battery);
    }
    let (alert, next) = next_alert(*warned, battery);
    *warned = next;
    if alert.is_some() {
        let percent = battery.and_then(|b| b.percent).unwrap_or(0);
        warn_low(app, percent);
    }
}

fn warn_low(app: &AppHandle, percent: u8) {
    play_chime();
    if crate::input::mode() == crate::input::Mode::Launcher {
        let _ = app.emit_to("main", "battery-low", percent);
    } else {
        // Com jogo aberto ou no modo desktop, o launcher não está à vista.
        let app = app.clone();
        std::thread::spawn(move || show_toast(&app, percent));
    }
}

const TOAST: &str = "toast";
const TOAST_SECS: u64 = 6;

/// Janela pequena por cima do jogo, que não rouba o foco nem recebe cliques.
fn show_toast(app: &AppHandle, percent: u8) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;

    if let Some(old) = app.get_webview_window(TOAST) {
        let _ = old.destroy();
    }
    let Ok(window) = WebviewWindowBuilder::new(app, TOAST, WebviewUrl::App(format!("index.html#toast-{percent}").into()))
        .title("Sofa - bateria")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .background_color(tauri::window::Color(42, 33, 24, 255))
        .build()
    else {
        return;
    };
    let _ = window.set_ignore_cursor_events(true);
    let Ok(Some(monitor)) = window.primary_monitor() else { return };
    let scale = monitor.scale_factor();
    let (mw, mx, my) = (monitor.size().width as i32, monitor.position().x, monitor.position().y);
    let (w, h, margin) = ((420.0 * scale) as i32, (96.0 * scale) as i32, (32.0 * scale) as i32);
    // Dá tempo da página carregar, para não aparecer um retângulo vazio.
    std::thread::sleep(std::time::Duration::from_millis(400));
    if let Ok(raw) = window.hwnd() {
        unsafe {
            let hwnd = HWND(raw.0 as _);
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize);
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), mx + mw - w - margin, my + margin, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(TOAST_SECS));
    let _ = window.destroy();
}

/// Dois tons curtos e baixos, descendo, como um alerta discreto.
fn chime_wav() -> Vec<u8> {
    const RATE: u32 = 22050;
    let mut samples: Vec<i16> = Vec::new();
    let mut tone = |freq: f32, ms: u32| {
        let n = RATE * ms / 1000;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let attack = (i as f32 / (RATE as f32 * 0.01)).min(1.0);
            let release = ((n - i) as f32 / (n as f32 * 0.6)).min(1.0);
            let v = (t * freq * std::f32::consts::TAU).sin() * 0.22 * attack * release;
            samples.push((v * i16::MAX as f32) as i16);
        }
    };
    tone(880.0, 130);
    tone(0.0, 50);
    tone(660.0, 200);

    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

fn play_chime() {
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    // O som toca de forma assíncrona; os dados precisam viver até o fim.
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let wav = WAV.get_or_init(chime_wav);
    unsafe {
        let _ = PlaySoundW(windows::core::PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn on_battery(p: u8) -> Option<Battery> {
        Some(Battery { state: PowerState::Battery, percent: Some(p) })
    }

    /// Diagnóstico com o controle real: `cargo test bluetooth_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bluetooth_real() {
        println!("BLUETOOTH {:?}", bluetooth_gamepad_percent());
    }

    #[test]
    fn traduz_estados_do_sdl() {
        assert_eq!(from_sdl(1, 55), on_battery(55));
        assert_eq!(from_sdl(3, 40).unwrap().state, PowerState::Charging);
        assert_eq!(from_sdl(2, -1), Some(Battery { state: PowerState::Wired, percent: None }));
        assert_eq!(from_sdl(0, 50), None, "desconhecido fica para a leitura do Bluetooth");
        assert_eq!(from_sdl(-1, 50), None);
    }

    #[test]
    fn reconhece_controles_pelo_nome() {
        for name in ["Xbox Wireless Controller", "Pro Controller", "DualSense Wireless Controller", "Joy-Con (L)", "8BitDo Gamepad"] {
            assert!(looks_like_gamepad(name), "{name}");
        }
        for name in ["JBL Tune 510BT", "Teclado Logitech", "Mouse MX"] {
            assert!(!looks_like_gamepad(name), "{name}");
        }
    }

    #[test]
    fn sem_leitura_nao_avisa() {
        let unknown = Some(Battery { state: PowerState::Unknown, percent: None });
        assert_eq!(next_alert(NOT_WARNED, unknown), (None, NOT_WARNED));
    }

    #[test]
    fn avisa_uma_vez_em_cada_limiar() {
        let (a, w) = next_alert(NOT_WARNED, on_battery(30));
        assert_eq!((a, w), (None, NOT_WARNED));
        let (a, w) = next_alert(w, on_battery(20));
        assert_eq!(a, Some(20));
        let (a, w) = next_alert(w, on_battery(18));
        assert_eq!(a, None, "não repete no mesmo limiar");
        let (a, w) = next_alert(w, on_battery(10));
        assert_eq!(a, Some(10));
        let (a, _) = next_alert(w, on_battery(5));
        assert_eq!(a, None);
    }

    #[test]
    fn queda_direto_para_o_minimo_avisa_uma_vez() {
        let (a, w) = next_alert(NOT_WARNED, on_battery(8));
        assert_eq!(a, Some(10));
        assert_eq!(next_alert(w, on_battery(7)).0, None);
    }

    #[test]
    fn carregar_rearma_o_aviso() {
        let (_, w) = next_alert(NOT_WARNED, on_battery(15));
        let charging = Some(Battery { state: PowerState::Charging, percent: Some(16) });
        let (a, w) = next_alert(w, charging);
        assert_eq!((a, w), (None, NOT_WARNED));
        assert_eq!(next_alert(w, on_battery(15)).0, Some(20));
    }

    #[test]
    fn oscilar_perto_do_limiar_nao_repete() {
        let (_, w) = next_alert(NOT_WARNED, on_battery(20));
        let (a, w) = next_alert(w, on_battery(22));
        assert_eq!(a, None);
        assert_eq!(next_alert(w, on_battery(20)).0, None);
    }

    #[test]
    fn som_e_um_wav_valido() {
        let wav = chime_wav();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(wav.len(), 44 + data_len);
    }
}
