//! Comandos chamados pela interface.

use crate::library::{scan_all, GameView, Meta};
use crate::providers::emulator::{preset, Emulator};
use crate::providers::manual::ManualGame;
use crate::session::{self, SessionInfo};
use crate::{art, credentials, importer, input, overlay, AppState};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

type St<'a> = State<'a, Arc<AppState>>;

fn with_meta(state: &AppState, id: &str, f: impl FnOnce(&mut Meta)) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    f(store.meta.entry(id.to_string()).or_default());
    store.save().map_err(|e| e.to_string())
}

pub fn rescan(state: &AppState) {
    let games = {
        let store = state.store.lock().unwrap();
        scan_all(&store)
    };
    *state.games.lock().unwrap() = games;
}

#[tauri::command]
pub fn library_list(state: St) -> Vec<GameView> {
    let store = state.store.lock().unwrap();
    let local = art::LocalArt::load(Path::new(&store.library_dir()));
    state
        .games
        .lock()
        .unwrap()
        .iter()
        .map(|g| store.view(g, local.cover(&g.id, &g.title), local.hero(&g.id, &g.title)))
        .collect()
}

#[tauri::command(async)]
pub fn library_refresh(state: St) {
    rescan(&state);
}

#[tauri::command]
pub fn set_favorite(state: St, id: String, value: bool) -> Result<(), String> {
    with_meta(&state, &id, |m| m.favorite = value)
}

#[tauri::command]
pub fn set_hidden(state: St, id: String, value: bool) -> Result<(), String> {
    with_meta(&state, &id, |m| m.hidden = value)
}

#[tauri::command]
pub fn set_art(state: St, id: String, path: Option<String>) -> Result<(), String> {
    with_meta(&state, &id, |m| m.art = path)
}

#[tauri::command]
pub fn set_hero(state: St, id: String, path: Option<String>) -> Result<(), String> {
    with_meta(&state, &id, |m| m.hero = path)
}

#[tauri::command]
pub fn reduce_animations_get(state: St) -> bool {
    state.store.lock().unwrap().reduce_animations
}

#[tauri::command]
pub fn reduce_animations_set(state: St, value: bool) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    store.reduce_animations = value;
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_launch_file(state: St, id: String, path: Option<String>) -> Result<(), String> {
    with_meta(&state, &id, |m| m.launch_file = path)
}

// ---- Sessão ----

#[tauri::command(async)]
pub fn launch_game(app: AppHandle, state: St, id: String) -> Result<(), String> {
    session::start(&app, &state, &id)
}

#[tauri::command]
pub fn session_info(state: St) -> Option<SessionInfo> {
    session::info(&state)
}

#[tauri::command(async)]
pub fn session_resume(app: AppHandle, state: St) {
    session::resume(&app, &state);
}

#[tauri::command(async)]
pub fn session_close(app: AppHandle, state: St) {
    session::close(&app, &state);
}

#[tauri::command(async)]
pub fn session_cancel(app: AppHandle, state: St) {
    session::cancel(&app, &state);
}

// ---- Emuladores e jogos manuais ----

#[tauri::command]
pub fn emulators_list(state: St) -> Vec<Emulator> {
    state.store.lock().unwrap().emulators.clone()
}

/// Cria a partir da predefinição, com um id que não colide com os existentes.
#[tauri::command]
pub fn emulator_add(state: St, kind: String, exe: String) -> Result<Emulator, String> {
    let mut store = state.store.lock().unwrap();
    let mut emu = preset(&kind, &exe);
    let mut n = 1;
    while store.emulators.iter().any(|e| e.id == emu.id) {
        n += 1;
        emu.id = format!("{kind}{n}");
    }
    if kind == "generic" {
        emu.name = Path::new(&exe).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(emu.name);
    }
    store.emulators.push(emu.clone());
    store.save().map_err(|e| e.to_string())?;
    Ok(emu)
}

