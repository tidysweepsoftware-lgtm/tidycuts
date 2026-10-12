//! Writes the sample pictures to a PPM sheet:
//! `cargo run --release -p tidycuts-render --example sample_sheet -- out.ppm [width]`

use tidycuts_render::sample;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("output path");
    let w: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(640);
    let h = w * 9 / 16;
    let pics = [
        sample::landscape(w, h),
        sample::sunset(w, h),
        sample::portrait(w, h),
        sample::night(w, h),
        sample::wildlife(w, h),
        sample::meadow(w, h),
        sample::pet(w, h),
        sample::nap(w, h),
        sample::city(w, h),
        sample::food(w, h),
    ];
    let t = std::time::Instant::now();
    let (sw, sh) = (w * 3, h * 4);
    let mut sheet = vec![0u8; (sw * sh * 3) as usize];
    for (i, p) in pics.iter().enumerate() {
        let (x0, y0) = ((i as u32 % 3) * w, (i as u32 / 3) * h);
        for y in 0..h {
            for x in 0..w {
                let s = ((y * w + x) * 4) as usize;
                let d = (((y0 + y) * sw + x0 + x) * 3) as usize;
                sheet[d..d + 3].copy_from_slice(&p.pixels[s..s + 3]);
            }
        }
    }
    let mut ppm = format!("P6\n{sw} {sh}\n255\n").into_bytes();
    ppm.extend_from_slice(&sheet);
    std::fs::write(out, ppm).unwrap();
    eprintln!("drawn in {:?}", t.elapsed());
}
