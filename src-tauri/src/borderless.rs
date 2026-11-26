//! Tela cheia sem borda: estica a janela do jogo até a tela e tira a moldura.
//!
//! Serve para jogos que, em tela cheia de verdade, ficam pretos na transmissão do Sunshine
//! (Vulkan desenhando direto no monitor). Em janela, quem monta a imagem é o Windows e a
//! captura funciona.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, IsZoomed, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, GWL_EXSTYLE, GWL_STYLE, HWND_TOP, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_RESTORE, WS_CAPTION, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_STATICEDGE, WS_EX_WINDOWEDGE,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
};

const FRAME: u32 = WS_CAPTION.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0 | WS_MAXIMIZEBOX.0 | WS_SYSMENU.0;
const EX_FRAME: u32 = WS_EX_DLGMODALFRAME.0 | WS_EX_CLIENTEDGE.0 | WS_EX_STATICEDGE.0 | WS_EX_WINDOWEDGE.0;

/// Posição e tamanho que a janela deve ter: o monitor do Sofa com uma linha a menos. Cobrindo
/// a tela inteira, o Windows e o driver podem tratar o jogo como tela cheia de novo, e a
/// transmissão volta a ficar preta.
pub fn target_rect(monitor: RECT) -> RECT {
    RECT { bottom: monitor.bottom - 1, ..monitor }
}

/// Se a janela ainda tem moldura ou não está no lugar certo.
pub fn needs_change(style: u32, ex_style: u32, current: RECT, target: RECT) -> bool {
    style & FRAME != 0 || ex_style & EX_FRAME != 0 || current != target
}

/// A janela principal: a maior, deixando de fora a da tela do GamePad (jogos de Wii U no Cemu).
pub fn pick_main(windows: &[(i64, String)]) -> Option<usize> {
    windows
        .iter()
        .enumerate()
        .filter(|(_, (_, title))| !title.to_lowercase().contains("gamepad"))
        .max_by_key(|(_, (area, _))| *area)
        .map(|(i, _)| i)
}

/// Se o Sofa escondeu a barra de tarefas (só mostra de volta o que ele mesmo escondeu).
static TASKBAR_HIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Barras de tarefas de todos os monitores.
fn taskbars() -> Vec<HWND> {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowExW;
    let mut out = vec![];
    for class in [w!("Shell_TrayWnd"), w!("Shell_SecondaryTrayWnd")] {
        let mut after: Option<HWND> = None;
        while let Ok(hwnd) = unsafe { FindWindowExW(None, after, class, None) } {
            if hwnd.is_invalid() {
                break;
            }
            out.push(hwnd);
            after = Some(hwnd);
        }
    }
    out
}

/// Com a janela uma linha menor que a tela, o Windows não a trata como tela cheia e deixa a
/// barra de tarefas por cima. Durante o jogo o Sofa esconde a barra; ao voltar, mostra de novo.
pub fn set_taskbar_hidden(hidden: bool) {
    use std::sync::atomic::Ordering;
    use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNA};
    if TASKBAR_HIDDEN.swap(hidden, Ordering::SeqCst) == hidden {
        return;
    }
    crate::library::debug_log(if hidden { "barra de tarefas escondida" } else { "barra de tarefas de volta" });
    for bar in taskbars() {
        let _ = unsafe { ShowWindow(bar, if hidden { SW_HIDE } else { SW_SHOWNA }) };
    }
}

/// Ao abrir o Sofa: se ele fechou de repente com a barra escondida, ela volta.
pub fn restore_taskbar_at_start() {
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNA;
    for bar in taskbars() {
        let _ = unsafe { ShowWindow(bar, SW_SHOWNA) };
    }
}

fn title(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

fn rect(hwnd: HWND) -> RECT {
    let mut r = RECT::default();
    let _ = unsafe { GetWindowRect(hwnd, &mut r) };
    r
}

/// Aplica a tela cheia sem borda na janela principal do jogo. Chamada a cada volta do monitor
/// da sessão; só mexe quando a janela saiu do lugar, para não brigar com o jogo.
pub fn apply(windows: &[HWND], monitor: RECT) {
    let info: Vec<(i64, String)> = windows
        .iter()
        .map(|&h| {
            let r = rect(h);
            (i64::from(r.right - r.left) * i64::from(r.bottom - r.top), title(h))
        })
        .collect();
    let Some(i) = pick_main(&info) else { return };
    let hwnd = windows[i];
    let target = target_rect(monitor);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if !needs_change(style, ex_style, rect(hwnd), target) {
            return;
        }
        crate::library::debug_log(&format!("tela cheia sem borda: {}", info[i].1));
        if IsZoomed(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !FRAME) as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex_style & !EX_FRAME) as isize);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            target.left,
            target.top,
            target.right - target.left,
            target.bottom - target.top,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uma_linha_a_menos_que_a_tela() {
        let monitor = RECT { left: 0, top: -1080, right: 1920, bottom: 0 };
        assert_eq!(target_rect(monitor), RECT { left: 0, top: -1080, right: 1920, bottom: -1 });
    }

    #[test]
    fn so_mexe_quando_precisa() {
        let target = target_rect(RECT { left: 0, top: 0, right: 1920, bottom: 1080 });
        assert!(!needs_change(0, 0, target, target));
        assert!(needs_change(WS_CAPTION.0, 0, target, target));
        assert!(needs_change(0, WS_EX_WINDOWEDGE.0, target, target));
        assert!(needs_change(0, 0, RECT { left: 100, top: 100, right: 1380, bottom: 820 }, target));
    }

    #[test]
    fn escolhe_a_maior_janela_sem_ser_a_do_gamepad() {
        let windows = vec![(500, "Wind Waker HD".to_string()), (900, "GamePad View".to_string()), (100, "Log".to_string())];
        assert_eq!(pick_main(&windows), Some(0));
        assert_eq!(pick_main(&[]), None);
    }
}
