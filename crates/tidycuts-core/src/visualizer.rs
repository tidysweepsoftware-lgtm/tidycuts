//! Audio visualizers (Audio Visualizer effects): bars, lines,
//! rings and dots that move with the sound. A visualizer is a generated
//! clip; its place and size come from the clip's transform.

use crate::clip::{ClipId, Color};
use serde::{Deserialize, Serialize};

/// Drawing styles, as (id, label).
pub const VISUALIZER_STYLES: [(&str, &str); 7] = [
    ("bars", "Bars"),
    ("blocks", "Blocks"),
    ("line", "Line"),
    ("wave", "Wave"),
    ("circle", "Circle"),
    ("ring", "Ring"),
    ("dots", "Dots"),
];

/// Most bars a visualizer can have (one per analysed band).
pub const MAX_BARS: u32 = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerSpec {
    /// One of [`VISUALIZER_STYLES`].
    pub style: String,
    /// Color at the bottom (or start) of the bars.
    pub color: Color,
    /// Color at the top (or end) of the bars.
    pub color2: Color,
    /// How many bars, points or columns, 8..=64.
    pub bars: u32,
    /// How strongly it reacts, percent (100 = normal).
    pub sensitivity: f64,
    /// Grows both ways from the middle instead of up from the bottom.
    pub mirror: bool,
    /// Bar width as a percent of each bar's space (the rest is gap), or the
    /// line width for line styles.
    pub thickness: f64,
    /// The clip whose sound it shows; `None` shows everything you hear.
    pub source: Option<ClipId>,
}

impl Default for VisualizerSpec {
    fn default() -> Self {
        VisualizerSpec {
            style: "bars".into(),
            color: [0.18, 0.85, 0.69, 1.0],
            color2: [0.24, 0.55, 1.0, 1.0],
            bars: 48,
            sensitivity: 100.0,
            mirror: false,
            thickness: 65.0,
            source: None,
        }
    }
}

impl VisualizerSpec {
    /// The style's position in [`VISUALIZER_STYLES`] (0 for unknown ones).
    pub fn style_index(&self) -> u32 {
        VISUALIZER_STYLES
            .iter()
            .position(|(id, _)| *id == self.style)
            .unwrap_or(0) as u32
    }

    pub fn style_label(&self) -> &'static str {
        VISUALIZER_STYLES[self.style_index() as usize].1
    }

    /// Whether the style is drawn round (and so in a square box).
    pub fn is_round(&self) -> bool {
        matches!(self.style.as_str(), "circle" | "ring")
    }

    /// Size at 100% scale, in pixels of a 1080-pixel-high frame.
    pub fn base_size(&self) -> [f64; 2] {
        if self.is_round() {
            [720.0, 720.0]
        } else {
            [1500.0, 420.0]
        }
    }
}
