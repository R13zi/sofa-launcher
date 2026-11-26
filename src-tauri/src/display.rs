//! Em qual monitor o Sofa roda: escolhido nas Configurações, ou o principal do Windows.

use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition};
use windows::Win32::Foundation::RECT;

#[derive(Serialize)]
pub struct MonitorInfo {
    /// Nome do Windows, como `\\.\DISPLAY2`. É o que fica guardado.
    pub name: String,
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Nome do monitor ligado a esta saída de vídeo, como "Lenovo HDR Monitor".
fn friendly_name(device: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};
    let mut dev = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
    let ok = unsafe { EnumDisplayDevicesW(&HSTRING::from(device), 0, &mut dev, 0) }.as_bool();
    if !ok {
        return None;
    }
    let len = dev.DeviceString.iter().position(|&c| c == 0).unwrap_or(dev.DeviceString.len());
    let name = String::from_utf16_lossy(&dev.DeviceString[..len]).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// "Monitor 2", a partir de `\\.\DISPLAY2`.
pub fn number(device: &str) -> Option<u32> {
    device.trim_start_matches(r"\\.\").strip_prefix("DISPLAY")?.parse().ok()
}

pub fn label(device: &str, friendly: Option<&str>, width: u32, height: u32) -> String {
    let base = number(device).map(|n| format!("Monitor {n}")).unwrap_or_else(|| device.to_string());
    match friendly {
        Some(f) if !f.eq_ignore_ascii_case("Generic PnP Monitor") => format!("{base} · {f} · {width}×{height}"),
        _ => format!("{base} · {width}×{height}"),
    }
}

fn primary_name(app: &AppHandle) -> Option<String> {
    app.primary_monitor().ok().flatten().and_then(|m| m.name().cloned())
}

pub fn list(app: &AppHandle) -> Vec<MonitorInfo> {
    let primary = primary_name(app);
    let mut out: Vec<MonitorInfo> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            let (width, height) = (m.size().width, m.size().height);
            Some(MonitorInfo {
                label: label(&name, friendly_name(&name).as_deref(), width, height),
                primary: primary.as_deref() == Some(name.as_str()),
                name,
                width,
                height,
            })
        })
        .collect();
    out.sort_by_key(|m| number(&m.name).unwrap_or(u32::MAX));
    out
}

/// O monitor escolhido, se estiver conectado; senão o principal.
pub fn chosen(app: &AppHandle) -> Option<Monitor> {
    let wanted = app.state::<std::sync::Arc<AppState>>().store.lock().unwrap().monitor.clone();
    let monitors = app.available_monitors().unwrap_or_default();
    wanted
        .and_then(|w| monitors.into_iter().find(|m| m.name() == Some(&w)))
        .or_else(|| app.primary_monitor().ok().flatten())
}

/// Área do monitor escolhido em pixels físicos, nas coordenadas da área de trabalho.
pub fn chosen_rect(app: &AppHandle) -> Option<RECT> {
    let m = chosen(app)?;
    let (pos, size) = (m.position(), m.size());
    Some(RECT { left: pos.x, top: pos.y, right: pos.x + size.width as i32, bottom: pos.y + size.height as i32 })
}

/// Leva a janela do Sofa para o monitor escolhido, em tela cheia.
pub fn place_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let Some(target) = chosen(app) else { return };
    let current = window.current_monitor().ok().flatten();
    if current.as_ref().and_then(|m| m.name()) == target.name() {
        return;
    }
    crate::library::debug_log(&format!("levando o Sofa para {:?}", target.name()));
    let _ = window.set_fullscreen(false);
    let pos = target.position();
    let _ = window.set_position(PhysicalPosition::new(pos.x + 50, pos.y + 50));
    let _ = window.set_fullscreen(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numero_e_nome_do_monitor() {
        assert_eq!(number(r"\\.\DISPLAY2"), Some(2));
        assert_eq!(number("outro"), None);
        assert_eq!(label(r"\\.\DISPLAY2", Some("Lenovo HDR Monitor"), 1920, 1080), "Monitor 2 · Lenovo HDR Monitor · 1920×1080");
        assert_eq!(label(r"\\.\DISPLAY1", Some("Generic PnP Monitor"), 1920, 1200), "Monitor 1 · 1920×1200");
        assert_eq!(label(r"\\.\DISPLAY1", None, 1920, 1200), "Monitor 1 · 1920×1200");
    }
}
