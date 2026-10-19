//! Text to speech with the voices already on the computer: SAPI on Windows,
//! `say` on macOS and eSpeak NG on Linux. Nothing is downloaded, nothing
//! leaves the computer, and there is nothing to license.
//!
//! The text goes to the speech program through its input or an environment
//! variable, never through a command line or a script, so what someone types
//! can't be run as a command.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// A voice that can be picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    /// What the speech program calls it.
    pub id: String,
    /// What the person sees.
    pub name: String,
    /// A language code such as `en` or `en_US`, when known.
    pub language: String,
}

/// The file extension the voice program writes.
pub fn extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "aiff"
    } else {
        "wav"
    }
}

fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    crate::ffmpeg::no_window(&mut cmd);
    cmd
}

/// The voices installed on this computer, English ones first. Empty when the
/// system has no speech program.
pub fn voices() -> Vec<Voice> {
    let mut list = if cfg!(windows) {
        // The system's speech interface first: it starts no other program. PowerShell
        // is the fallback for a computer where that does not work.
        #[cfg(windows)]
        {
            if let Ok(found) = crate::sapi::voices() {
                if !found.is_empty() {
                    let mut found = found;
                    found.sort_by_key(|v| (!v.language.starts_with("en"), v.name.to_lowercase()));
                    return found;
                }
            }
        }
        let script = "Add-Type -AssemblyName System.Speech; \
            $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
            $s.GetInstalledVoices() | ForEach-Object { $_.VoiceInfo.Name + '|' + $_.VoiceInfo.Culture.Name }";
        let out = command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output();
        out.map(|o| parse_windows_voices(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    } else if cfg!(target_os = "macos") {
        command("say")
            .args(["-v", "?"])
            .output()
            .map(|o| parse_say_voices(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    } else {
        ["espeak-ng", "espeak"]
            .iter()
            .find_map(|p| {
                command(p)
                    .arg("--voices")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
            })
            .map(|o| parse_espeak_voices(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    };
    // English first, then by name.
    list.sort_by_key(|v| (!v.language.starts_with("en"), v.name.to_lowercase()));
    list
}

/// Lines like `Microsoft David Desktop|en-US`.
pub fn parse_windows_voices(text: &str) -> Vec<Voice> {
    text.lines()
        .filter_map(|l| {
            let (name, lang) = l.trim().split_once('|')?;
            (!name.is_empty()).then(|| Voice {
                id: name.to_string(),
                name: name.to_string(),
                language: lang.replace('-', "_"),
            })
        })
        .collect()
}

/// Lines like `Alex                en_US    # Most people recognize me by my voice.`
pub fn parse_say_voices(text: &str) -> Vec<Voice> {
    text.lines()
        .filter_map(|l| {
            let head = l.split('#').next()?.trim_end();
            // The language code is the last word before the comment.
            let (name, lang) = head.rsplit_once(char::is_whitespace)?;
            let name = name.trim();
            (!name.is_empty() && lang.contains('_')).then(|| Voice {
                id: name.to_string(),
                name: name.to_string(),
                language: lang.to_string(),
            })
        })
        .collect()
}

/// The table `espeak-ng --voices` prints:
/// `Pty Language Age/Gender VoiceName File Other Languages`.
pub fn parse_espeak_voices(text: &str) -> Vec<Voice> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            if cols.len() < 5 {
                return None;
            }
            Some(Voice {
                id: cols[1].to_string(),
                name: cols[3].replace('_', " "),
                language: cols[1].to_string(),
            })
        })
        .collect()
}

/// Speaks `text` into the audio file `out` with `voice` (empty for the
/// system's own). `speed` is a factor, 1.0 being normal.
pub fn speak(text: &str, voice: &str, speed: f64, out: &Path) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("There is nothing to say yet.".into());
    }
    let speed = speed.clamp(0.5, 2.5);
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not make the folder: {e}"))?;
    }
    let result = if cfg!(windows) {
        speak_windows(text, voice, speed, out)
    } else if cfg!(target_os = "macos") {
        let mut cmd = command("say");
        if !voice.is_empty() {
            cmd.args(["-v", voice]);
        }
        cmd.args(["-r", &format!("{:.0}", 175.0 * speed), "-o"])
            .arg(out);
        cmd.args(["-f", "-"]);
        run_with_input(cmd, text)
    } else {
        let program = ["espeak-ng", "espeak"]
            .into_iter()
            .find(|p| command(p).arg("--version").output().is_ok())
            .ok_or("No speech program was found. Install eSpeak NG (espeak-ng) to make voiceovers on this computer.")?;
        let mut cmd = command(program);
        if !voice.is_empty() {
            cmd.args(["-v", voice]);
        }
        cmd.args(["-s", &format!("{:.0}", 175.0 * speed), "-w"])
            .arg(out);
        cmd.arg("--stdin");
        run_with_input(cmd, text)
    };
    result?;
    if std::fs::metadata(out)
        .map(|m| m.len() > 44)
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err("The voice produced no sound.".into())
    }
}

