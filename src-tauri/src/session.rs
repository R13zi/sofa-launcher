//! Sessão de jogo: inicia, acompanha processos, passa o foco ao jogo e fecha.
//! O launcher não se esconde durante o jogo: fica atrás dele, para o desktop nunca aparecer.

use crate::input::{self, Mode};
use crate::importer::auto_file_for;
use crate::library::now_secs;
use crate::providers::manual::{has_placeholder, resolve_args};
use crate::providers::{Game, Launch};
use crate::{art, AppState};
use serde::Serialize;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP, VK_MENU};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, PostMessageW, SetForegroundWindow, SetWindowPos, ShowWindow, GW_OWNER, HWND_NOTOPMOST,
    HWND_TOPMOST, SET_WINDOW_POS_FLAGS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_RESTORE, SW_SHOWNORMAL,
    WM_CLOSE,
};

/// Tempo que a loja tem para abrir o jogo (pode haver atualização ou instalação de dependências).
const APPEAR_TIMEOUT: Duration = Duration::from_secs(180);
/// Depois disso o launcher sai do topo, para não esconder janelas da loja que pedem resposta.
const TOPMOST_LIMIT: Duration = Duration::from_secs(15);
/// Tempo sem processos para considerar o jogo encerrado (jogos que reiniciam a si mesmos).
const GONE_AFTER: Duration = Duration::from_secs(3);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(8);
const POLL_LAUNCHING: Duration = Duration::from_millis(500);
const POLL_PLAYING: Duration = Duration::from_secs(1);
/// Janelas menores que isto são auxiliares invisíveis, não o jogo.
const MIN_WINDOW_SIZE: i32 = 100;
const AUTOSTART_GUARD: Duration = Duration::from_secs(45);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Launching,
    Playing,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Surgiu a janela do jogo: passar o foco para ela.
    Handoff,
    ReleaseTopmost,
    Ended,
    TimedOut,
}

/// Decide as transições da sessão a partir do que foi observado, sem tocar no sistema.
pub struct Tracker {
    pub phase: Phase,
    started: Instant,
    pub first_seen: Option<Instant>,
    last_seen: Option<Instant>,
    topmost: bool,
}

impl Tracker {
    pub fn new(now: Instant) -> Self {
        Tracker { phase: Phase::Launching, started: now, first_seen: None, last_seen: None, topmost: true }
    }

    pub fn observe(&mut self, now: Instant, has_process: bool, has_window: bool) -> Vec<Step> {
        if has_process {
            self.first_seen.get_or_insert(now);
            self.last_seen = Some(now);
        }
        match self.last_seen {
            Some(last) if now.duration_since(last) >= GONE_AFTER => return vec![Step::Ended],
            None if now.duration_since(self.started) >= APPEAR_TIMEOUT => return vec![Step::TimedOut],
            _ => {}
        }
        let mut steps = vec![];
        if self.phase == Phase::Launching {
            if has_process && has_window {
                self.phase = Phase::Playing;
                self.topmost = false;
                steps.push(Step::Handoff);
            } else if self.topmost && now.duration_since(self.started) >= TOPMOST_LIMIT {
                self.topmost = false;
                steps.push(Step::ReleaseTopmost);
            }
        }
        steps
    }
}

pub enum Target {
    Job(JobHandle),
    Dir(String),
}

pub struct JobHandle(HANDLE);
// Um handle de Job Object pode ser usado de qualquer thread.
unsafe impl Send for JobHandle {}
unsafe impl Sync for JobHandle {}
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub struct Session {
    pub game_id: String,
    pub title: String,
    pub art: Option<String>,
    pub hero: Option<String>,
    target: Target,
    tracker: Tracker,
}

#[derive(Serialize, Clone)]
pub struct SessionInfo {
    pub game_id: String,
    pub title: String,
    pub art: Option<String>,
    pub hero: Option<String>,
    pub phase: Phase,
}

impl Session {
    fn info(&self) -> SessionInfo {
        SessionInfo {
            game_id: self.game_id.clone(),
            title: self.title.clone(),
            art: self.art.clone(),
            hero: self.hero.clone(),
            phase: self.tracker.phase,
        }
    }
}

