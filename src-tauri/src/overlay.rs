//! Modo desktop: cursor pelo analógico e teclado virtual sobre qualquer janela.
//! Janela criada só quando usada, para não gastar memória durante o jogo.

use crate::input::logic::Action;
use crate::input::{self, Mode};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE};

pub static KEYBOARD_OPEN: AtomicBool = AtomicBool::new(false);

const LABEL: &str = "overlay";

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

fn unicode_input(unit: u16, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_UNICODE | KEYEVENTF_KEYUP } else { KEYEVENTF_UNICODE };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: unit, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn vk_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn mouse_input(dx: i32, dy: i32, data: u32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: data, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// Texto vira eventos Unicode, que funcionam em qualquer layout de teclado do Windows.
pub fn type_text(text: &str) {
    let inputs: Vec<INPUT> = text.encode_utf16().flat_map(|u| [unicode_input(u, false), unicode_input(u, true)]).collect();
    send(&inputs);
}

pub fn press_key(vk: VIRTUAL_KEY) {
    send(&[vk_input(vk, false), vk_input(vk, true)]);
}

pub fn key_by_name(name: &str) -> Option<VIRTUAL_KEY> {
    Some(match name {
        "backspace" => VK_BACK,
        "enter" => VK_RETURN,
        "tab" => VK_TAB,
        "escape" => VK_ESCAPE,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "up" => VK_UP,
        "down" => VK_DOWN,
        _ => return None,
    })
}

/// Ctrl+V na janela em primeiro plano.
pub fn paste() {
    send(&[vk_input(VK_CONTROL, false), vk_input(VK_V, false), vk_input(VK_V, true), vk_input(VK_CONTROL, true)]);
}

/// Coloca um texto na área de transferência do Windows.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_UNICODETEXT: u32 = 13;

    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        OpenClipboard(None).map_err(|e| e.to_string())?;
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| e.to_string())?;
            let memory: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2).map_err(|e| e.to_string())?;
            let target = GlobalLock(memory) as *mut u16;
            if target.is_null() {
                let _ = GlobalFree(Some(memory));
                return Err("Não foi possível reservar memória.".into());
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
            let _ = GlobalUnlock(memory);
            // Depois de aceito, o Windows passa a ser o dono da memória.
            if let Err(e) = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))) {
                let _ = GlobalFree(Some(memory));
                return Err(e.to_string());
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

pub fn move_cursor(dx: i32, dy: i32) {
    send(&[mouse_input(dx, dy, 0, MOUSEEVENTF_MOVE)]);
}

pub fn scroll(amount: i32) {
    send(&[mouse_input(0, 0, amount as u32, MOUSEEVENTF_WHEEL)]);
}

fn click(down: MOUSE_EVENT_FLAGS, up: MOUSE_EVENT_FLAGS) {
    send(&[mouse_input(0, 0, 0, down), mouse_input(0, 0, 0, up)]);
}

fn alt_tab() {
    send(&[vk_input(VK_MENU, false), vk_input(VK_TAB, false), vk_input(VK_TAB, true), vk_input(VK_MENU, true)]);
}

#[derive(Serialize, Clone)]
struct InputPayload {
    action: Action,
}

/// Ações vindas do controle enquanto o modo desktop está ativo.
pub fn on_action(app: &AppHandle, action: Action) {
    if KEYBOARD_OPEN.load(Ordering::Relaxed) {
        let _ = app.emit_to(LABEL, "input", InputPayload { action });
        return;
    }
    match action {
        Action::Confirm => click(MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        Action::X => click(MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        Action::Y => set_keyboard(app, true),
        Action::Back => exit(app),
        Action::Menu => press_key(VK_RETURN),
        Action::View => press_key(VK_ESCAPE),
        Action::Lb => alt_tab(),
        Action::Rb => press_key(VK_TAB),
        Action::Up => press_key(VK_UP),
        Action::Down => press_key(VK_DOWN),
        Action::Left => press_key(VK_LEFT),
        Action::Right => press_key(VK_RIGHT),
        // Só fazem parte da combinação, tratada antes de chegar aqui.
        Action::L3 | Action::R3 => {}
    }
}

/// Alterna entre a barra de ajuda fina e o teclado, mantendo a janela colada na base da tela.
pub fn set_keyboard(app: &AppHandle, open: bool) {
    KEYBOARD_OPEN.store(open, Ordering::Relaxed);
    let Some(window) = app.get_webview_window(LABEL) else { return };
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        let height = if open { size.height * 0.5 } else { size.height * 0.07 };
        let _ = window.set_size(LogicalSize::new(size.width, height));
        let _ = window.set_position(tauri::LogicalPosition::new(0.0, size.height - height));
    }
    // Com o teclado fechado, cliques atravessam a barra de ajuda.
    let _ = window.set_ignore_cursor_events(!open);
    let _ = app.emit_to(LABEL, "desktop-keyboard", open);
}

pub fn enter(app: &AppHandle) -> Result<(), String> {
    KEYBOARD_OPEN.store(false, Ordering::Relaxed);
    if app.get_webview_window(LABEL).is_none() {
        let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html#overlay".into()))
            .title("Sofa - modo desktop")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
        // Sem isto, o teclado roubaria o foco da janela que deve receber o texto.
        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                let hwnd = HWND(hwnd.0 as _);
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE.0 as isize);
            }
        }
    }
    set_keyboard(app, false);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.show();
    }
    crate::session::hide_launcher(app);
    input::set_mode(Mode::Desktop);
    Ok(())
}

pub fn close(app: &AppHandle) {
    KEYBOARD_OPEN.store(false, Ordering::Relaxed);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}

/// Sai do modo desktop e volta ao launcher, que abre o menu rápido.
pub fn exit(app: &AppHandle) {
    close(app);
    crate::session::show_launcher(app, false);
    let _ = app.emit_to("main", "combo", ());
}
