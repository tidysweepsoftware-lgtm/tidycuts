//! Keeps the program's last warnings and errors, so that "Copy details" in the About box can hand
//! them over in a problem report (the program has no console window to read them from).

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// How many lines are kept.
const KEEP: usize = 80;

static LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

/// Sends everything to the usual logger and also remembers warnings and errors.
struct Tee {
    inner: env_logger::Logger,
}

impl log::Log for Tee {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.inner.enabled(metadata) || metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        self.inner.log(record);
        if record.level() <= log::Level::Warn {
            let secs = START.get_or_init(Instant::now).elapsed().as_secs();
            let mut text = format!(
                "[+{secs}s] {} {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
            // One long message must not push everything else out.
            if text.len() > 600 {
                let mut end = 600;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
                text.push('…');
            }
            let lines = LINES.get_or_init(|| Mutex::new(VecDeque::new()));
            let mut lines = lines.lock().unwrap_or_else(|e| e.into_inner());
            if lines.len() >= KEEP {
                lines.pop_front();
            }
            lines.push_back(text);
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Starts logging (warnings and errors by default; `RUST_LOG` changes that).
pub fn init_logging() {
    START.get_or_init(Instant::now);
    let inner =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).build();
    log::set_max_level(inner.filter().max(log::LevelFilter::Warn));
    let _ = log::set_boxed_logger(Box::new(Tee { inner }));
}

/// The remembered warnings and errors, oldest first.
pub fn recent() -> Vec<String> {
    LINES
        .get()
        .map(|l| {
            l.lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