/// Compara ignorando maiúsculas e exige separador, para `C:\Jogo` não casar com `C:\Jogo2`.
pub fn path_in_dir(path: &str, dir: &str) -> bool {
    let path = path.to_lowercase().replace('/', "\\");
    let dir = dir.to_lowercase().replace('/', "\\");
    let dir = dir.trim_end_matches('\\');
    !dir.is_empty() && path.starts_with(dir) && path[dir.len()..].starts_with('\\')
}

fn image_path(process: HANDLE) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

fn target_pids(target: &Target) -> Vec<u32> {
    let mut pids = vec![];
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return pids };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        while ok {
            let pid = entry.th32ProcessID;
            if pid != 0 {
                if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                    let matches = match target {
                        Target::Job(job) => {
                            let mut result = BOOL(0);
                            IsProcessInJob(process, Some(job.0), &mut result).is_ok() && result.as_bool()
                        }
                        Target::Dir(dir) => image_path(process).is_some_and(|p| path_in_dir(&p, dir)),
                    };
                    let _ = CloseHandle(process);
                    if matches {
                        pids.push(pid);
                    }
                }
            }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    pids
}

struct EnumData {
    pids: Vec<u32>,
    windows: Vec<HWND>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam.0 as *mut EnumData);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let has_owner = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid());
    let mut rect = RECT::default();
    let big_enough = GetWindowRect(hwnd, &mut rect).is_ok()
        && rect.right - rect.left >= MIN_WINDOW_SIZE
        && rect.bottom - rect.top >= MIN_WINDOW_SIZE;
    if data.pids.contains(&pid) && IsWindowVisible(hwnd).as_bool() && !has_owner && big_enough {
        data.windows.push(hwnd);
    }
    BOOL(1)
}

fn windows_of(pids: &[u32]) -> Vec<HWND> {
    if pids.is_empty() {
        return vec![];
    }
    let mut data = EnumData { pids: pids.to_vec(), windows: vec![] };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut data as *mut EnumData as isize));
    }
    data.windows
}

/// O Windows só deixa trocar o primeiro plano logo após uma entrada do usuário;
/// um Alt sintético satisfaz essa regra. Nada é enviado para dentro do jogo além disso.
pub fn force_foreground(hwnd: HWND) {
    unsafe {
        keybd_event(VK_MENU.0 as u8, 0, Default::default(), 0);
        keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}

fn set_low_memory(app: &AppHandle, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use windows::core::Interface;
    let Some(window) = app.get_webview_window("main") else { return };
    let _ = window.with_webview(move |webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        if let Ok(core) = core.cast::<ICoreWebView2_19>() {
            let level = if low {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            let _ = core.SetMemoryUsageTargetLevel(level);
        }
    });
}

fn launcher_hwnd(app: &AppHandle) -> Option<HWND> {
    let hwnd = app.get_webview_window("main")?.hwnd().ok()?;
    Some(HWND(hwnd.0 as _))
}

fn launcher_is_foreground(app: &AppHandle) -> bool {
    launcher_hwnd(app).is_some_and(|h| unsafe { GetForegroundWindow() } == h)
}

const KEEP_GEOMETRY: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_NOACTIVATE.0);

/// Chamada direta e síncrona, em vez de `set_always_on_top`, que é assíncrona: sair do topo
/// coloca a janela acima das comuns e, se isso ocorresse depois de o jogo receber o foco,
/// o launcher cobriria o jogo.
fn set_topmost(app: &AppHandle, on_top: bool) {
    let Some(hwnd) = launcher_hwnd(app) else { return };
    let order = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe {
        let _ = SetWindowPos(hwnd, Some(order), 0, 0, 0, 0, KEEP_GEOMETRY);
    }
}

pub fn show_launcher(app: &AppHandle, on_top: bool) {
    set_low_memory(app, false);
    input::set_mode(Mode::Launcher);
    set_topmost(app, on_top);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
    if let Some(hwnd) = launcher_hwnd(app) {
        force_foreground(hwnd);
    }
}

/// Só o modo desktop esconde o launcher, porque a função dele é mostrar as outras janelas.
pub fn hide_launcher(app: &AppHandle) {
    set_topmost(app, false);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    set_low_memory(app, true);
}

/// Coloca o launcher atrás do jogo e entrega o controle a ele.
fn to_backdrop(app: &AppHandle, pids: &[u32]) {
    input::set_mode(Mode::InGame);
    set_topmost(app, false);
    set_low_memory(app, true);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
    }
    if let Some(&game) = windows_of(pids).first() {
        // Logo abaixo do jogo: assim, quando ele fechar, o Windows ativa o launcher.
        if let Some(launcher) = launcher_hwnd(app) {
            unsafe {
                let _ = SetWindowPos(launcher, Some(game), 0, 0, 0, 0, KEEP_GEOMETRY);
            }
        }
        force_foreground(game);
    }
}

