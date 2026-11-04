#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod art;
mod commands;
mod credentials;
mod importer;
mod input;
mod library;
mod overlay;
mod providers;
mod session;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;

pub struct AppState {
    pub store: Mutex<library::Store>,
    pub games: Mutex<Vec<providers::Game>>,
    pub session: Mutex<Option<session::Session>>,
    pub importing: AtomicBool,
    pub import_cancel: AtomicBool,
}

fn main() {
    // Duas instâncias reagiriam ao mesmo controle ao mesmo tempo. O mutex vive até o fim do processo.
    let _guard = unsafe { CreateMutexW(None, false, w!("Local\\SofaLauncher")) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
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
    commands::apply_autostart_default(&state);
    commands::rescan(&state);
    let from_autostart = std::env::args().any(|a| a == commands::AUTOSTART_ARG);

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state.clone())
        .setup(move |app| {
            input::spawn(app.handle().clone());
            session::spawn_monitor(app.handle().clone(), state.clone());
            importer::spawn_detector(app.handle().clone(), state.clone());
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
            commands::set_art,
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
