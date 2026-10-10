//! The engine between the project model and the screen: decoding, the
//! scrub cache, frame planning, playback, audio mixing and export.

pub mod analysis;
pub mod audio_fx;
pub mod decode;
pub mod detect;
pub mod export;
pub mod frames;
pub mod levels;
pub mod mixer;
pub mod plan;
pub mod playback;
pub mod previews;
pub mod proxy;
pub mod scopes;
pub mod scrub;
pub mod snapshot;
pub mod statics;
pub mod stretch;
pub mod sync;

pub use frames::{FrameProvider, FrameServer, PreviewFrames, StreamFrames};
pub use plan::{Planner, PreparedFrame};
pub use playback::Playback;
pub use scrub::ScrubCache;
pub use statics::Statics;