pub fn start(app: &AppHandle, state: &Arc<AppState>, game_id: &str) -> Result<(), String> {
    crate::library::debug_log(&format!("launch_game {game_id}"));
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Aguarde a importação terminar ou cancele-a para jogar.".into());
    }
    let mut session = state.session.lock().unwrap();
    if session.is_some() {
        return Err("Já existe um jogo aberto.".into());
    }
    let game = state
        .games
        .lock()
        .unwrap()
        .iter()
        .find(|g| g.id == game_id)
        .cloned()
        .ok_or("Jogo não encontrado. Atualize a biblioteca.")?;

    let launch_config = with_launch_file(&game, state)?;

    // A tela de carregamento entra antes do jogo, para o desktop não aparecer.
    set_topmost(app, true);

    let launched = launch(&launch_config);
    let target = match launched {
        Ok(target) => target,
        Err(e) => {
            set_topmost(app, false);
            return Err(e);
        }
    };

    let view = {
        let mut store = state.store.lock().unwrap();
        let local = art::LocalArt::load(Path::new(&store.library_dir()));
        let view = store.view(&game, local.cover(&game.id, &game.title), local.hero(&game.id, &game.title));
        store.meta.entry(game.id.clone()).or_default().last_played = now_secs();
        let _ = store.save();
        view
    };

    let new_session = Session {
        game_id: game.id.clone(),
        title: game.title.clone(),
        art: view.art,
        hero: view.hero,
        target,
        tracker: Tracker::new(Instant::now()),
    };
    let info = new_session.info();
    *session = Some(new_session);
    drop(session);

    let _ = app.emit("session", Some(info));
    Ok(())
}

/// Acrescenta o arquivo do jogo aos argumentos. Só `.exe` adicionados usam isso:
/// emuladores já recebem a ROM, e as lojas não aceitam argumentos por aqui.
fn with_launch_file(game: &Game, state: &AppState) -> Result<Launch, String> {
    let Launch::Exec { path, args, cwd } = &game.launch else { return Ok(game.launch.clone()) };
    if game.source != "manual" {
        return Ok(game.launch.clone());
    }
    let (chosen, library) = {
        let store = state.store.lock().unwrap();
        (store.meta.get(&game.id).and_then(|m| m.launch_file.clone()), store.library_dir())
    };
    if let Some(file) = &chosen {
        if !Path::new(file).exists() {
            return Err(format!(
                "O arquivo escolhido para {} não existe mais:\n{file}\n\nTroque ou remova em \"Arquivo ao iniciar\", nos detalhes do jogo.",
                game.title
            ));
        }
    }
    let auto = if chosen.is_none() && has_placeholder(args) {
        auto_file_for(&game.title, Path::new(&library))
    } else {
        None
    };
    Ok(Launch::Exec {
        path: path.clone(),
        args: resolve_args(args, chosen.as_deref(), auto.as_deref()),
        cwd: cwd.clone(),
    })
}

fn launch(launch: &Launch) -> Result<Target, String> {
    match launch {
        Launch::Uri { uri, watch_dir } => {
            let result = unsafe {
                ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(uri.as_str()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
            };
            // ShellExecute indica sucesso com um valor acima de 32.
            if result.0 as isize <= 32 {
                return Err(format!("Não foi possível abrir {uri}. A loja está instalada?"));
            }
            Ok(Target::Dir(watch_dir.clone()))
        }
        Launch::Exec { path, args, cwd } => {
            let child = std::process::Command::new(path)
                .args(args)
                .current_dir(cwd)
                .spawn()
                .map_err(|e| match e.raw_os_error() {
                    Some(740) => format!("{path} exige permissão de administrador."),
                    _ => format!("Não foi possível iniciar {path}: {e}"),
                })?;
            let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|e| e.to_string())?;
            let job = JobHandle(job);
            unsafe {
                AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle() as _)).map_err(|e| e.to_string())?;
            }
            Ok(Target::Job(job))
        }
    }
}

pub fn info(state: &AppState) -> Option<SessionInfo> {
    state.session.lock().unwrap().as_ref().map(Session::info)
}

