//! Guarda chaves no Gerenciador de Credenciais do Windows.

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Security::Credentials::*;

/// Chave do SteamGridDB, usada para buscar capas.
pub const ART: &str = "Sofa/SteamGridDB";
/// Chave da API Steam, usada para listar os jogos da conta.
pub const STEAM: &str = "Sofa/SteamWebAPI";

pub fn save(secret: &str) -> Result<(), String> {
    save_to(ART, secret)
}

pub fn load() -> Option<String> {
    load_from(ART)
}

pub fn save_to(target: &str, secret: &str) -> Result<(), String> {
    let target = HSTRING::from(target);
    let mut blob = secret.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_ptr() as *mut u16),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    unsafe { CredWriteW(&credential, 0) }.map_err(|e| e.to_string())
}

pub fn load_from(target: &str) -> Option<String> {
    let target = HSTRING::from(target);
    let mut ptr: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(&target, CRED_TYPE_GENERIC, None, &mut ptr).ok()?;
        let cred = &*ptr;
        let bytes = std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize);
        let value = String::from_utf8_lossy(bytes).into_owned();
        CredFree(ptr as *const _);
        Some(value)
    }
}