fn run_with_input(mut cmd: Command, text: &str) -> Result<(), String> {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start the speech program: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("Could not give it the text: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("The speech program stopped: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "The speech program failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The text, voice and file travel in environment variables, so none of them
/// is ever part of the script.
fn speak_windows(text: &str, voice: &str, speed: f64, out: &Path) -> Result<(), String> {
    let first = {
        #[cfg(windows)]
        {
            crate::sapi::speak(text, voice, windows_rate(speed), out)
        }
        #[cfg(not(windows))]
        {
            Err::<(), String>(String::new())
        }
    };
    if first.is_ok()
        && std::fs::metadata(out)
            .map(|m| m.len() > 44)
            .unwrap_or(false)
    {
        return Ok(());
    }
    if let Err(e) = &first {
        log::warn!("speech through SAPI failed ({e}); trying PowerShell");
    }
    let script = "Add-Type -AssemblyName System.Speech; \
        $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
        if ($env:TC_VOICE) { $s.SelectVoice($env:TC_VOICE) }; \
        $s.Rate = [int]$env:TC_RATE; \
        $s.SetOutputToWaveFile($env:TC_OUT); \
        $s.Speak($env:TC_TEXT); \
        $s.Dispose()";
    let rate = windows_rate(speed);
    let mut cmd = command("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("TC_TEXT", text)
        .env("TC_VOICE", voice)
        .env("TC_RATE", rate.to_string())
        .env("TC_OUT", out);
    run_with_input(cmd, "")
}

/// SAPI's rate runs from -10 (slow) to 10 (fast); 0 is normal.
pub fn windows_rate(speed: f64) -> i32 {
    let r = if speed >= 1.0 {
        (speed - 1.0) * 5.0
    } else {
        (speed - 1.0) * 10.0
    };
    (r.round() as i32).clamp(-10, 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_lists_are_read_from_each_system() {
        let w = parse_windows_voices("Microsoft David Desktop|en-US\nMicrosoft Hedda|de-DE\n\n");
        assert_eq!(w.len(), 2);
        assert_eq!(
            (w[0].name.as_str(), w[0].language.as_str()),
            ("Microsoft David Desktop", "en_US")
        );

        let m = parse_say_voices(
            "Alex                en_US    # Most people recognize me by my voice.\n\
             Bad News            en_US    # The light you see at the end of the tunnel.\n\
             Anna                de_DE    # Hallo! Ich heiße Anna.\n",
        );
        assert_eq!(m.len(), 3);
        assert_eq!(
            (m[1].name.as_str(), m[1].language.as_str()),
            ("Bad News", "en_US")
        );

        let e = parse_espeak_voices(
            "Pty Language       Age/Gender VoiceName          File                 Other Languages\n \
             5  en-gb           --/M      English_(Great_Britain) gmw/en-GB            (en 2)\n \
             5  de              --/M      German             gmw/de               \n",
        );
        assert_eq!(e.len(), 2);
        assert_eq!(
            (e[0].id.as_str(), e[0].name.as_str()),
            ("en-gb", "English (Great Britain)")
        );
    }

    #[test]
    fn speed_maps_to_each_systems_scale() {
        assert_eq!(windows_rate(1.0), 0);
        assert_eq!(windows_rate(2.0), 5);
        assert_eq!(windows_rate(0.5), -5);
        assert_eq!(windows_rate(100.0), 10);
        assert_eq!(windows_rate(0.01), -10);
    }

    #[test]
    fn nothing_to_say_is_refused_and_text_is_never_run() {
        assert!(speak("   ", "", 1.0, Path::new("/tmp/never.wav")).is_err());
    }

    #[test]
    fn the_system_voice_makes_a_sound_file() {
        if cfg!(windows) || cfg!(target_os = "macos") || voices().is_empty() {
            eprintln!("no speech program here; skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("tidycuts-speech-{}", std::process::id()));
        let out = dir.join(format!("hello.{}", extension()));
        // Quotes, dollar signs and shell characters are just words.
        speak(
            "Hello \"world\"; $(touch /tmp/pwned) `id` done.",
            "en",
            1.0,
            &out,
        )
        .unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 2000);
        assert!(!Path::new("/tmp/pwned").exists());
        let slow = dir.join("slow.wav");
        let fast = dir.join("fast.wav");
        speak("The same sentence for both.", "en", 0.6, &slow).unwrap();
        speak("The same sentence for both.", "en", 1.8, &fast).unwrap();
        assert!(
            std::fs::metadata(&slow).unwrap().len() > std::fs::metadata(&fast).unwrap().len() * 2
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