fn current_pids(state: &AppState) -> Vec<u32> {
    state.session.lock().unwrap().as_ref().map(|s| target_pids(&s.target)).unwrap_or_default()
}

fn terminate(pids: &[u32]) {
    for &pid in pids {
        unsafe {
            if let Ok(process) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                let _ = TerminateProcess(process, 1);
                let _ = CloseHandle(process);
            }
        }
    }
}

/// Fecha o menu e devolve o foco ao jogo.
pub fn resume(app: &AppHandle, state: &AppState) {
    to_backdrop(app, &current_pids(state));
}

/// Pede para o jogo fechar; se não fechar a tempo, força o encerramento.
pub fn close(app: &AppHandle, state: &Arc<AppState>) {
    let pids = current_pids(state);
    // Sai do topo antes, para um eventual "deseja salvar?" do jogo ficar visível.
    to_backdrop(app, &pids);
    for hwnd in windows_of(&pids) {
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    let state = state.clone();
    std::thread::spawn(move || {
        std::thread::sleep(CLOSE_TIMEOUT);
        terminate(&current_pids(&state));
    });
}

/// Desiste de um jogo que ainda está abrindo.
pub fn cancel(app: &AppHandle, state: &AppState) {
    let finished = state.session.lock().unwrap().take();
    if let Some(session) = finished {
        terminate(&target_pids(&session.target));
    }
    show_launcher(app, false);
    let _ = app.emit("session", Option::<SessionInfo>::None);
}

fn finish(app: &AppHandle, state: &AppState, session: Session, error: Option<String>) {
    if let Some(seen) = session.tracker.first_seen {
        let mut store = state.store.lock().unwrap();
        store.meta.entry(session.game_id.clone()).or_default().playtime_secs += seen.elapsed().as_secs();
        let _ = store.save();
    }
    crate::overlay::close(app);
    show_launcher(app, false);
    let _ = app.emit("session", Option::<SessionInfo>::None);
    if let Some(message) = error {
        let _ = app.emit("session-failed", message);
    }
}

/// Acompanha a sessão: passa o foco quando a janela do jogo surge e detecta o fim.
pub fn spawn_monitor(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let mut delay = POLL_PLAYING;
        let mut had_windows = false;
        loop {
            std::thread::sleep(delay);
            let mut guard = state.session.lock().unwrap();
            let Some(session) = guard.as_mut() else {
                delay = POLL_PLAYING;
                continue;
            };
            let pids = target_pids(&session.target);
            let windows = windows_of(&pids);
            let steps = session.tracker.observe(Instant::now(), !pids.is_empty(), !windows.is_empty());
            delay = if session.tracker.phase == Phase::Launching { POLL_LAUNCHING } else { POLL_PLAYING };

            for step in steps {
                match step {
                    Step::Handoff => {
                        let info = session.info();
                        to_backdrop(&app, &pids);
                        let _ = app.emit("session", Some(info));
                    }
                    Step::ReleaseTopmost => set_topmost(&app, false),
                    Step::Ended => {
                        let finished = guard.take().unwrap();
                        drop(guard);
                        finish(&app, &state, finished, None);
                        break;
                    }
                    Step::TimedOut => {
                        let finished = guard.take().unwrap();
                        let title = finished.title.clone();
                        drop(guard);
                        finish(&app, &state, finished, Some(format!("{title} não abriu a tempo. Verifique se a loja não está pedindo alguma confirmação.")));
                        break;
                    }
                }
            }
            // Se o jogo trocar de janela (de uma tela de abertura para a principal, por exemplo),
            // o foco pode cair no launcher e deixar o jogo escondido atrás dele.
            let game_window = windows_of(&pids).first().copied();
            if input::mode() == Mode::InGame {
                match game_window {
                    Some(hwnd) if launcher_is_foreground(&app) => force_foreground(hwnd),
                    // A janela do jogo sumiu e o foco foi para outro programa: o launcher assume.
                    None if had_windows && !launcher_is_foreground(&app) => {
                        if let Some(hwnd) = launcher_hwnd(&app) {
                            force_foreground(hwnd);
                        }
                    }
                    _ => {}
                }
            }
            had_windows = game_window.is_some();
        }
    });
}

