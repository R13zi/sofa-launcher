#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod battery;
mod borderless;
mod commands;
mod credentials;
mod display;
mod download_watch;
mod importer;
mod input;
mod library;
mod overlay;
mod providers;
mod session;
mod storage;
mod sunshine;
mod volume;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject, INFINITE};

/// Só uma cópia do Sofa roda por vez; a vigia do Moonlight também olha este mutex.
pub const INSTANCE_MUTEX: windows::core::PCWSTR = w!("Local\\SofaLauncher");

/// A segunda cópia do Sofa avisa a primeira por este evento, para ela vir para a frente.
const SHOW_EVENT: windows::core::PCWSTR = w!("Local\\SofaShow");

/// Pede à cópia aberta do Sofa que venha para a frente.
fn signal_show() {
    if let Ok(event) = unsafe { CreateEventW(None, false, false, SHOW_EVENT) } {
        let _ = unsafe { SetEvent(event) };
    }
}

/// Abrir o Sofa de novo (pelo Moonlight, por exemplo) traz o launcher para a frente,
/// a não ser que um jogo ou o modo desktop esteja na tela.
fn spawn_show_listener(app: tauri::AppHandle) {
    let Ok(event) = (unsafe { CreateEventW(None, false, false, SHOW_EVENT) }) else { return };
    let event = event.0 as isize;
    std::thread::spawn(move || loop {
        let handle = windows::Win32::Foundation::HANDLE(event as _);
        unsafe { WaitForSingleObject(handle, INFINITE) };
        library::debug_log("Sofa aberto de novo: trazendo para a frente");
        if input::mode() == input::Mode::Launcher {
            session::show_launcher(&app, false);
        }
    });
}

pub struct AppState {
    pub store: Mutex<library::Store>,
    pub games: Mutex<Vec<providers::Game>>,
    pub session: Mutex<Option<session::Session>>,
    pub importing: AtomicBool,
    pub import_cancel: AtomicBool,
}

/// Ctrl + Shift + F10 abre o menu mesmo com um jogo em primeiro plano, como o L3 + R3.
/// Se outro programa já usa o atalho, o Sofa segue funcionando sem ele.
fn register_menu_shortcut(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                input::trigger_combo(app);
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        library::debug_log(&format!("atalho global indisponível: {e}"));
        return;
    }
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F10);
    if let Err(e) = app.global_shortcut().register(shortcut) {
        library::debug_log(&format!("atalho global indisponível: {e}"));
    }
}

fn main() {
    // Cadastro no Sunshine, feito por uma cópia aberta como administrador, sem janela.
    if let Some(code) = sunshine::run_admin_task(std::env::args()) {
        std::process::exit(code);
    }

    // Aberto pelo Moonlight: acompanha o Sofa e termina junto, para o Sunshine encerrar a transmissão.
    if let Some(code) = sunshine::run_moonlight_watch(std::env::args(), signal_show) {
        std::process::exit(code);
    }

    // Duas instâncias reagiriam ao mesmo controle ao mesmo tempo. O mutex vive até o fim do processo.
    let _guard = unsafe { CreateMutexW(None, false, INSTANCE_MUTEX) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        signal_show();
        return;
    }

    let store = library::Store::load();
    input::set_swap(store.swap_confirm);
    let state = Arc::new(AppState {
        store: Mutex::new(store),
        games: Mutex::new(vec![]),
        session: Mutex::new(None),
        importing: AtomicBool::new(false),
        import_cancel: AtomicBool::new(false),
    });
    borderless::restore_taskbar_at_start();
    commands::apply_autostart_default(&state);
    commands::rescan(&state);
    let from_autostart = std::env::args().any(|a| a == commands::AUTOSTART_ARG);

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state.clone())
        .setup(move |app| {
            register_menu_shortcut(app.handle());
            display::place_main(app.handle());
            input::spawn(app.handle().clone());
            spawn_show_listener(app.handle().clone());
            session::spawn_monitor(app.handle().clone(), state.clone());
            importer::spawn_detector(app.handle().clone(), state.clone());
            download_watch::spawn(app.handle().clone(), state.clone());
            if from_autostart {
                session::spawn_autostart_guard(app.handle().clone(), state.clone());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library_list,
            commands::library_refresh,
            commands::set_favorite,
            commands::set_hidden,
            commands::set_borderless,
            commands::set_art,
            commands::set_hero,
            commands::reduce_animations_get,
            commands::reduce_animations_set,
            commands::set_launch_file,
            commands::launch_game,
            commands::session_info,
            commands::session_resume,
            commands::session_close,
            commands::session_cancel,
            commands::emulators_list,
            commands::emulator_add,
            commands::emulator_update,
            commands::emulator_remove,
            commands::manual_add,
            commands::manual_remove,
            commands::import_scan,
            commands::import_start,
            commands::import_cancel,
            commands::update_check,
            commands::update_install,
            commands::game_files_list,
            commands::clipboard_copy,
            commands::game_update_status,
            commands::game_update_check,
            commands::controller_battery,
            commands::sunshine_status,
            commands::monitors_list,
            commands::monitor_get,
            commands::monitor_set,
            commands::sunshine_register,
            commands::storage_overview,
            commands::game_size,
            commands::game_uninstall,
            commands::store_installable,
            commands::steam_install_targets,
            commands::downloads,
            commands::store_open_downloads,
            commands::steam_install,
            commands::epic_install,
            commands::epic_open_library,
            commands::steam_key_status,
            commands::steam_key_save,
            commands::volume_get,
            commands::volume_set,
            commands::volume_set_mute,
            commands::library_dir_get,
            commands::library_dir_set,
            commands::fs_roots,
            commands::fs_list,
            commands::desktop_enter,
            commands::desktop_exit,
            commands::desktop_keyboard,
            commands::desktop_type,
            commands::desktop_key,
            commands::art_key_status,
            commands::art_key_save,
            commands::art_fetch,
            commands::autostart_get,
            commands::autostart_set,
            commands::swap_confirm_get,
            commands::swap_confirm_set,
            commands::power,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Sofa");
}
