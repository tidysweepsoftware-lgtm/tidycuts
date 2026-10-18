//! The engine between the project model and the screen: decoding, the
//! scrub cache, frame planning, playback, audio mixing and export.

pub mod analysis;
pub mod audio_fx;
pub mod autoduck;
pub mod autograde;
pub mod captions;
pub mod decode;
pub mod detect;
pub mod export;
pub mod frames;
pub mod gentle;
pub mod imagegen;
pub mod imagegen_service;
pub mod level_guard;
pub mod levels;
pub mod limiter;
pub mod loudness;
pub mod mixer;
pub mod plan;
pub mod playback;
pub mod previews;
pub mod proxy;
pub mod proxyfile;
pub mod reframe;
pub mod scopes;
pub mod scrub;
pub mod snapshot;
pub mod stabilize;
pub mod statics;
pub mod stretch;
pub mod studio_voice;
pub mod sync;
pub mod voice_clean;
pub mod whispercpp;

pub use frames::{FrameProvider, FrameServer, PreviewFrames, StreamFrames};
pub use plan::{Planner, PreparedFrame};
pub use playback::Playback;
pub use scrub::ScrubCache;
pub use statics::Statics;
