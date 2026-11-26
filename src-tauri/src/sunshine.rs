//! Cadastro do Sofa no Sunshine, o servidor do Moonlight, para abrir o Sofa pela TV.
//!
//! A lista de apps do Sunshine fica em `Program Files` e só muda como administrador. Por isso o
//! Sofa abre uma cópia de si mesmo com a permissão do Windows, que grava a lista e sai.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "Sofa";
const REGISTER_ARG: &str = "--sunshine-register";
const UNREGISTER_ARG: &str = "--sunshine-unregister";
/// O Sunshine abre o Sofa com este argumento: essa cópia só acompanha o Sofa de verdade.
const WATCH_ARG: &str = "--moonlight";
const COVER: &[u8] = include_bytes!("../assets/sunshine-cover.png");
const COVER_FILE: &str = "sofa-cover.png";
const SERVICE: &str = "SunshineService";

#[derive(Serialize, Debug, PartialEq)]
pub struct Status {
    pub installed: bool,
    pub registered: bool,
}

/// Pasta do Sunshine a partir do executável do serviço (`...\Sunshine\tools\sunshinesvc.exe`).
fn dir_from_service(image_path: &str) -> Option<PathBuf> {
    let exe = PathBuf::from(image_path.trim().trim_matches('"'));
    let parent = exe.parent()?;
    let is_tools = parent.file_name().is_some_and(|n| n.eq_ignore_ascii_case("tools"));
    Some(if is_tools { parent.parent()?.to_path_buf() } else { parent.to_path_buf() })
}

pub fn dir() -> Option<PathBuf> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let from_service = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(r"SYSTEM\CurrentControlSet\Services\{SERVICE}"))
        .and_then(|k| k.get_value::<String, _>("ImagePath"))
        .ok()
        .and_then(|p| dir_from_service(&p));
    from_service
        .into_iter()
        .chain([PathBuf::from(r"C:\Program Files\Sunshine")])
        .find(|d| d.join("config").is_dir())
}

fn parse(text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(json!({ "apps": [], "env": {} }));
    }
    let doc: Value = serde_json::from_str(text).map_err(|e| format!("A lista de apps do Sunshine está corrompida: {e}"))?;
    if !doc.get("apps").is_some_and(Value::is_array) {
        return Err("A lista de apps do Sunshine não tem o formato esperado.".into());
    }
    Ok(doc)
}

fn apps_mut(doc: &mut Value) -> &mut Vec<Value> {
    doc.get_mut("apps").and_then(Value::as_array_mut).expect("conferido em parse")
}

fn is_sofa(app: &Value) -> bool {
    app.get("name").and_then(Value::as_str) == Some(APP_NAME)
}

fn command(exe: &str) -> String {
    format!("\"{exe}\" {WATCH_ARG}")
}

/// Põe (ou troca) o item do Sofa, mantendo os outros apps e o resto do arquivo.
/// O Sunshine abre a cópia vigia (`--moonlight`) e encerra a transmissão quando ela termina,
/// ou seja, quando o Sofa fecha. Assim o aparelho volta para o Moonlight em vez de mostrar o desktop.
pub fn upsert_app(text: &str, exe: &str, image: &str) -> Result<String, String> {
    let mut doc = parse(text)?;
    let apps = apps_mut(&mut doc);
    apps.retain(|a| !is_sofa(a));
    apps.push(json!({ "name": APP_NAME, "image-path": image, "cmd": command(exe), "auto-detach": false }));
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

pub fn remove_app(text: &str) -> Result<String, String> {
    let mut doc = parse(text)?;
    apps_mut(&mut doc).retain(|a| !is_sofa(a));
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

/// Executável que o item do Sofa abre, se ele existe no formato atual (com a cópia vigia).
pub fn registered_exe(text: &str) -> Option<String> {
    let doc = parse(text).ok()?;
    let app = doc.get("apps")?.as_array()?.iter().find(|a| is_sofa(a))?;
    let exe = app.get("cmd")?.as_str()?.strip_suffix(WATCH_ARG)?;
    Some(exe.trim().trim_matches('"').to_string())
}

fn same_path(a: &str, b: &Path) -> bool {
    a.eq_ignore_ascii_case(&b.to_string_lossy())
}

pub fn status() -> Status {
    let Some(dir) = dir() else { return Status { installed: false, registered: false } };
    let text = std::fs::read_to_string(dir.join("config").join("apps.json")).unwrap_or_default();
    let exe = std::env::current_exe().ok();
    // Reinstalado em outra pasta, o item antigo abriria o exe errado: conta como não cadastrado.
    let registered = registered_exe(&text).zip(exe).is_some_and(|(e, current)| same_path(&e, &current));
    Status { installed: true, registered }
}

// ---- Cópia como administrador ----

// Códigos de saída da cópia de administrador.
const OK: i32 = 0;
const NOT_FOUND: i32 = 2;
const BAD_LIST: i32 = 3;
const WRITE_FAILED: i32 = 4;
const RESTART_FAILED: i32 = 5;

fn exit_message(code: u32) -> String {
    match code as i32 {
        NOT_FOUND => "O Sunshine não foi encontrado neste PC.".into(),
        BAD_LIST => "A lista de apps do Sunshine não pôde ser lida. Confira no painel do Sunshine.".into(),
        WRITE_FAILED => "Não foi possível gravar a lista de apps do Sunshine.".into(),
        RESTART_FAILED => "A lista foi gravada, mas o Sunshine não reiniciou. Reinicie o PC ou o Sunshine para o Sofa aparecer no Moonlight.".into(),
        other => format!("O cadastro no Sunshine falhou (código {other})."),
    }
}

/// Grava num arquivo temporário e troca pelo original, para não deixar a lista pela metade.
fn write_replace(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("json.sofa-tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

fn restart_service() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &format!("Restart-Service -Name {SERVICE} -Force -ErrorAction Stop")])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success())
}

