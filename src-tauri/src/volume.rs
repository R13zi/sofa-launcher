//! Volume geral do Windows, no dispositivo de saída padrão (a TV, no PC da sala).
//!
//! Usa a API de áudio do Windows diretamente: ela não mostra o balão de volume do sistema,
//! então nada aparece por cima do jogo além do menu do Sofa.

use serde::Serialize;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct VolumeState {
    /// De 0 a 100.
    pub level: u8,
    pub muted: bool,
}

/// Valor da interface (0 a 100) para o que o Windows espera (0.0 a 1.0).
pub fn to_scalar(level: i32) -> f32 {
    level.clamp(0, 100) as f32 / 100.0
}

/// O inverso, arredondado: 0.625 vira 63.
pub fn from_scalar(scalar: f32) -> u8 {
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

const NO_OUTPUT: &str = "Sem saída de áudio";

fn with_endpoint<T>(f: impl FnOnce(&IAudioEndpointVolume) -> windows::core::Result<T>) -> Result<T, String> {
    // A thread pode já ter o COM iniciado em outro modo (a principal, por exemplo); nesse caso
    // as chamadas funcionam do mesmo jeito, e só não há o que encerrar no fim.
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    // Os objetos COM são soltos dentro do bloco, antes de encerrar o COM.
    let result = (|| unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        f(&endpoint)
    })();
    if initialized {
        unsafe { CoUninitialize() };
    }
    result.map_err(|_| NO_OUTPUT.to_string())
}

pub fn get() -> Result<VolumeState, String> {
    with_endpoint(|endpoint| unsafe {
        Ok(VolumeState {
            level: from_scalar(endpoint.GetMasterVolumeLevelScalar()?),
            muted: endpoint.GetMute()?.as_bool(),
        })
    })
}

pub fn set_level(level: i32) -> Result<(), String> {
    with_endpoint(|endpoint| unsafe { endpoint.SetMasterVolumeLevelScalar(to_scalar(level), std::ptr::null()) })
}

pub fn set_mute(muted: bool) -> Result<(), String> {
    with_endpoint(|endpoint| unsafe { endpoint.SetMute(muted, std::ptr::null()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limita_o_volume_de_0_a_100() {
        assert_eq!(to_scalar(-5), 0.0);
        assert_eq!(to_scalar(62), 0.62);
        assert_eq!(to_scalar(140), 1.0);
    }

    #[test]
    fn converte_o_valor_do_windows_arredondando() {
        assert_eq!(from_scalar(0.625), 63);
        assert_eq!(from_scalar(0.0), 0);
        assert_eq!(from_scalar(1.2), 100);
        assert_eq!(from_scalar(to_scalar(37)), 37);
    }

    /// Usa o áudio de verdade: `cargo test volume_real -- --ignored`. Regrava o mesmo valor,
    /// então o volume não muda.
    #[test]
    #[ignore]
    fn volume_real() {
        let before = get().expect("ler o volume");
        set_level(before.level as i32).expect("gravar o volume");
        set_mute(before.muted).expect("gravar o mudo");
        assert_eq!(get().unwrap(), before);
        println!("volume atual: {before:?}");
    }
}
