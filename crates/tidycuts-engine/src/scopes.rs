//! Video scopes: graphs of the colours in a picture, as colorists use them.
//! The waveform and parade show how bright the picture is across its width,
//! the vectorscope which colours it holds and how strong they are, and the
//! histogram how many pixels sit at each level. All are counted from the
//! picture as it is shown (gamma-encoded RGB).

/// Columns of the waveform and of each parade channel.
pub const WAVE_COLS: usize = 320;
/// Rows of the waveform and parade (one per level, top is 100%).
pub const LEVELS: usize = 256;

/// Counts for every scope of one picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Scopes {
    pub width: u32,
    pub height: u32,
    /// Luma waveform: `LEVELS` rows of `WAVE_COLS` counts, top row = white.
    pub luma: Vec<u32>,
    /// The same for red, green and blue (the parade).
    pub red: Vec<u32>,
    pub green: Vec<u32>,
    pub blue: Vec<u32>,
    /// Vectorscope: 256 by 256 counts, blue-yellow across (blue right) and
    /// red-cyan up (red up).
    pub vector: Vec<u32>,
    /// Histogram: luma, red, green and blue, 256 bins each.
    pub hist: [[u32; 256]; 4],
}

/// BT.709 luma and the two colour differences, each -0.5..=0.5 for the
/// differences, from gamma-encoded 0..1 RGB.
pub fn ycbcr(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    (y, (b - y) / 1.8556, (r - y) / 1.5748)
}

/// Where a colour lands on the vectorscope, as fractions of its width and
/// height from the top left.
pub fn vector_position(r: f32, g: f32, b: f32) -> (f32, f32) {
    let (_, cb, cr) = ycbcr(r, g, b);
    (cb + 0.5, 0.5 - cr)
}

/// Counts the scopes of an RGBA picture (4 bytes a pixel, rows in a row).
pub fn analyse(rgba: &[u8], width: usize, height: usize) -> Scopes {
    let mut s = Scopes {
        width: width as u32,
        height: height as u32,
        luma: vec![0; WAVE_COLS * LEVELS],
        red: vec![0; WAVE_COLS * LEVELS],
        green: vec![0; WAVE_COLS * LEVELS],
        blue: vec![0; WAVE_COLS * LEVELS],
        vector: vec![0; 256 * 256],
        hist: [[0; 256]; 4],
    };
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return s;
    }
    for y in 0..height {
        let row = &rgba[y * width * 4..(y + 1) * width * 4];
        let (pixels, _) = row.as_chunks::<4>();
        for (x, px) in pixels.iter().enumerate() {
            let col = x * WAVE_COLS / width;
            let (r, g, b) = (px[0], px[1], px[2]);
            let (yf, cb, cr) = ycbcr(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            let luma = (yf * 255.0).round().clamp(0.0, 255.0) as usize;
            for (plane, v) in [
                (&mut s.luma, luma),
                (&mut s.red, r as usize),
                (&mut s.green, g as usize),
                (&mut s.blue, b as usize),
            ] {
                plane[(LEVELS - 1 - v) * WAVE_COLS + col] += 1;
            }
            let vx = ((cb + 0.5) * 255.0).round().clamp(0.0, 255.0) as usize;
            let vy = ((0.5 - cr) * 255.0).round().clamp(0.0, 255.0) as usize;
            s.vector[vy * 256 + vx] += 1;
            s.hist[0][luma] += 1;
            s.hist[1][r as usize] += 1;
            s.hist[2][g as usize] += 1;
            s.hist[3][b as usize] += 1;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        (0..w * h)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect()
    }

    #[test]
    fn a_gray_picture_is_one_line_in_the_middle_of_the_vectorscope() {
        let s = analyse(&solid(32, 16, [128, 128, 128]), 32, 16);
        let total: u32 = s.vector.iter().sum();
        assert_eq!(total, 32 * 16);
        // All of it at the centre (128, 128 within a pixel).
        let mut centre = 0;
        for y in 127..=128 {
            for x in 127..=128 {
                centre += s.vector[y * 256 + x];
            }
        }
        assert_eq!(centre, total);
        // And one level in the waveform and histogram.
        assert_eq!(s.hist[0][128], total);
        assert_eq!(s.luma[(LEVELS - 1 - 128) * WAVE_COLS], 16);
    }

    #[test]
    fn red_lands_up_and_left_and_blue_right() {
        let red = analyse(&solid(8, 8, [255, 0, 0]), 8, 8);
        let (x, y) = vector_position(1.0, 0.0, 0.0);
        assert!(x < 0.5 && y < 0.5, "red is up and left: {x} {y}");
        let hot = red.vector.iter().position(|c| *c > 0).unwrap();
        assert!(hot / 256 < 128 && hot % 256 < 128);
        let (x, _) = vector_position(0.0, 0.0, 1.0);
        assert!(x > 0.9, "blue is far right: {x}");
    }

    #[test]
    fn the_parade_keeps_the_channels_apart() {
        // Left half red, right half blue.
        let (w, h) = (64, 4);
        let mut px = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                px.extend(if x < w / 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                });
            }
        }
        let s = analyse(&px, w, h);
        let at = |plane: &Vec<u32>, level: usize, col: usize| {
            plane[(LEVELS - 1 - level) * WAVE_COLS + col]
        };
        // Red is full on the left and zero on the right; blue the other way.
        assert!(at(&s.red, 255, 5) > 0 && at(&s.red, 0, WAVE_COLS - 5) > 0);
        assert!(at(&s.blue, 255, WAVE_COLS - 5) > 0 && at(&s.blue, 0, 5) > 0);
        assert_eq!(at(&s.green, 0, 5), at(&s.green, 0, WAVE_COLS - 5));
        assert_eq!(s.hist[1][255], (w * h / 2) as u32);
    }

    #[test]
    fn a_short_buffer_gives_empty_scopes() {
        let s = analyse(&[0; 8], 100, 100);
        assert_eq!(s.hist[0].iter().sum::<u32>(), 0);
    }
}
