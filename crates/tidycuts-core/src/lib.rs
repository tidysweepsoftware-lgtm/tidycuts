//! Core data model for TidyCuts: projects, media, tracks, clips and time.
//!
//! Nothing in this crate touches the GUI or decodes media, so the same model
//! can be driven by the editor UI and by the MCP server.

pub mod clip;
mod cuts;
pub mod edit;
mod freeze;
pub mod keyframe;
pub mod layout;
pub mod looks;
pub mod marker;
pub mod project;
pub mod speed;
pub mod time;
pub mod visualizer;

pub use clip::{
    AudioSettings, BackgroundFill, BlendMode, Clip, ClipAnimation, ClipBackground, ClipId, Color,
    ColorAdjust, Crop, Effect, Enhance, Generator, Mask, MaskShape, Shadow, StickerSpec, TextAlign,
    TitleSpec, TrackMatte, Transform, Transition, Upscale, VideoSettings, Wheel, Wheels,
};
pub use edit::{Edge, MAX_SPEED, MIN_CLIP, MIN_SPEED};
pub use keyframe::{Animatable, Easing, Keyframe};
pub use layout::{layout, Cell, CellShape, LayoutDef, LAYOUTS};
pub use looks::{ColorLut, Curve, Curves, Hsl, HslBand, LutSource, HSL_RANGES};
pub use marker::{Marker, MarkerId};
pub use project::{
    is_project_file, MediaId, MediaItem, MediaKind, Project, ProjectError, ProjectSettings, Track,
    TrackId, TrackKind, LEGACY_PROJECT_EXTENSIONS, MAX_AUDIO_TRACKS, MAX_VIDEO_TRACKS,
    PROJECT_EXTENSION, USE_PROXIES,
};
pub use speed::{RampPoint, RampPreset, SpeedRamp, RAMP_MAX, RAMP_MIN};
pub use time::{FrameRate, Time};
pub use visualizer::{VisualizerSpec, VISUALIZER_STYLES};