#[tauri::command]
pub fn emulator_update(state: St, emulator: Emulator) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    let slot = store.emulators.iter_mut().find(|e| e.id == emulator.id).ok_or("Emulador não encontrado")?;
    *slot = emulator;
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn emulator_remove(state: St, id: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    store.emulators.retain(|e| e.id != id);
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn manual_add(state: St, title: String, path: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    let id = format!("{}", crate::library::now_secs());
    store.manual.push(ManualGame { id, title, path, args: vec![], cwd: None });
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn manual_remove(state: St, game_id: String) -> Result<(), String> {
    let id = game_id.strip_prefix("manual:").ok_or("Não é um jogo manual")?;
    let mut store = state.store.lock().unwrap();
    store.manual.retain(|m| m.id != id);
    store.save().map_err(|e| e.to_string())
}

// ---- Importação ----

#[tauri::command(async)]
pub fn import_scan(state: St) -> Vec<importer::DrivePlan> {
    importer::scan_drives(&state)
}

#[tauri::command(async)]
pub fn import_start(
    app: AppHandle,
    state: St,
    drive: String,
    ids: Vec<String>,
    choices: HashMap<String, String>,
) -> Result<(), String> {
    importer::start(app, state.inner().clone(), drive, ids, choices)
}

#[tauri::command]
pub fn import_cancel(state: St) {
    importer::cancel(&state);
}

#[tauri::command(async)]
pub fn game_files_list(state: St) -> Vec<importer::GameFiles> {
    let library = state.store.lock().unwrap().library_dir();
    importer::list_game_files(Path::new(&library))
}

#[tauri::command]
pub fn clipboard_copy(text: String) -> Result<(), String> {
    overlay::copy_to_clipboard(&text)
}

fn find_game(state: &St, id: &str) -> Result<crate::providers::Game, String> {
    state.games.lock().unwrap().iter().find(|g| g.id == id).cloned().ok_or_else(|| "Jogo não encontrado.".to_string())
}

/// Estado de atualização lido agora do manifesto da loja, para acompanhar o progresso.
#[tauri::command]
pub fn game_update_status(state: St, id: String) -> Result<Option<crate::providers::UpdateState>, String> {
    let game = find_game(&state, &id)?;
    let Some(manifest) = game.manifest.as_deref() else { return Ok(None) };
    Ok(match game.source.as_str() {
        "steam" => crate::providers::steam::read_update(manifest),
        "epic" => crate::providers::epic::read_update(manifest),
        _ => None,
    })
}

/// Pede à loja que procure atualizações. Devolve se foi preciso abrir a Steam.
#[tauri::command]
pub fn game_update_check(state: St, id: String) -> Result<bool, String> {
    let game = find_game(&state, &id)?;
    match (game.source.as_str(), &game.launch) {
        ("steam", _) => crate::providers::steam::ensure_running(),
        ("epic", crate::providers::Launch::Uri { uri, .. }) => {
            session::open_uri(&crate::providers::epic::update_check_uri(uri))?;
            Ok(false)
        }
        _ => Err("Este jogo não tem uma loja para procurar atualizações.".into()),
    }
}

#[tauri::command]
pub fn volume_get() -> Result<crate::volume::VolumeState, String> {
    crate::volume::get()
}

#[tauri::command]
pub fn volume_set(level: i32) -> Result<(), String> {
    crate::volume::set_level(level)
}

#[tauri::command]
pub fn volume_set_mute(muted: bool) -> Result<(), String> {
    crate::volume::set_mute(muted)
}

#[tauri::command]
pub fn library_dir_get(state: St) -> String {
    state.store.lock().unwrap().library_dir()
}

#[tauri::command]
pub fn library_dir_set(state: St, path: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    store.library_dir = Some(path);
    store.save().map_err(|e| e.to_string())
}

// ---- Atualizações ----

#[derive(Serialize)]
pub struct UpdateInfo {
    current: String,
    version: Option<String>,
    notes: Option<String>,
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    done: u64,
    total: Option<u64>,
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateInfo, String> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(UpdateInfo {
        current: app.package_info().version.to_string(),
        version: update.as_ref().map(|u| u.version.clone()),
        notes: update.and_then(|u| u.body),
    })
}

/// Baixa e instala. A assinatura é conferida pelo atualizador antes de instalar.
/// Ao terminar, o instalador fecha e reabre o Sofa.
#[tauri::command]
pub async fn update_install(app: AppHandle, state: St<'_>) -> Result<(), String> {
    if state.session.lock().unwrap().is_some() {
        return Err("Feche o jogo antes de atualizar.".into());
    }
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Aguarde a importação terminar antes de atualizar.".into());
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("O Sofa já está na versão mais recente.")?;
    let mut done = 0u64;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let _ = progress_app.emit("update-progress", UpdateProgress { done, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())
}

// ---- Explorador de arquivos ----

#[derive(Serialize)]
pub struct FsEntry {
    name: String,
    path: String,
    dir: bool,
}

#[tauri::command]
pub fn fs_roots() -> Vec<FsEntry> {
    let mut out = vec![];
    if let Ok(profile) = std::env::var("USERPROFILE") {
        for (name, sub) in [("Documentos", "Documents"), ("Área de Trabalho", "Desktop"), ("Downloads", "Downloads")] {
            let path = Path::new(&profile).join(sub);
            if path.is_dir() {
                out.push(FsEntry { name: name.into(), path: path.to_string_lossy().into_owned(), dir: true });
            }
        }
    }
    for letter in 'A'..='Z' {
        let path = format!("{letter}:\\");
        if Path::new(&path).is_dir() {
            out.push(FsEntry { name: format!("Disco {letter}:"), path, dir: true });
        }
    }
    out
}

/// Lista pastas e, se `exts` não estiver vazio, só os arquivos com essas extensões.
#[tauri::command]
pub fn fs_list(path: String, exts: Vec<String>, dirs_only: bool) -> Result<Vec<FsEntry>, String> {
    let exts: Vec<String> = exts.iter().map(|e| e.to_lowercase()).collect();
    let mut out: Vec<FsEntry> = std::fs::read_dir(&path)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let dir = p.is_dir();
            let keep = dir
                || (!dirs_only
                    && (exts.is_empty()
                        || p.extension().is_some_and(|x| exts.contains(&x.to_string_lossy().to_lowercase()))));
            keep.then(|| FsEntry { name: e.file_name().to_string_lossy().into_owned(), path: p.to_string_lossy().into_owned(), dir })
        })
        .collect();
    out.sort_by_key(|e| (!e.dir, e.name.to_lowercase()));
    Ok(out)
}

// ---- Modo desktop ----

#[tauri::command(async)]
pub fn desktop_enter(app: AppHandle) -> Result<(), String> {
    overlay::enter(&app)
}

#[tauri::command(async)]
pub fn desktop_exit(app: AppHandle) {
    overlay::exit(&app);
}

#[tauri::command(async)]
pub fn desktop_keyboard(app: AppHandle, open: bool) {
    overlay::set_keyboard(&app, open);
}

#[tauri::command]
pub fn desktop_type(text: String) {
    overlay::type_text(&text);
}

#[tauri::command]
pub fn desktop_key(name: String) -> Result<(), String> {
    if name == "paste" {
        overlay::paste();
        return Ok(());
    }
    let vk = overlay::key_by_name(&name).ok_or("Tecla desconhecida")?;
    overlay::press_key(vk);
    Ok(())
}

// ---- Capas ----

#[tauri::command]
pub fn art_key_status() -> bool {
    credentials::load().is_some()
}

#[tauri::command]
pub fn art_key_save(key: String) -> Result<(), String> {
    credentials::save(key.trim())
}

#[tauri::command(async)]
pub fn art_fetch(app: AppHandle, state: St) {
    art::fetch_missing(app, state.inner().clone());
}

// ---- Configurações e sistema ----

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const AUTOSTART_ARG: &str = "--autostart";

/// Liga na primeira vez (o padrão) e, depois, só atualiza o caminho de quem deixou ligado.
/// Quem desligou continua desligado.
pub fn should_register_autostart(already_defaulted: bool, enabled: bool) -> bool {
    !already_defaulted || enabled
}

/// Só no build de release, para o `tauri dev` não registrar o executável de desenvolvimento.
pub fn apply_autostart_default(state: &AppState) {
    if cfg!(debug_assertions) {
        return;
    }
    let mut store = state.store.lock().unwrap();
    if !should_register_autostart(store.autostart_defaulted, autostart_get()) {
        return;
    }
    if autostart_set(true).is_ok() && !store.autostart_defaulted {
        store.autostart_defaulted = true;
        let _ = store.save();
    }
}

#[tauri::command]
pub fn autostart_get() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>("Sofa"))
        .is_ok()
}

