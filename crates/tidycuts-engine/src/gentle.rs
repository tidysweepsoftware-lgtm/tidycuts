//! "Keep my computer responsive": while exporting, the work runs at a lower
//! priority and pauses a little between pictures, so the window, the mouse
//! and other programs stay smooth. It costs roughly a fifth of the speed.

use std::sync::atomic::{AtomicBool, Ordering};

static GENTLE: AtomicBool = AtomicBool::new(true);

/// Turns the mode on or off (the setting in Preferences).
pub fn set(on: bool) {
    GENTLE.store(on, Ordering::Relaxed);
}

pub fn on() -> bool {
    GENTLE.load(Ordering::Relaxed)
}

/// Lowers the priority of the calling thread below the window's.
pub fn lower_this_thread() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        // SAFETY: both calls only act on the calling thread.
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
        }
    }
}

/// Adds a line to the timing log when `TIDYCUTS_PROFILE` names a file.
pub fn log(text: &str) {
    use std::io::Write;
    let Some(path) = std::env::var_os("TIDYCUTS_PROFILE") else {
        return;
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{text}");
    }
}