fn admin_task(register: bool) -> i32 {
    let Some(dir) = dir() else { return NOT_FOUND };
    let config = dir.join("config");
    let list = config.join("apps.json");
    let text = std::fs::read_to_string(&list).unwrap_or_default();
    let new = if register {
        let cover = config.join(COVER_FILE);
        if std::fs::write(&cover, COVER).is_err() {
            return WRITE_FAILED;
        }
        let Ok(exe) = std::env::current_exe() else { return WRITE_FAILED };
        upsert_app(&text, &exe.to_string_lossy(), &cover.to_string_lossy())
    } else {
        let _ = std::fs::remove_file(config.join(COVER_FILE));
        remove_app(&text)
    };
    let Ok(new) = new else { return BAD_LIST };
    // Cópia de segurança da lista original, só na primeira vez.
    let backup = config.join("apps.json.antes-do-sofa");
    if !text.is_empty() && !backup.exists() {
        let _ = std::fs::write(&backup, &text);
    }
    if write_replace(&list, &new).is_err() {
        return WRITE_FAILED;
    }
    if restart_service() { OK } else { RESTART_FAILED }
}

/// Chamada no começo do `main`: com `--sunshine-register` ou `--sunshine-unregister`, faz o
/// cadastro e devolve o código de saída, sem abrir a janela do Sofa.
pub fn run_admin_task(args: impl Iterator<Item = String>) -> Option<i32> {
    let register = match args.skip(1).next().as_deref() {
        Some(REGISTER_ARG) => true,
        Some(UNREGISTER_ARG) => false,
        _ => return None,
    };
    let code = admin_task(register);
    crate::library::debug_log(&format!("Sunshine: {} terminou com código {code}", if register { "cadastro" } else { "remoção" }));
    Some(code)
}

// ---- Cópia vigia do Moonlight ----

/// O Sofa aberto segura este mutex (veja `main`); ele some quando o Sofa fecha.
fn sofa_running() -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, crate::INSTANCE_MUTEX) } {
        Ok(handle) => {
            let _ = unsafe { CloseHandle(handle) };
            true
        }
        Err(_) => false,
    }
}

/// Abre o Sofa fora do grupo de processos do Sunshine, para "sair do app" no Moonlight não fechá-lo.
fn spawn_sofa() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let Ok(exe) = std::env::current_exe() else { return false };
    std::process::Command::new(&exe)
        .creation_flags(CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
        .or_else(|_| std::process::Command::new(&exe).spawn())
        .is_ok()
}