#[tauri::command]
pub fn autostart_set(enabled: bool) -> Result<(), String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("Sofa", &format!("\"{}\" {AUTOSTART_ARG}", exe.display())).map_err(|e| e.to_string())
    } else {
        match key.delete_value("Sofa") {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[tauri::command]
pub fn swap_confirm_get(state: St) -> bool {
    state.store.lock().unwrap().swap_confirm
}

#[tauri::command]
pub fn swap_confirm_set(state: St, value: bool) -> Result<(), String> {
    input::set_swap(value);
    let mut store = state.store.lock().unwrap();
    store.swap_confirm = value;
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn power(app: AppHandle, action: String) -> Result<(), String> {
    match action.as_str() {
        "exit" => {
            app.exit(0);
            Ok(())
        }
        "suspend" => unsafe {
            let _ = windows::Win32::System::Power::SetSuspendState(false, false, false);
            Ok(())
        },
        "shutdown" | "restart" => {
            let flag = if action == "shutdown" { "/s" } else { "/r" };
            std::process::Command::new("shutdown").args([flag, "/t", "0"]).spawn().map(|_| ()).map_err(|e| e.to_string())
        }
        _ => Err("Ação desconhecida".into()),
    }
}

/// Avisa a interface de que a janela principal voltou (chamado pela combinação).
pub fn notify_combo(app: &AppHandle) {
    if app.get_webview_window("main").is_some() {
        let _ = app.emit_to("main", "combo", ());
    }
}

#[cfg(test)]
mod tests {
    use super::should_register_autostart;

    #[test]
    fn inicio_automatico_liga_uma_vez_e_respeita_quem_desligou() {
        // Primeira execução: liga, esteja como estiver.
        assert!(should_register_autostart(false, false));
        // Quem desligou depois continua desligado.
        assert!(!should_register_autostart(true, false));
        // Quem deixou ligado tem o caminho atualizado.
        assert!(should_register_autostart(true, true));
    }
}
