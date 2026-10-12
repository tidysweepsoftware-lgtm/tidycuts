//! GPU rendering for TidyCuts: the compositor that draws every frame of the
//! preview and the export, the built-in catalogs (effects, filters,
//! transitions, titles, stickers) and the CPU rasterizers for text and
//! stickers.

pub mod catalog;
mod compositor;
pub mod fonts;
mod frame;
pub mod gpu;
pub mod looks;
mod looks_gpu;
pub mod lut;
pub mod motion;
pub mod motion_blur;
pub mod noise;
pub mod particles;
pub mod sample;
pub mod sdf;
pub mod text;

pub use compositor::{Compositor, Nv12Target, Renderer, Target};
pub use frame::*;

/// The complete WGSL source, for validation in tests.
pub fn shader_source() -> &'static str {
    compositor::SHADER_SOURCE
}