/// Depois do boot, outros programas (como a Steam) podem abrir por cima do launcher.
pub fn spawn_autostart_guard(app: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let started = Instant::now();
        while started.elapsed() < AUTOSTART_GUARD {
            std::thread::sleep(Duration::from_secs(2));
            let idle = input::mode() == Mode::Launcher && state.session.lock().unwrap().is_none();
            if !idle {
                return;
            }
            if !launcher_is_foreground(&app) {
                if let Some(hwnd) = launcher_hwnd(&app) {
                    force_foreground(hwnd);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn passa_o_foco_quando_a_janela_surge() {
        let t0 = Instant::now();
        let mut t = Tracker::new(t0);
        assert!(t.observe(t0 + secs(1), false, false).is_empty());
        assert!(t.observe(t0 + secs(2), true, false).is_empty());
        assert_eq!(t.phase, Phase::Launching);
        assert_eq!(t.observe(t0 + secs(3), true, true), vec![Step::Handoff]);
        assert_eq!(t.phase, Phase::Playing);
        assert!(t.observe(t0 + secs(4), true, true).is_empty());
    }

    #[test]
    fn sai_do_topo_uma_vez_se_o_jogo_demorar() {
        let t0 = Instant::now();
        let mut t = Tracker::new(t0);
        assert!(t.observe(t0 + secs(14), false, false).is_empty());
        assert_eq!(t.observe(t0 + secs(15), false, false), vec![Step::ReleaseTopmost]);
        assert!(t.observe(t0 + secs(16), false, false).is_empty());
    }

    #[test]
    fn desiste_se_nenhum_processo_surgir() {
        let t0 = Instant::now();
        let mut t = Tracker::new(t0);
        assert_eq!(t.observe(t0 + secs(180), false, false), vec![Step::TimedOut]);
    }

    #[test]
    fn jogo_sem_janela_nao_expira_enquanto_o_processo_existe() {
        let t0 = Instant::now();
        let mut t = Tracker::new(t0);
        t.observe(t0 + secs(16), true, false);
        assert!(t.observe(t0 + secs(300), true, false).is_empty());
        assert_eq!(t.phase, Phase::Launching);
    }

    #[test]
    fn termina_so_depois_de_um_tempo_sem_processos() {
        let t0 = Instant::now();
        let mut t = Tracker::new(t0);
        t.observe(t0 + secs(1), true, true);
        assert!(t.observe(t0 + secs(2), false, false).is_empty());
        assert_eq!(t.observe(t0 + secs(4), false, false), vec![Step::Ended]);
    }

    #[test]
    fn jogo_que_reinicia_a_si_mesmo_nao_encerra_a_sessao() {
        let t0 = Instant::now();
        let mut t = Tracker::new(t0);
        t.observe(t0 + secs(1), true, true);
        assert!(t.observe(t0 + secs(3), false, false).is_empty());
        assert!(t.observe(t0 + secs(4), true, true).is_empty());
        assert!(t.observe(t0 + secs(6), false, false).is_empty());
    }

    #[test]
    fn job_acompanha_o_processo_e_percebe_o_fim() {
        // O cmd abre um ping como filho, sem janela de jogo nem mudança de foco.
        let child = std::process::Command::new("cmd")
            .args(["/c", "ping -n 3 127.0.0.1 >nul"])
            .spawn()
            .unwrap();
        let job = JobHandle(unsafe { CreateJobObjectW(None, PCWSTR::null()) }.unwrap());
        unsafe { AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle() as _)) }.unwrap();
        let target = Target::Job(job);
        std::thread::sleep(Duration::from_millis(500));
        assert!(target_pids(&target).len() >= 2, "deveria ver o cmd e o ping filho");
        std::thread::sleep(Duration::from_secs(4));
        assert!(target_pids(&target).is_empty());
    }

    #[test]
    fn caminho_dentro_da_pasta() {
        assert!(path_in_dir(r"C:\Steam\Common\Half-Life\hl.exe", r"c:\steam\common\half-life"));
        assert!(path_in_dir(r"C:\Jogo\bin\a.exe", "C:/Jogo/"));
    }

    #[test]
    fn pasta_com_prefixo_parecido_nao_casa() {
        assert!(!path_in_dir(r"C:\Jogo2\a.exe", r"C:\Jogo"));
        assert!(!path_in_dir(r"C:\Jogo", r"C:\Jogo"));
        assert!(!path_in_dir(r"C:\Jogo\a.exe", ""));
    }
}
