//! The Windows voices, used directly through the system's speech interface (SAPI).
//!
//! No other program is started for this: some computers (a locked-down work or
//! test computer, Windows in S mode) do not let an app start PowerShell, or run
//! it in a mode where it cannot load the speech parts.

use crate::speech::Voice;
use std::path::Path;
use windows::core::{Interface, GUID, PCWSTR, PWSTR};
use windows::Win32::Media::Audio::WAVEFORMATEX;
use windows::Win32::Media::Speech::{
    ISpObjectToken, ISpObjectTokenCategory, ISpStream, ISpVoice, SpObjectTokenCategory, SpStream,
    SpVoice, SPCAT_VOICES, SPFM_CREATE_ALWAYS, SPF_IS_NOT_XML,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};

/// The kind of sound file to write: a plain wave file (SPDFID_WaveFormat).
const WAVE_FORMAT_ID: GUID = GUID::from_u128(0xC31ADBAE_527F_4FF5_A230_F62BB61FF70C);

/// Gets COM ready on this thread (harmless when it already is).
fn init() {
    // SAFETY: plain COM start-up on the calling thread; an "already started"
    // answer is fine and the result is not needed.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Takes the text out of a string the system allocated, and frees it.
fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: `p` came from the speech interface and is a valid, nul-ended string.
    let s = unsafe { p.to_string().unwrap_or_default() };
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// The name of a voice token.
fn token_name(t: &ISpObjectToken) -> String {
    // SAFETY: valid token; asking for its default value, which is its name.
    unsafe { take(t.GetStringValue(PCWSTR::null()).unwrap_or(PWSTR::null())) }
}

/// A language code such as `en_US` from the voice's "Language" attribute
/// (a hexadecimal Windows language number like `409`, or several with `;`).
fn token_language(t: &ISpObjectToken) -> String {
    // SAFETY: valid token; reads a text value of its Attributes key.
    unsafe {
        let Ok(key) = t.OpenKey(PCWSTR(wide("Attributes").as_ptr())) else {
            return String::new();
        };
        let lang = take(
            key.GetStringValue(PCWSTR(wide("Language").as_ptr()))
                .unwrap_or(PWSTR::null()),
        );
        let first = lang.split(';').next().unwrap_or("").trim().to_string();
        match u32::from_str_radix(&first, 16) {
            // The main languages people look for first.
            Ok(0x409) => "en_US".into(),
            Ok(0x809) => "en_GB".into(),
            Ok(n) if n & 0xff == 0x09 => "en".into(),
            Ok(n) => format!("lcid_{n:x}"),
            Err(_) => String::new(),
        }
    }
}

fn tokens() -> Result<Vec<ISpObjectToken>, String> {
    init();
    // SAFETY: standard SAPI enumeration of the installed voices.
    unsafe {
        let cat: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_ALL)
                .map_err(|e| format!("The Windows speech system is not available ({e})."))?;
        cat.SetId(SPCAT_VOICES, false)
            .map_err(|e| format!("Could not look for voices ({e})."))?;
        let list = cat
            .EnumTokens(PCWSTR::null(), PCWSTR::null())
            .map_err(|e| format!("Could not list the voices ({e})."))?;
        let mut count = 0u32;
        list.GetCount(&mut count).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for i in 0..count {
            if let Ok(t) = list.Item(i) {
                out.push(t);
            }
        }
        Ok(out)
    }
}

/// The voices installed on this computer.
pub fn voices() -> Result<Vec<Voice>, String> {
    Ok(tokens()?
        .iter()
        .map(|t| {
            // The token's name ends with its language ("Microsoft David Desktop - English
            // (United States)"): the id keeps all of it, the name shown is the short part.
            let id = token_name(t);
            let name = id.split(" - ").next().unwrap_or(&id).to_string();
            Voice {
                id,
                name,
                language: token_language(t),
            }
        })
        .filter(|v| !v.id.is_empty())
        .collect())
}

/// Speaks `text` into a wave file. `rate` runs from -10 (slow) to 10 (fast).
pub fn speak(text: &str, voice: &str, rate: i32, out: &Path) -> Result<(), String> {
    init();
    // SAFETY: standard SAPI use; every interface is released when it goes out of scope.
    unsafe {
        let v: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL)
            .map_err(|e| format!("The Windows speech system is not available ({e})."))?;
        if !voice.is_empty() {
            if let Some(t) = tokens()?.into_iter().find(|t| token_name(t) == voice) {
                v.SetVoice(&t)
                    .map_err(|e| format!("Could not use the voice {voice} ({e})."))?;
            }
        }
        v.SetRate(rate).map_err(|e| e.to_string())?;
        let stream: ISpStream = CoCreateInstance(&SpStream, None, CLSCTX_ALL)
            .map_err(|e| format!("Could not make the sound file ({e})."))?;
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 1,
            nSamplesPerSec: 22_050,
            nAvgBytesPerSec: 44_100,
            nBlockAlign: 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let path = wide(&out.to_string_lossy());
        stream
            .BindToFile(
                PCWSTR(path.as_ptr()),
                SPFM_CREATE_ALWAYS,
                Some(&WAVE_FORMAT_ID),
                Some(&format),
                0,
            )
            .map_err(|e| format!("Could not open the sound file ({e})."))?;
        v.SetOutput(
            &stream
                .cast::<windows::core::IUnknown>()
                .map_err(|e| e.to_string())?,
            true,
        )
        .map_err(|e| format!("Could not send the voice to the file ({e})."))?;
        let t = wide(text);
        v.Speak(PCWSTR(t.as_ptr()), SPF_IS_NOT_XML.0 as u32, None)
            .map_err(|e| format!("The voice could not speak ({e})."))?;
        v.WaitUntilDone(u32::MAX)
            .map_err(|e| format!("The voice did not finish ({e})."))?;
        let _ = stream.Close();
    }
    Ok(())
}
