//! Drawing a sound's waveform the way editors do: a filled shape rising from
//! the bottom of the track, tall where the sound is loud and low where it is
//! quiet, so speech shows its words and pauses.

use egui::{epaint::Mesh, Color32, Painter, Pos2, Rect, Shape, Stroke};

/// How tall the picture of a sound at `level` (0..1 of full scale) is, as a
/// fraction of the room. A little less than linear, so quiet speech still
/// shows without flattening the peaks.
pub fn height_of(level: f32) -> f32 {
    level.clamp(0.0, 1.0).powf(0.7)
}

/// The sound level around `pos` peaks into the list (a position of `a..b`
/// peaks): the loudest peak in the stretch, or when the stretch is narrower
/// than one peak, a blend of its neighbours so close-ups stay smooth.
fn level_at(peaks: &[f32], a: f64, b: f64) -> f32 {
    if peaks.is_empty() {
        return 0.0;
    }
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let last = peaks.len() - 1;
    if b - a < 1.0 {
        let mid = ((a + b) * 0.5 - 0.5).max(0.0);
        let i = (mid.floor() as usize).min(last);
        let f = (mid - i as f64) as f32;
        let next = peaks[(i + 1).min(last)];
        return peaks[i] * (1.0 - f) + next * f;
    }
    let i0 = (a.max(0.0) as usize).min(last);
    let i1 = ((b.ceil().max(0.0)) as usize).clamp(i0 + 1, peaks.len());
    peaks[i0..i1].iter().copied().fold(0.0, f32::max)
}

/// How much a file's peaks are enlarged for drawing: a quiet recording is
/// brought up so its words show, up to eight times; a loud one is left as it
/// is. The loudest peak ends up near the top of the track.
pub fn scale_for(peaks: &[f32]) -> f32 {
    // Large files are read every few peaks: the exact loudest one doesn't matter.
    let step = (peaks.len() / 20_000).max(1);
    let loudest = peaks
        .iter()
        .step_by(step)
        .copied()
        .fold(0.0f32, f32::max)
        .max(0.02);
    (0.92 / loudest).clamp(1.0, 8.0)
}

/// Draws `peaks` (`per_sec` of them for every second of the file) inside
/// `area`: the baseline is the bottom edge and a full-scale sound reaches the
/// top. `time_of` says which second of the file is at a screen x. `gain` is
/// the clip's volume. Returns the enlargement used (see [`scale_for`]), for
/// drawing lines that must agree with the waveform.
pub fn draw(
    painter: &Painter,
    peaks: &[f32],
    per_sec: f64,
    area: Rect,
    time_of: impl Fn(f32) -> f64,
    gain: f32,
    color: Color32,
) -> f32 {
    let scale = scale_for(peaks);
    if peaks.is_empty() || area.width() <= 0.0 || area.height() <= 1.0 {
        return scale;
    }
    let base = area.bottom();
    let room = area.height() - 1.0;
    let mut tops: Vec<Pos2> = Vec::with_capacity(area.width() as usize + 2);
    let mut x = area.left().floor();
    while x <= area.right() + 1.0 {
        let level =
            level_at(peaks, time_of(x) * per_sec, time_of(x + 1.0) * per_sec) * gain * scale;
        tops.push(Pos2::new(x, base - (height_of(level) * room).max(0.6)));
        x += 1.0;
    }
    let mut mesh = Mesh::default();
    for pair in tops.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(Pos2::new(p.x, base), color);
        mesh.colored_vertex(p, color);
        mesh.colored_vertex(q, color);
        mesh.colored_vertex(Pos2::new(q.x, base), color);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    let p = painter.with_clip_rect(area.expand2(egui::vec2(0.0, 1.0)));
    p.add(Shape::mesh(mesh));
    // A brighter edge along the top.
    p.add(Shape::line(
        tops,
        Stroke::new(1.0, color.lerp_to_gamma(Color32::WHITE, 0.35)),
    ));
    scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_sounds_show_but_stay_below_loud_ones() {
        assert_eq!(height_of(0.0), 0.0);
        assert!((height_of(1.0) - 1.0).abs() < 1e-6);
        assert!(height_of(0.05) > 0.05);
        assert!(height_of(0.05) < height_of(0.3));
        assert_eq!(height_of(3.0), 1.0);
    }

    #[test]
    fn quiet_files_are_enlarged_and_loud_ones_left_alone() {
        assert!((scale_for(&[0.05, 0.1, 0.02]) - 8.0).abs() < 1e-6);
        assert!((scale_for(&[0.3, 0.46]) - 2.0).abs() < 1e-6);
        assert_eq!(scale_for(&[0.5, 1.0, 0.2]), 1.0);
        // Silence is not blown up without limit.
        assert_eq!(scale_for(&[0.0; 10]), 8.0);
        assert_eq!(scale_for(&[]), 8.0);
    }

    #[test]
    fn a_stretch_takes_its_loudest_peak() {
        let peaks = [0.1, 0.9, 0.2, 0.3];
        assert_eq!(level_at(&peaks, 0.0, 4.0), 0.9);
        assert_eq!(level_at(&peaks, 2.0, 4.0), 0.3);
        assert_eq!(level_at(&[], 0.0, 4.0), 0.0);
    }

    #[test]
    fn close_up_blends_neighbouring_peaks() {
        let peaks = [0.0, 1.0];
        let mid = level_at(&peaks, 0.75, 0.80);
        assert!(mid > 0.0 && mid < 1.0, "{mid}");
    }
}