/// Com `--moonlight`: abre o Sofa (ou traz o que já está aberto para a frente) e espera ele
/// fechar. Quando esta cópia termina, o Sunshine encerra a transmissão e o aparelho volta para
/// a lista do Moonlight, em vez de mostrar o desktop.
pub fn run_moonlight_watch(args: impl Iterator<Item = String>, bring_to_front: impl FnOnce()) -> Option<i32> {
    use std::time::{Duration, Instant};
    if args.skip(1).next().as_deref() != Some(WATCH_ARG) {
        return None;
    }
    if sofa_running() {
        bring_to_front();
    } else {
        if !spawn_sofa() {
            crate::library::debug_log("Moonlight: não foi possível abrir o Sofa");
            return Some(1);
        }
        let started = Instant::now();
        while !sofa_running() {
            if started.elapsed() > Duration::from_secs(60) {
                crate::library::debug_log("Moonlight: o Sofa não abriu");
                return Some(1);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    crate::library::debug_log("Moonlight: acompanhando o Sofa");
    while sofa_running() {
        std::thread::sleep(Duration::from_millis(500));
    }
    crate::library::debug_log("Moonlight: o Sofa fechou, encerrando a transmissão");
    Some(0)
}

/// Abre a cópia de administrador (o Windows pede a permissão) e espera ela terminar.
pub fn request(register: bool) -> Result<(), String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    if dir().is_none() {
        return Err(exit_message(NOT_FOUND as u32));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let file = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(if register { REGISTER_ARG } else { UNREGISTER_ARG });
    let verb = HSTRING::from("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        if let Err(e) = ShellExecuteExW(&mut info) {
            return Err(if e.code() == ERROR_CANCELLED.to_hresult() {
                "O Windows não deu a permissão, então nada foi alterado.".into()
            } else {
                format!("Não foi possível pedir a permissão do Windows: {e}")
            });
        }
        if info.hProcess.is_invalid() {
            return Err("Não foi possível acompanhar o cadastro no Sunshine.".into());
        }
        // A permissão e o reinício do Sunshine levam alguns segundos.
        WaitForSingleObject(info.hProcess, 120_000);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if code == OK as u32 { Ok(()) } else { Err(exit_message(code)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = r#"{
    "apps": [
        { "image-path": "desktop.png", "name": "Desktop" },
        { "auto-detach": true, "cmd": "steam://open/bigpicture", "image-path": "steam.png", "name": "Steam Big Picture",
          "prep-cmd": [ { "do": "", "undo": "steam://close/bigpicture" } ], "wait-all": true },
        { "cmd": "\"C:\\Users\\Usuario\\Documents\\WindWakerHD\\Wind Waker HD.exe\"", "name": "Wind Waker HD" }
    ],
    "env": {}
}"#;

    fn names(text: &str) -> Vec<String> {
        let doc: Value = serde_json::from_str(text).unwrap();
        doc["apps"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn cadastra_mantendo_os_outros_apps() {
        let out = upsert_app(REAL, r"C:\Sofa\sofa.exe", r"C:\Sunshine\config\sofa-cover.png").unwrap();
        assert_eq!(names(&out), ["Desktop", "Steam Big Picture", "Wind Waker HD", "Sofa"]);
        assert_eq!(registered_exe(&out).as_deref(), Some(r"C:\Sofa\sofa.exe"));
        let doc: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(doc["apps"][1]["prep-cmd"][0]["undo"], "steam://close/bigpicture");
        assert_eq!(doc["apps"][3]["image-path"], r"C:\Sunshine\config\sofa-cover.png");
        assert_eq!(doc["apps"][3]["cmd"], r#""C:\Sofa\sofa.exe" --moonlight"#);
    }

    #[test]
    fn troca_um_cadastro_antigo() {
        let old = upsert_app(REAL, r"D:\antigo\sofa.exe", "x.png").unwrap();
        let out = upsert_app(&old, r"C:\Sofa\sofa.exe", "x.png").unwrap();
        assert_eq!(names(&out).iter().filter(|n| *n == "Sofa").count(), 1);
        assert_eq!(registered_exe(&out).as_deref(), Some(r"C:\Sofa\sofa.exe"));
    }

    #[test]
    fn remove_so_o_sofa() {
        let with = upsert_app(REAL, r"C:\Sofa\sofa.exe", "x.png").unwrap();
        let out = remove_app(&with).unwrap();
        assert_eq!(names(&out), ["Desktop", "Steam Big Picture", "Wind Waker HD"]);
        assert_eq!(registered_exe(&out), None);
    }

    #[test]
    fn lista_vazia_ou_invalida() {
        assert_eq!(names(&upsert_app("", r"C:\s.exe", "x.png").unwrap()), ["Sofa"]);
        assert!(upsert_app("{ quebrado", r"C:\s.exe", "x.png").is_err());
        assert!(upsert_app(r#"{"apps": 3}"#, r"C:\s.exe", "x.png").is_err());
    }

    #[test]
    fn exe_em_outra_pasta_nao_conta() {
        let out = upsert_app(REAL, r"D:\antigo\sofa.exe", "x.png").unwrap();
        assert!(!same_path(&registered_exe(&out).unwrap(), Path::new(r"C:\Sofa\sofa.exe")));
        assert!(same_path(r"c:\sofa\SOFA.exe", Path::new(r"C:\Sofa\sofa.exe")));
    }

    #[test]
    fn cadastro_antigo_sem_vigia_conta_como_nao_cadastrado() {
        let old = r#"{"apps": [{"name": "Sofa", "detached": ["\"C:\\Sofa\\sofa.exe\""]}]}"#;
        assert!(parse(old).is_ok());
        assert_eq!(registered_exe(old), None);
    }

    #[test]
    fn pasta_a_partir_do_servico() {
        assert_eq!(
            dir_from_service(r#""C:\Program Files\Sunshine\tools\sunshinesvc.exe""#),
            Some(PathBuf::from(r"C:\Program Files\Sunshine"))
        );
        assert_eq!(dir_from_service(r"D:\Sun\sunshine.exe"), Some(PathBuf::from(r"D:\Sun")));
    }

    #[test]
    fn capa_e_png() {
        assert_eq!(&COVER[1..4], b"PNG");
    }
}
