// Video effects. `u.kind` picks the effect (ids match catalog/effects.rs),
// `u.flags` is the pass index for multi-pass effects, `u.time` is seconds
// since the clip started. Parameters are in p0..p3 (in catalog order) and
// colors in c0/c1. A is the layer (or canvas) being processed and has the
// same size as the destination.

// ---- Blur ---------------------------------------------------------------

// Separable Gaussian, pass 0 horizontal, pass 1 vertical. p0.x = radius
// as a fraction of the height (0..1 maps to 0..6%).
fn fx_gaussian(uv: vec2<f32>, radius_frac: f32) -> vec4<f32> {
    let radius_px = radius_frac * 0.06 * u.dst_size.y;
    if radius_px < 0.5 {
        return sa(uv);
    }
    var dir = vec2<f32>(1.0, 0.0);
    if u.flags == 1u {
        dir = vec2<f32>(0.0, 1.0);
    }
    let taps = min(ceil(radius_px), 24.0);
    let stride = radius_px / taps;
    let sigma = radius_px * 0.5;
    var acc = sa(uv);
    var wsum = 1.0;
    for (var i = 1.0; i <= taps; i = i + 1.0) {
        let d = i * stride;
        let w = exp(-(d * d) / (2.0 * sigma * sigma));
        let o = dir * d * px();
        acc = acc + (sa(uv + o) + sa(uv - o)) * w;
        wsum = wsum + 2.0 * w;
    }
    return acc / wsum;
}

// Spin blur around the centre. p0.x = amount.
fn fx_radial_blur(uv: vec2<f32>) -> vec4<f32> {
    let amount = u.p0.x * 0.25;
    let c = vec2<f32>(0.5, 0.5);
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 24; i = i + 1) {
        let a = (f32(i) / 23.0 - 0.5) * amount;
        var d = uv - c;
        d.x = d.x * aspect();
        d = rot2(a) * d;
        d.x = d.x / aspect();
        acc = acc + sa(c + d);
    }
    return acc / 24.0;
}

// Zoom blur towards (p0.y, p0.z). p0.x = amount.
fn fx_zoom_blur(uv: vec2<f32>) -> vec4<f32> {
    let c = u.p0.yz;
    let amount = u.p0.x * 0.3;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 24; i = i + 1) {
        let s = 1.0 - amount * f32(i) / 23.0;
        acc = acc + sa(c + (uv - c) * s);
    }
    return acc / 24.0;
}

// Motion blur along angle p0.y (degrees). p0.x = amount.
fn fx_directional_blur(uv: vec2<f32>) -> vec4<f32> {
    let ang = radians(u.p0.y);
    let dir = vec2<f32>(cos(ang) / aspect(), sin(ang)) * u.p0.x * 0.08;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 24; i = i + 1) {
        let t = f32(i) / 23.0 - 0.5;
        acc = acc + sa(uv + dir * t);
    }
    return acc / 24.0;
}

// ---- Stylize -----------------------------------------------------------

// Pixelate. p0.x = block size (0..1 maps to 0.3%..8% of the height).
fn fx_mosaic(uv: vec2<f32>) -> vec4<f32> {
    let cell = max(mix(0.003, 0.08, u.p0.x) * u.dst_size.y, 1.0);
    let p = (floor(uv * u.dst_size / cell) + 0.5) * cell / u.dst_size;
    return sa(p);
}

fn fx_invert(uv: vec2<f32>) -> vec4<f32> {
    let c = sa(uv);
    return vec4<f32>(c.a - c.rgb, c.a);
}

// Halftone dots. p0.x = dot size.
fn fx_halftone(uv: vec2<f32>) -> vec4<f32> {
    let cell = mix(0.006, 0.04, u.p0.x) * u.dst_size.y;
    let p = uv * u.dst_size;
    let r = rot2(0.785) * p;
    let g = (floor(r / cell) + 0.5) * cell;
    let center = transpose(rot2(0.785)) * g;
    let c = sa(center / u.dst_size);
    let l = luma(c.rgb / max(c.a, 0.0001));
    let d = length(r - g) / cell;
    let rad = sqrt(1.0 - l) * 0.75;
    let ink = 1.0 - smoothstep(rad - 0.08, rad + 0.08, d);
    let paper = vec3<f32>(0.97, 0.95, 0.9);
    return vec4<f32>(mix(paper, vec3<f32>(0.08), ink) * c.a, c.a);
}

// Mirror. p0.x = mode: 0 left half, 1 right half, 2 top half, 3 bottom half, 4 quad.
fn fx_mirror(uv: vec2<f32>) -> vec4<f32> {
    var p = uv;
    let mode = u32(u.p0.x + 0.5);
    if mode == 0u {
        p.x = 0.5 - abs(p.x - 0.5);
    } else if mode == 1u {
        p.x = 0.5 + abs(p.x - 0.5);
    } else if mode == 2u {
        p.y = 0.5 - abs(p.y - 0.5);
    } else if mode == 3u {
        p.y = 0.5 + abs(p.y - 0.5);
    } else {
        p = vec2<f32>(0.5) - abs(p - vec2<f32>(0.5));
    }
    return sa(p);
}

// Kaleidoscope. p0.x = segments (2..12), p0.y = rotation speed.
fn fx_kaleidoscope(uv: vec2<f32>) -> vec4<f32> {
    let n = max(floor(mix(2.0, 12.0, u.p0.x)), 2.0);
    var p = centered(uv);
    let r = length(p);
    var a = atan2(p.y, p.x) + u.time * u.p0.y;
    let seg = TAU / n;
    a = abs(((a % seg) + seg) % seg - seg * 0.5);
    p = vec2<f32>(cos(a), sin(a)) * r;
    return sa(vec2<f32>(p.x / aspect() + 0.5, p.y + 0.5));
}

// ---- Glitch & retro ----------------------------------------------------

// RGB split / chromatic aberration. p0.x = amount, p0.y = wobble speed.
fn fx_rgb_split(uv: vec2<f32>) -> vec4<f32> {
    let wob = 1.0 + 0.5 * sin(u.time * u.p0.y * 6.0);
    let o = vec2<f32>(u.p0.x * 0.03 * wob / aspect(), 0.0);
    let r = sa(uv + o);
    let g = sa(uv);
    let b = sa(uv - o);
    return vec4<f32>(r.r, g.g, b.b, max(max(r.a, g.a), b.a));
}

// Digital glitch: displaced slices and colour channels. p0.x = amount,
// p0.y = speed.
fn fx_glitch(uv: vec2<f32>) -> vec4<f32> {
    let t = floor(u.time * mix(4.0, 24.0, u.p0.y));
    let amount = u.p0.x;
    let band = floor(uv.y * 24.0);
    let r = hash12(vec2<f32>(band, t));
    var p = uv;
    if r < amount * 0.6 {
        p.x = p.x + (hash12(vec2<f32>(t, band)) - 0.5) * 0.15 * amount;
    }
    let block = floor(uv * vec2<f32>(16.0, 9.0));
    let rb = hash12(block + t);
    if rb < amount * 0.12 {
        p = p + (hash22(block + t) - 0.5) * 0.08;
    }
    let shift = vec2<f32>(0.012 * amount * (hash11(t) - 0.5) * 2.0, 0.0);
    let c = sa(p);
    return vec4<f32>(sa(p + shift).r, c.g, sa(p - shift).b, c.a);
}

// VHS tape: wobble, colour bleed, noise and tracking lines. p0.x = amount.
fn fx_vhs(uv: vec2<f32>) -> vec4<f32> {
    let amount = u.p0.x;
    var p = uv;
    p.x = p.x + sin(uv.y * 40.0 + u.time * 3.0) * 0.0015 * amount;
    let track = fract(u.time * 0.15);
    let dist = abs(uv.y - track);
    if dist < 0.03 {
        p.x = p.x + (hash12(vec2<f32>(floor(uv.y * 300.0), floor(u.time * 30.0))) - 0.5) * 0.03 * amount;
    }
    let o = vec2<f32>(0.004 * amount, 0.0);
    let c = sa(p);
    var rgb = vec3<f32>(sa(p + o).r, c.g, sa(p - o).b);
    let line = 0.92 + 0.08 * sin(uv.y * u.dst_size.y * 1.5);
    let noise = hash12(uv * u.dst_size + vec2<f32>(u.time * 61.0, u.time * 17.0));
    rgb = rgb * mix(1.0, line, amount) + (noise - 0.5) * 0.12 * amount;
    let lum = luma(rgb);
    rgb = mix(rgb, vec3<f32>(lum) * vec3<f32>(1.05, 1.0, 0.92), 0.25 * amount);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(c.a)), c.a);
}

// Old film: sepia, grain, flicker, scratches and dust. p0.x = amount.
fn fx_old_film(uv: vec2<f32>) -> vec4<f32> {
    let amount = u.p0.x;
    let frame = floor(u.time * 18.0);
    var p = uv + vec2<f32>(0.0, (hash11(frame) - 0.5) * 0.004 * amount);
    let c = unpremul(sa(p));
    let l = luma(c.rgb);
    var rgb = mix(c.rgb, vec3<f32>(l) * vec3<f32>(1.07, 0.95, 0.78), amount);
    let flicker = 1.0 + (hash11(frame * 1.7) - 0.5) * 0.12 * amount;
    rgb = rgb * flicker;
    let grain = hash12(uv * u.dst_size + frame * 13.0) - 0.5;
    rgb = rgb + grain * 0.12 * amount;
    // Vertical scratches.
    for (var i = 0; i < 3; i = i + 1) {
        let fi = f32(i);
        let x = hash11(frame * 0.37 + fi * 11.0);
        let visible = step(0.55, hash11(frame + fi * 3.0));
        let s = 1.0 - smoothstep(0.0, 0.0015, abs(uv.x - x));
        rgb = mix(rgb, vec3<f32>(0.9), s * visible * amount * 0.6);
    }
    // Dust specks.
    let cell = floor(uv * vec2<f32>(40.0, 22.0));
    let h = hash12(cell + frame);
    if h > 0.995 {
        let d = length(fract(uv * vec2<f32>(40.0, 22.0)) - 0.5);
        rgb = mix(rgb, vec3<f32>(0.05), (1.0 - smoothstep(0.05, 0.2, d)) * amount);
    }
    // Vignette.
    let v = length((uv - 0.5) * vec2<f32>(1.2, 1.0));
    rgb = rgb * (1.0 - smoothstep(0.4, 0.95, v) * 0.6 * amount);
    return premul(vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), c.a));
}

// Film grain. p0.x = amount, p0.y = size.
fn fx_grain(uv: vec2<f32>) -> vec4<f32> {
    let c = sa(uv);
    let size = mix(1.0, 4.0, u.p0.y);
    let cell = floor(uv * u.dst_size / size);
    let n = hash12(cell + vec2<f32>(floor(u.time * 24.0) * 7.0, 3.0)) - 0.5;
    let l = luma(c.rgb);
    let w = 1.0 - abs(l - 0.5) * 1.2;
    return vec4<f32>(clamp(c.rgb + n * u.p0.x * 0.35 * w * c.a, vec3<f32>(0.0), vec3<f32>(c.a)), c.a);
}

// ---- Distortion --------------------------------------------------------

// Wave. p0.x = amplitude, p0.y = frequency, p0.z = speed.
fn fx_wave(uv: vec2<f32>) -> vec4<f32> {
    var p = uv;
    let f = mix(2.0, 30.0, u.p0.y);
    p.x = p.x + sin(uv.y * f + u.time * u.p0.z * 6.0) * u.p0.x * 0.03;
    p.y = p.y + cos(uv.x * f * 0.7 + u.time * u.p0.z * 5.0) * u.p0.x * 0.015;
    return sa(p);
}

// ---- Keying ------------------------------------------------------------

// Chroma key (green screen). c0 = key color, p0 = (tolerance, softness,
// spill suppression).
fn fx_chroma_key(uv: vec2<f32>) -> vec4<f32> {
    let c = unpremul(sa(uv));
    let key = u.c0.rgb;
    // Compare chroma only so shadows on the screen key out too.
    let cy = rgb_to_yiq(c.rgb).yz;
    let ky = rgb_to_yiq(key).yz;
    let d = length(cy - ky);
    let tol = mix(0.02, 0.35, u.p0.x);
    let soft = mix(0.005, 0.25, u.p0.y);
    let alpha = smoothstep(tol, tol + soft, d);
    var rgb = c.rgb;
    // Spill: pull the key colour's dominant channel down to the others.
    let spill = u.p0.z;
    if key.g >= key.r && key.g >= key.b {
        rgb.g = mix(rgb.g, min(rgb.g, max(rgb.r, rgb.b)), spill);
    } else if key.b >= key.r {
        rgb.b = mix(rgb.b, min(rgb.b, max(rgb.r, rgb.g)), spill);
    } else {
        rgb.r = mix(rgb.r, min(rgb.r, max(rgb.g, rgb.b)), spill);
    }
    return premul(vec4<f32>(rgb, c.a * alpha));
}

// ---- Light & particles -------------------------------------------------

// Warm light leaks drifting across the frame. p0 = (intensity, speed), c0 = color.
fn fx_light_leak(uv: vec2<f32>) -> vec4<f32> {
    let c = sa(uv);
    let t = u.time * mix(0.05, 0.6, u.p0.y);
    let p = centered(uv);
    var glow = 0.0;
    for (var i = 0; i < 3; i = i + 1) {
        let fi = f32(i);
        let center = vec2<f32>(sin(t * (0.7 + fi * 0.3) + fi * 2.1) * 0.9, cos(t * (0.5 + fi * 0.2) + fi * 1.3) * 0.45);
        let d = length(p - center);
        glow = glow + exp(-d * d * (3.0 + fi * 2.0));
    }
    glow = glow * u.p0.x;
    let leak = u.c0.rgb * glow;
    // Screen blend.
    let rgb = c.rgb + leak * c.a - c.rgb * leak;
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(c.a)), c.a);
}

// Falling snow. p0 = (amount, speed, size).
fn fx_snow(uv: vec2<f32>) -> vec4<f32> {
    let c = sa(uv);
    var flakes = 0.0;
    for (var l = 0; l < 4; l = l + 1) {
        let fl = f32(l);
        let scale = 8.0 + fl * 7.0;
        let speed = (0.08 + fl * 0.04) * mix(0.3, 2.0, u.p0.y);
        var p = vec2<f32>(uv.x * aspect(), uv.y) * scale;
        p.y = p.y - u.time * speed * scale;
        p.x = p.x + sin(u.time * 0.7 + fl + p.y * 0.3) * 0.3;
        let cell = floor(p);
        let h = hash22(cell + fl * 31.0);
        if h.x < u.p0.x * 0.6 {
            let center = cell + 0.2 + h * 0.6;
            let r = mix(0.04, 0.12, u.p0.z) * (1.0 - fl * 0.15);
            let d = length(p - center);
            flakes = flakes + (1.0 - smoothstep(r * 0.4, r, d)) * (1.0 - fl * 0.18);
        }
    }
    flakes = clamp(flakes, 0.0, 1.0);
    return vec4<f32>(mix(c.rgb, vec3<f32>(c.a), flakes), c.a);
}

// ---- Frames & layouts --------------------------------------------------

// Rounded corners with a border. p0 = (radius, border width), c0 = border color.
fn fx_rounded_frame(uv: vec2<f32>) -> vec4<f32> {
    let c = sa(uv);
    let size = u.dst_size;
    let p = (uv - 0.5) * size;
    let half_size = size * 0.5;
    let r = u.p0.x * 0.25 * min(size.x, size.y);
    let q = abs(p) - (half_size - vec2<f32>(r));
    let dist = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    let inside = clamp(0.5 - dist, 0.0, 1.0);
    let bw = u.p0.y * 0.04 * size.y;
    let border = clamp(0.5 - (dist + bw), 0.0, 1.0);
    let border_mask = inside - border;
    let col = c * border + premul(u.c0) * border_mask;
    return col;
}

// Cinematic letterbox bars. p0.x = target aspect: values above 1 are the
// ratio itself (2.35 = 2.35:1), values 0..1 map to 1.5..2.8.
fn fx_letterbox(uv: vec2<f32>) -> vec4<f32> {
    let c = sa(uv);
    let ratio = select(mix(1.5, 2.8, u.p0.x), u.p0.x, u.p0.x > 1.0);
    let h = clamp(aspect() / ratio, 0.2, 1.0);
    let bar = (1.0 - h) * 0.5;
    if uv.y < bar || uv.y > 1.0 - bar {
        return premul(u.c0);
    }
    return c;
}

// Multi-frame grid of n x n copies. p0.x = n (2..4).
fn fx_grid(uv: vec2<f32>, n: f32) -> vec4<f32> {
    let gap = 0.006;
    let cell = fract(uv * n);
    let inner = (cell - gap * n) / (1.0 - 2.0 * gap * n);
    if any(inner < vec2<f32>(0.0)) || any(inner > vec2<f32>(1.0)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return sa(inner);
}


// ---- Shared helpers ----------------------------------------------------
//
// Positions in "height units" are `centered(uv)`: the picture height is 1,
// the centre is (0,0), so sizes look the same at every resolution.

// Straight-alpha color of A.
fn fx_get(uv: vec2f) -> vec4f {
    return unpremul(sa(uv));
}

// Clamps a straight color and premultiplies it with alpha `a`.
fn fx_put(rgb: vec3f, a: f32) -> vec4f {
    return vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)) * a, a);
}

// Screen blend of `light` over `base`.
fn fx_screen(base: vec3f, light: vec3f) -> vec3f {
    let l = clamp(light, vec3f(0.0), vec3f(1.0));
    return vec3f(1.0) - (vec3f(1.0) - base) * (vec3f(1.0) - l);
}

// Height-unit position (as from `centered`) back to uv.
fn fx_uv(p: vec2f) -> vec2f {
    return vec2f(p.x / aspect() + 0.5, p.y + 0.5);
}

// One destination pixel in height units, for anti-aliasing.
fn fx_pxh() -> f32 {
    return 1.0 / max(u.dst_size.y, 1.0);
}

// Coverage of a shape from its signed distance in height units.
fn fx_fill(d: f32) -> f32 {
    return clamp(0.5 - d / fx_pxh(), 0.0, 1.0);
}

fn fx_l(uv: vec2f) -> f32 {
    return luma(sa(uv).rgb);
}

// Step for edge filters: `k`/720 of the height (at least one pixel), in uv.
fn fx_edge_step(k: f32) -> vec2f {
    let s = max(k / 720.0, fx_pxh());
    return vec2f(s / aspect(), s);
}

// Sobel gradient of luma with taps `o` apart (uv). A hard edge of contrast
// C gives a length of about 4 C.
fn fx_sobel(uv: vec2f, o: vec2f) -> vec2f {
    let tl = fx_l(uv + vec2f(-o.x, -o.y));
    let tc = fx_l(uv + vec2f(0.0, -o.y));
    let tr = fx_l(uv + vec2f(o.x, -o.y));
    let ml = fx_l(uv + vec2f(-o.x, 0.0));
    let mr = fx_l(uv + vec2f(o.x, 0.0));
    let bl = fx_l(uv + vec2f(-o.x, o.y));
    let bc = fx_l(uv + vec2f(0.0, o.y));
    let br = fx_l(uv + vec2f(o.x, o.y));
    let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
    let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
    return vec2f(gx, gy);
}

// Anti-aliased cosine stripes in 0..1. `x` in height units, `freq` in
// cycles per height unit. Fades to 0.5 once the stripes are finer than the
// pixels, so small previews show the average instead of moiré.
fn fx_stripes(x: f32, freq: f32, phase: f32) -> f32 {
    let cpp = freq / max(u.dst_size.y, 1.0);
    let aa = 1.0 - smoothstep(0.18, 0.45, cpp);
    return 0.5 + 0.5 * cos(TAU * (x * freq + phase)) * aa;
}

// Average of A over a disc of `radius` height units with `n` taps.
fn fx_disc(uv: vec2f, radius: f32, n: i32) -> vec4f {
    let r = vec2f(radius / aspect(), radius);
    let nf = f32(n);
    var acc = vec4f(0.0);
    for (var i = 0; i < n; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let rad = sqrt((fi + 0.5) / nf);
        acc = acc + sa(uv + vec2f(cos(ang), sin(ang)) * rad * r);
    }
    return acc / nf;
}

// Maps uv inside a window of aspect `wa` onto the picture so it covers the
// window (centre crop).
fn fx_cover(l: vec2f, wa: f32) -> vec2f {
    let a = aspect();
    var s = vec2f(1.0);
    if wa > a {
        s.y = a / wa;
    } else {
        s.x = wa / a;
    }
    return (l - 0.5) * s + 0.5;
}

// Signed distance to a box of half size `b` with corner radius `r`.
fn fx_sd_box(p: vec2f, b: vec2f, r: f32) -> f32 {
    let q = abs(p) - b + vec2f(r);
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn fx_sd_segment(p: vec2f, a: vec2f, b: vec2f) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Heart with its tip at (0,0) and top near y = 1 (y up).
fn fx_sd_heart(p_in: vec2f) -> f32 {
    let p = vec2f(abs(p_in.x), p_in.y);
    if p.y + p.x > 1.0 {
        let d = p - vec2f(0.25, 0.75);
        return sqrt(dot(d, d)) - 0.35355;
    }
    let a = p - vec2f(0.0, 1.0);
    let b = p - vec2f(0.5 * max(p.x + p.y, 0.0));
    return sqrt(min(dot(a, a), dot(b, b))) * sign(p.x - p.y);
}

// ---- Lens & warps --------------------------------------------------------

// Lens distortion. `amount` in -1..1: positive bulges the picture like a
// wide-angle lens (barrel); negative pulls the edges out (pincushion), which
// straightens the barrel distortion of action cameras. The picture is
// scaled so no empty edges show.
fn fx_lens_uv(uv: vec2f, amount: f32) -> vec2f {
    let a = aspect();
    let diag = 0.5 * sqrt(a * a + 1.0);
    let p = centered(uv) / diag;
    let r2 = dot(p, p);
    let k = clamp(amount, -1.0, 1.0) * 0.9;
    var f = 1.0;
    if k >= 0.0 {
        f = (1.0 + k * r2) / (1.0 + k);
    } else {
        // Keep the nearest edge midpoint on the edge.
        let e = 0.5 * min(a, 1.0) / diag;
        f = (1.0 - k * e * e) / (1.0 - k * r2);
    }
    return fx_uv(p * f * diag);
}

fn fx_lens(uv: vec2f) -> vec4f {
    return sa(fx_lens_uv(uv, u.p0.x));
}

// Fisheye. p0 = (strength, circular 0/1).
fn fx_fisheye(uv: vec2f) -> vec4f {
    let a = aspect();
    let theta_max = mix(0.35, 1.42, clamp(u.p0.x, 0.0, 1.0));
    let tmax = tan(theta_max);
    let p = centered(uv);
    let len = max(length(p), 1e-5);
    let dir = p / len;
    if u.p0.y > 0.5 {
        // The whole picture squeezed into a circle the height of the frame.
        let r = len / 0.5;
        let rs = tan(min(r, 1.0) * theta_max) / tmax;
        let reach = min(0.5 * a / max(abs(dir.x), 1e-5), 0.5 / max(abs(dir.y), 1e-5));
        let col = sa(fx_uv(dir * rs * reach));
        let inside = fx_fill(len - 0.5);
        let shade = 1.0 - smoothstep(0.7, 1.0, r) * 0.4;
        return vec4f(col.rgb * shade * inside, col.a * inside + (1.0 - inside));
    }
    let diag = 0.5 * sqrt(a * a + 1.0);
    let r = len / diag;
    let rs = tan(r * theta_max) / tmax;
    let shade = 1.0 - smoothstep(0.55, 1.0, r) * 0.3 * u.p0.x;
    let col = sa(fx_uv(dir * rs * diag));
    return vec4f(col.rgb * shade, col.a);
}

// Bulge (amount > 0) or pinch (amount < 0) inside a circle.
// p0 = (amount, radius, centre x, centre y).
fn fx_bulge(uv: vec2f, amount: f32) -> vec4f {
    let c = centered(u.p0.zw);
    let p = centered(uv);
    let d = p - c;
    let dist = length(d);
    let radius = mix(0.1, 0.9, u.p0.y);
    if dist >= radius {
        return sa(uv);
    }
    let t = dist / radius;
    let k = clamp(amount, -1.0, 1.0) * 0.85;
    let f = 1.0 - k * (1.0 - t) * (1.0 - t);
    return sa(fx_uv(c + d * f));
}

// Twirl. p0 = (angle in degrees, radius, speed).
fn fx_twirl(uv: vec2f) -> vec4f {
    let p = centered(uv);
    let radius = mix(0.15, 1.0, u.p0.y);
    let dist = length(p);
    if dist >= radius {
        return sa(uv);
    }
    let t = 1.0 - dist / radius;
    var ang = radians(u.p0.x) * t * t;
    if u.p0.z > 0.001 {
        ang = ang * cos(u.time * u.p0.z * 3.0);
    }
    return sa(fx_uv(rot2(ang) * p));
}

// Water ripples spreading from a point. p0 = (amount, wavelength, speed,
// centre x), p1.x = centre y.
fn fx_ripple(uv: vec2f) -> vec4f {
    let c = centered(vec2f(u.p0.w, u.p1.x));
    let p = centered(uv);
    let d = p - c;
    let dist = length(d);
    let wl = mix(0.02, 0.15, u.p0.y);
    let ph = dist * TAU / wl - u.time * mix(1.0, 12.0, u.p0.z);
    let damp = exp(-dist * 1.5) * smoothstep(0.0, wl, dist);
    let dir = d / max(dist, 1e-4);
    let off = dir * sin(ph) * u.p0.x * 0.012 * damp;
    let col = fx_get(fx_uv(p + off));
    let shade = 1.0 + cos(ph) * u.p0.x * 0.25 * damp;
    return fx_put(col.rgb * shade, col.a);
}

// Heat haze shimmer rising from the bottom. p0 = (amount, speed, scale).
fn fx_heat(uv: vec2f) -> vec4f {
    let p = centered(uv);
    let sc = mix(4.0, 18.0, u.p0.z);
    let t = u.time * mix(0.3, 3.0, u.p0.y);
    let q = p * sc + vec2f(0.0, t);
    let n = vec2f(vnoise(q), vnoise(q + vec2f(5.2, 1.3))) - 0.5;
    let n2 = vec2f(vnoise(q * 2.1 + 3.0), vnoise(q * 2.1 + 7.0)) - 0.5;
    let w = mix(0.35, 1.0, uv.y);
    return sa(fx_uv(p + (n + n2 * 0.5) * u.p0.x * 0.045 * w));
}

// The picture seen through a wall of glass blocks. p0 = (tile size,
// refraction).
fn fx_tiles(uv: vec2f) -> vec4f {
    let s = mix(0.05, 0.3, u.p0.x);
    let p = centered(uv) + vec2f(0.5 * s);
    let g = floor(p / s);
    let l = p / s - g - 0.5;
    let center = (g + 0.5) * s - vec2f(0.5 * s);
    let mag = mix(1.0, -0.4, u.p0.y);
    let col = fx_get(fx_uv(center + l * s * mag));
    let e = max(abs(l.x), abs(l.y));
    let bev = smoothstep(0.34, 0.5, e);
    let lit = dot(l / max(length(l), 1e-4), vec2f(-0.707, -0.707));
    var rgb = col.rgb + vec3f(bev * lit * 0.22 - bev * 0.04);
    let groove = smoothstep(0.5 - fx_pxh() * 1.5 / s, 0.5, e);
    rgb = rgb * (1.0 - groove * 0.45);
    return fx_put(rgb, col.a);
}

// ---- More blurs ----------------------------------------------------------

// Tilt-shift miniature: blur above and below a sharp band (pass 0
// horizontal, pass 1 vertical plus a punchier color).
// p0 = (blur, focus position, focus size, saturation).
fn fx_tilt_shift(uv: vec2f) -> vec4f {
    let dist = abs(uv.y - u.p0.y);
    let band = u.p0.z * 0.35;
    let k = smoothstep(band, band + 0.25, dist);
    var c = fx_gaussian(uv, u.p0.x * k);
    if u.flags == 1u {
        let s = unpremul(c);
        let l = luma(s.rgb);
        var rgb = mix(vec3f(l), s.rgb, 1.0 + u.p0.w * 0.7);
        rgb = (rgb - 0.5) * (1.0 + u.p0.w * 0.2) + 0.5;
        c = fx_put(rgb, s.a);
    }
    return c;
}

// Focus blur: sharp inside a circle, blurred towards the edges (two passes).
// p0 = (blur, focus size, centre x, centre y).
fn fx_focus_blur(uv: vec2f) -> vec4f {
    let d = length(centered(uv) - centered(u.p0.zw));
    let r0 = mix(0.05, 0.6, u.p0.y);
    let k = smoothstep(r0, r0 + 0.35, d);
    return fx_gaussian(uv, u.p0.x * k);
}

// Lens blur with bright bokeh discs. p0 = (radius, highlights).
fn fx_lens_blur(uv: vec2f) -> vec4f {
    let radius = u.p0.x * 0.045;
    if radius * u.dst_size.y < 0.5 {
        return sa(uv);
    }
    let r = vec2f(radius / aspect(), radius);
    let boost = u.p0.y * 40.0;
    var acc = vec4f(0.0);
    var wsum = 0.0;
    for (var i = 0; i < 72; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let rad = sqrt((fi + 0.5) / 72.0);
        let c = sa(uv + vec2f(cos(ang), sin(ang)) * rad * r);
        let hi = max(luma(c.rgb) - 0.6, 0.0) / 0.4;
        let w = 1.0 + boost * hi * hi;
        acc = acc + c * w;
        wsum = wsum + w;
    }
    return acc / wsum;
}

// ---- Modern looks ----------------------------------------------------------
//
// Cinematic and short-form looks: halation and diffusion, anamorphic
// streaks, light sweeps, beat-timed flashes and pulses, speed lines, a
// camcorder overlay, flowing colour, prism fringes, liquid warping, dust,
// embers and mist.

// How far into the beat the clip is: 0 on the beat, climbing to 1 just before
// the next one.
fn fx_beat(bpm: f32) -> f32 {
    return fract(max(u.time, 0.0) * bpm / 60.0);
}

// Halation: the warm glow film puts around bright lights.
// p0 = (amount, radius, threshold), c0 = glow color.
fn fx_halation(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let th = mix(0.3, 0.9, u.p0.z);
    let r = mix(0.01, 0.1, u.p0.y);
    let g = fx_glow_gather(uv, r, th) * 0.55 + fx_glow_gather(uv, r * 2.4, th) * 0.45;
    // Mostly the red layer of the film scatters, so the glow is warm.
    let h = g.r * 0.7 + g.g * 0.2 + g.b * 0.1;
    return fx_put(fx_screen(c.rgb, u.c0.rgb * h * u.p0.x * 5.0), c.a);
}

// Diffusion filter: softened detail, glowing highlights, a touch less
// contrast. p0 = (amount, spread, highlights).
fn fx_pro_mist(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let r = mix(0.006, 0.05, u.p0.y);
    let b = unpremul(fx_disc(uv, r, 40)).rgb;
    let hl = fx_glow_gather(uv, r * 1.8, 0.5);
    var rgb = mix(c.rgb, b, u.p0.x * 0.5);
    rgb = fx_screen(rgb, hl * u.p0.z * u.p0.x * 2.5);
    rgb = (rgb - 0.5) * (1.0 - 0.15 * u.p0.x) + 0.5 + 0.015 * u.p0.x;
    return fx_put(rgb, c.a);
}

// A soft, tinted haze over everything. p0 = (amount, softness), c0 = tint.
fn fx_dream_glow(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let b = unpremul(fx_disc(uv, mix(0.015, 0.09, u.p0.y), 48)).rgb;
    let glow = b * b * 1.4 * u.c0.rgb;
    var rgb = fx_screen(c.rgb, glow * u.p0.x);
    rgb = mix(rgb, mix(rgb, b, 0.6) + vec3f(0.03, 0.01, 0.04), u.p0.x * 0.4);
    let l = luma(rgb);
    rgb = mix(vec3f(l), rgb, 1.0 + 0.12 * u.p0.x);
    return fx_put(rgb, c.a);
}

// Horizontal streaks stretched from bright lights, like an anamorphic lens.
// p0 = (amount, length, threshold), c0 = streak color.
fn fx_anamorphic(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let th = mix(0.4, 0.95, u.p0.z);
    let len = mix(0.04, 0.6, u.p0.y);
    var acc = vec3f(0.0);
    var wsum = 0.0;
    for (var i = 1; i <= 36; i = i + 1) {
        let t = f32(i) / 36.0;
        let w = exp(-t * 3.2);
        let o = vec2f(t * len, 0.0);
        let a = max(sa(uv + o).rgb - vec3f(th), vec3f(0.0));
        let b = max(sa(uv - o).rgb - vec3f(th), vec3f(0.0));
        acc = acc + (a + b) * w;
        wsum = wsum + w;
    }
    let streak = acc / wsum / max(1.0 - th, 0.05);
    let l = luma(streak);
    let rgb = fx_screen(c.rgb, (u.c0.rgb * l * 1.6 + streak * 0.25) * u.p0.x * 2.4);
    return fx_put(rgb, c.a);
}

// A band of light that sweeps across the picture, then rests.
// p0 = (speed, width, angle in degrees, intensity), c0 = light color.
fn fx_light_sweep(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let a = radians(u.p0.z);
    let dir = vec2f(cos(a), sin(a));
    let span = 0.5 * (abs(dir.x) * aspect() + abs(dir.y));
    let w = mix(0.04, 0.3, u.p0.y);
    let ph = clamp(fract(u.time * mix(0.2, 0.9, u.p0.x) * 0.5) * 1.6, 0.0, 1.0);
    let centre = mix(-span - w * 2.0, span + w * 2.0, ph);
    let d = (dot(p, dir) - centre) / w;
    let band = exp(-d * d * 2.2);
    let lit = 0.4 + 0.6 * smoothstep(0.1, 0.8, luma(c.rgb));
    return fx_put(fx_screen(c.rgb, u.c0.rgb * band * lit * u.p0.w * 0.9), c.a);
}

// Darkens everything outside a soft circle of light.
// p0 = (size, softness, darkness, centre x), p1.x = centre y.
fn fx_spotlight(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let d = length(centered(uv) - centered(vec2f(u.p0.w, u.p1.x)));
    let r = mix(0.12, 0.8, u.p0.x);
    let m = smoothstep(r, r + mix(0.02, 0.7, u.p0.y), d);
    let rgb = c.rgb * (1.0 - m * u.p0.z) * (1.0 + (1.0 - m) * 0.08);
    return fx_put(rgb, c.a);
}

// A flash on every beat. p0 = (beats per minute, intensity, decay), c0 = color.
fn fx_beat_flash(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let f = pow(1.0 - fx_beat(u.p0.x), mix(5.0, 1.5, u.p0.z)) * u.p0.y;
    return fx_put(c.rgb * (1.0 + f * 0.6) + u.c0.rgb * f * 0.55, c.a);
}

// Red and blue edges that split apart on every beat.
// p0 = (beats per minute, amount).
fn fx_rgb_pulse(uv: vec2f) -> vec4f {
    let k = pow(1.0 - fx_beat(u.p0.x), 3.0) * u.p0.y * 0.06;
    let p = centered(uv);
    let off = p * k * (0.4 + length(p) * 1.2);
    let a = sa(fx_uv(p - off));
    let g = sa(uv);
    let b = sa(fx_uv(p + off));
    return vec4f(a.r, g.g, b.b, g.a);
}

// Action lines streaming out from the middle, redrawn a few times a second.
// p0 = (density, clear area, speed, opacity), c0 = color.
fn fx_speed_lines(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let r = length(p);
    let turn = atan2(p.y, p.x) / TAU + 0.5;
    let n = mix(36.0, 150.0, u.p0.x);
    let s = turn * n;
    let slot = floor(s);
    let within = fract(s);
    let frame = floor(u.time * mix(5.0, 20.0, u.p0.z));
    let h = hash12(vec2f(slot, frame));
    let h2 = hash12(vec2f(slot + 31.7, frame * 1.3 + 4.0));
    let on = step(0.45, h);
    let width = mix(0.18, 0.7, h2);
    let tri = clamp(1.0 - abs(within - 0.5) * 2.0 / width, 0.0, 1.0);
    let inner = mix(0.14, 0.62, u.p0.y) * (0.85 + 0.3 * h2);
    let reach = smoothstep(inner, inner + 0.2, r);
    let line = tri * on * reach * u.p0.w;
    return fx_put(mix(c.rgb, u.c0.rgb, line), c.a);
}

// Faded copies trailing behind, like a multiple exposure.
// p0 = (amount, distance, angle in degrees, speed).
fn fx_echo(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let a = radians(u.p0.z);
    let dir = vec2f(cos(a) / aspect(), sin(a));
    let drift = 0.6 + 0.4 * sin(u.time * mix(0.5, 3.0, u.p0.w) * PI);
    let dist = mix(0.01, 0.12, u.p0.y) * drift;
    var rgb = c.rgb;
    for (var i = 1; i <= 4; i = i + 1) {
        let k = f32(i);
        let g = fx_get(uv - dir * dist * k).rgb;
        let w = u.p0.x * (1.0 - (k - 1.0) / 4.0) * 0.7;
        rgb = mix(rgb, max(rgb, g * (1.0 - 0.12 * k)), w);
    }
    return fx_put(rgb, c.a);
}

// ---- camcorder overlay: a tiny 3x5 dot font ------------------------------

// 0-9, ':', R, E, C; each glyph is 15 bits, the top row first.
var<private> FONT: array<u32, 14> = array<u32, 14>(0x7b6fu, 0x2c97u, 0x73e7u, 0x73cfu, 0x5bc9u, 0x79cfu, 0x79efu, 0x7249u, 0x7befu, 0x7bcfu, 0x0410u, 0x6badu, 0x79a7u, 0x3923u);

// 1 where a glyph has a dot in `cell` (column 0..2, row 0..4).
fn fx_glyph_px(id: u32, cell: vec2i) -> f32 {
    if cell.x < 0 || cell.x > 2 || cell.y < 0 || cell.y > 4 {
        return 0.0;
    }
    let bit = 14u - u32(cell.y * 3 + cell.x);
    return f32((FONT[id] >> bit) & 1u);
}

// One glyph with its top-left corner at `o` (pixels), `d` pixels per dot.
fn fx_glyph_at(pp: vec2f, o: vec2f, d: f32, id: u32) -> f32 {
    return fx_glyph_px(id, vec2i(floor((pp - o) / d)));
}

// The clip time as hh:mm:ss:ff at 30 frames a second.
fn fx_timecode(pp: vec2f, o: vec2f, d: f32) -> f32 {
    let t = u32(max(u.time, 0.0) * 30.0);
    let ff = t % 30u;
    let ss = (t / 30u) % 60u;
    let mm = (t / 1800u) % 60u;
    let hh = (t / 108000u) % 100u;
    var ids = array<u32, 11>(hh / 10u, hh % 10u, 10u, mm / 10u, mm % 10u, 10u, ss / 10u, ss % 10u, 10u, ff / 10u, ff % 10u);
    var v = 0.0;
    for (var i = 0u; i < 11u; i = i + 1u) {
        v = max(v, fx_glyph_at(pp, o + vec2f(f32(i) * 4.0 * d, 0.0), d, ids[i]));
    }
    return v;
}

// The white parts of the overlay: corner brackets, REC, the timecode and a
// battery. `m` is the margin, `d` the pixels per font dot.
fn fx_hud_ink(pp: vec2f, size: vec2f, m: f32, d: f32) -> f32 {
    var v = 0.0;
    let t = max(2.0, size.y * 0.004);
    let arm = size.y * 0.06;
    let q = min(pp - vec2f(m), size - vec2f(m) - pp);
    if q.x >= 0.0 && q.y >= 0.0 {
        if (q.x < t && q.y < arm) || (q.y < t && q.x < arm) {
            v = 1.0;
        }
    }
    let inset = m + size.y * 0.03;
    // REC
    let rec = vec2f(inset + d * 6.0, inset);
    v = max(v, fx_glyph_at(pp, rec, d, 11u));
    v = max(v, fx_glyph_at(pp, rec + vec2f(4.0 * d, 0.0), d, 12u));
    v = max(v, fx_glyph_at(pp, rec + vec2f(8.0 * d, 0.0), d, 13u));
    // Timecode, bottom left.
    v = max(v, fx_timecode(pp, vec2f(inset, size.y - inset - 5.0 * d), d));
    // Battery, top right: outline, charge and a nub.
    let x1 = size.x - inset;
    let b0 = vec2f(x1 - 10.0 * d, inset);
    let b1 = vec2f(x1 - 1.0 * d, inset + 5.0 * d);
    let th = max(d * 0.6, 1.0);
    if pp.x >= b0.x && pp.x <= b1.x && pp.y >= b0.y && pp.y <= b1.y {
        let inside = pp.x > b0.x + th && pp.x < b1.x - th && pp.y > b0.y + th && pp.y < b1.y - th;
        if !inside {
            v = 1.0;
        } else if pp.x < mix(b0.x, b1.x, 0.72) {
            v = 1.0;
        }
    }
    if pp.x > b1.x && pp.x <= b1.x + d && pp.y > b0.y + 1.5 * d && pp.y < b1.y - 1.5 * d {
        v = 1.0;
    }
    return v;
}

// A camera viewfinder: brackets, a blinking red REC dot, the running
// timecode and a battery. p0 = (size, opacity).
fn fx_camcorder(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let size = u.dst_size;
    let pp = uv * size;
    let d = max(floor(size.y * mix(0.006, 0.014, u.p0.x)), 2.0);
    let m = size.y * 0.055;
    let ink = fx_hud_ink(pp, size, m, d);
    let shade = fx_hud_ink(pp - vec2f(d * 0.5), size, m, d);
    var rgb = mix(c.rgb, vec3f(0.0), shade * 0.5 * u.p0.y);
    rgb = mix(rgb, vec3f(1.0), ink * u.p0.y);
    // The red dot beside REC blinks once a second.
    let inset = m + size.y * 0.03;
    let dot_c = vec2f(inset + d * 2.0, inset + d * 2.5);
    let blink = step(fract(max(u.time, 0.0) * 0.9), 0.6);
    let dot_m = (1.0 - smoothstep(d * 1.6, d * 1.9, length(pp - dot_c))) * blink;
    rgb = mix(rgb, vec3f(1.0, 0.12, 0.1), dot_m * u.p0.y);
    return fx_put(rgb, c.a);
}

// Slow drifting blobs of colour laid over the picture.
// p0 = (amount, speed, scale), c0/c1 = colors.
fn fx_fluid_colour(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let t = u.time * mix(0.05, 0.5, u.p0.y);
    let q = vec2f(uv.x * aspect(), uv.y) * mix(0.8, 3.0, u.p0.z);
    let w = vec2f(fbm(q + vec2f(t, 0.0)), fbm(q + vec2f(5.2, 1.3) - vec2f(0.0, t)));
    let n = fbm(q + 2.5 * w + vec2f(t * 0.5));
    let third = hsv2rgb(vec3f(fract(rgb2hsv(u.c0.rgb).x + 0.35), 0.75, 1.0));
    var col = mix(u.c0.rgb, u.c1.rgb, smoothstep(0.25, 0.75, n));
    col = mix(col, third, smoothstep(0.55, 0.9, w.x) * 0.7);
    let rgb = mix(c.rgb, c.rgb * (0.55 + col * 1.1) + col * 0.12, u.p0.x);
    return fx_put(rgb, c.a);
}

// A rainbow smear at the edges, like light through a prism.
// p0 = (mode: 0 radial, 1 linear, amount, angle in degrees, intensity).
fn fx_prism(uv: vec2f) -> vec4f {
    let p = centered(uv);
    let diag = 0.5 * sqrt(aspect() * aspect() + 1.0);
    let r = length(p) / diag;
    var dir = p / max(length(p), 0.0001);
    var reach = u.p0.y * 0.04 * (0.15 + r * r * 1.4);
    if u.p0.x > 0.5 {
        dir = vec2f(cos(radians(u.p0.z)), sin(radians(u.p0.z)));
        reach = u.p0.y * 0.04;
    }
    var acc = vec3f(0.0);
    var wsum = vec3f(0.0);
    var alpha = 0.0;
    for (var i = 0; i < 12; i = i + 1) {
        let t = f32(i) / 11.0;
        let c = sa(fx_uv(p + dir * (t - 0.5) * 2.0 * reach));
        let w = hsv2rgb(vec3f(t * 0.82, 0.85, 1.0));
        acc = acc + c.rgb * w;
        wsum = wsum + w;
        alpha = max(alpha, c.a);
    }
    let rgb = mix(sa(uv).rgb, acc / wsum, u.p0.w);
    return vec4f(min(rgb, vec3f(alpha)), alpha);
}

// Smooth, flowing ripples through the picture, with glassy colour edges.
// p0 = (amount, scale, speed).
fn fx_liquid(uv: vec2f) -> vec4f {
    let t = u.time * mix(0.1, 0.9, u.p0.z);
    let q = vec2f(uv.x * aspect(), uv.y) * mix(1.5, 5.0, u.p0.y);
    let o = vec2f(fbm(q + vec2f(t, 3.1)), fbm(q + vec2f(7.7, t * 0.8))) - 0.5;
    let d = o * u.p0.x * 0.11 * vec2f(1.0 / aspect(), 1.0);
    let r = sa(uv + d * 1.12).r;
    let g = sa(uv + d).g;
    let b = sa(uv + d * 0.88).b;
    return vec4f(r, g, b, sa(uv + d).a);
}

// Specks of dust drifting through a beam of light.
// p0 = (amount, speed, size), c0 = color.
fn fx_dust(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.1, 0.7, u.p0.y);
    var glow = 0.0;
    for (var l = 0; l < 3; l = l + 1) {
        let fl = f32(l);
        let sc = 7.0 + fl * 6.0;
        let q = base * sc + vec2f(t * (0.3 + fl * 0.2), -t * (0.25 + fl * 0.15)) + vec2f(fl * 5.3, fl * 2.9);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 31.0);
                if h.x > u.p0.x * 0.6 {
                    continue;
                }
                let h2 = hash22(cell * 1.9 + 4.7);
                let wander = vec2f(sin(t * 3.0 * (0.5 + h2.x) + h.y * 6.28), cos(t * 2.5 * (0.5 + h2.y) + h.x * 6.28));
                let pos = cell + 0.5 + (h2 - 0.5) * 0.8 + 0.2 * wander;
                let d = length(q - pos) / sc;
                let r = mix(0.0015, 0.005, u.p0.z) * (0.5 + 1.2 * h2.y) * (1.0 + fl * 0.5);
                let tw = 0.4 + 0.6 * smoothstep(-0.2, 0.8, sin(t * 2.0 * (0.5 + h2.x) + h.y * 40.0));
                glow = glow + exp(-d * d / (r * r)) * tw * (0.5 + 0.5 * h2.y) * (1.0 - fl * 0.2);
            }
        }
    }
    let light = 0.55 + 0.9 * smoothstep(1.3, 0.1, length(uv - vec2f(0.8, 0.15)));
    return fx_put(c.rgb + u.c0.rgb * glow * light * 0.9, c.a);
}

// Glowing sparks rising and flickering.
// p0 = (amount, speed, size), c0 = color.
fn fx_embers(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.2, 1.4, u.p0.y);
    var col = vec3f(0.0);
    for (var l = 0; l < 3; l = l + 1) {
        let fl = f32(l);
        let sc = 5.0 + fl * 4.0;
        let q = base * sc + vec2f(0.0, t * (1.2 + fl * 0.8)) + vec2f(fl * 7.1, fl * 3.3);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 23.0);
                if h.x > u.p0.x * 0.8 {
                    continue;
                }
                let h2 = hash22(cell * 1.3 + 8.1);
                let sway = 0.35 * sin(t * (1.0 + h2.x * 2.0) + h.y * 6.28);
                let pos = cell + vec2f(0.5 + sway * 0.6 + (h2.x - 0.5) * 0.4, 0.5 + (h2.y - 0.5) * 0.6);
                let dq = (q - pos) / sc;
                let d = length(vec2f(dq.x, dq.y * 0.55));
                let r = mix(0.0025, 0.007, u.p0.z) * (0.5 + h2.y);
                let flick = 0.5 + 0.5 * sin(t * 8.0 * (0.5 + h2.x) + h.y * 50.0);
                let core = exp(-d * d / (r * r));
                let halo = exp(-d * d / (r * r * 12.0)) * 0.4;
                let heat = mix(u.c0.rgb, vec3f(1.0, 0.9, 0.6), core);
                col = col + heat * (core * 1.2 + halo) * (0.3 + 0.7 * flick) * (1.0 - fl * 0.2);
            }
        }
    }
    let warm = u.c0.rgb * 0.06 * u.p0.x * uv.y * uv.y;
    return fx_put(c.rgb + col + warm, c.a);
}

// Slow, low fog that thickens towards the ground.
// p0 = (amount, speed, height), c0 = color.
fn fx_mist(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let t = u.time * mix(0.02, 0.2, u.p0.y);
    let q = vec2f(uv.x * aspect(), uv.y);
    let n = fbm(q * 1.6 + vec2f(t, -t * 0.3)) * 0.65 + fbm(q * 3.4 + vec2f(-t * 1.4, t * 0.5) + 9.0) * 0.35;
    let ground = smoothstep(1.0 - mix(0.25, 1.0, u.p0.z), 1.0, uv.y);
    let fog = smoothstep(0.25, 0.7, n) * (0.4 + 0.6 * ground) * u.p0.x;
    return fx_put(mix(c.rgb, u.c0.rgb, fog * 0.85) + vec3f(fog * 0.04), c.a);
}


// ---- Blur or pixelate one area --------------------------------------------

// How much of the chosen area covers `uv` (0 outside, 1 inside, soft across
// the feather). p0 = (shape, strength, centre x, centre y), p1 = (width,
// height, feather, outside). Width and height are fractions of the frame's;
// shape 0 is an ellipse, 1 a rectangle, 2 a rounded rectangle. "Outside"
// swaps the area for everything around it.
fn fx_area_mask(uv: vec2f) -> f32 {
    let half = max(u.p1.xy, vec2f(0.004)) * vec2f(aspect(), 1.0) * 0.5;
    let p = centered(uv) - centered(u.p0.zw);
    let small = min(half.x, half.y);
    var d: f32;
    if u.p0.x < 0.5 {
        d = (length(p / half) - 1.0) * small;
    } else {
        var r = 0.0;
        if u.p0.x > 1.5 {
            r = small * 0.4;
        }
        let q = abs(p) - half + vec2f(r);
        d = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
    }
    let f = max(u.p1.z * small * 0.9, fx_pxh());
    var m = 1.0 - smoothstep(-f, f, d);
    if u.p1.w > 0.5 {
        m = 1.0 - m;
    }
    return m;
}

// Blur inside the area (two passes, like Blur). The strength reaches 9% of
// the picture height, enough to hide a face.
fn fx_blur_area(uv: vec2f) -> vec4f {
    return fx_gaussian(uv, u.p0.y * 1.5 * fx_area_mask(uv));
}

// Pixelate inside the area. p0.y = block size.
fn fx_pixelate_area(uv: vec2f) -> vec4f {
    let m = fx_area_mask(uv);
    if m <= 0.0 {
        return sa(uv);
    }
    let cell = max(mix(0.006, 0.12, u.p0.y) * u.dst_size.y, 2.0);
    let p = (floor(uv * u.dst_size / cell) + 0.5) * cell / u.dst_size;
    return mix(sa(uv), sa(p), m);
}

// Frosted glass. p0 = (amount, grain size).
fn fx_frosted(uv: vec2f) -> vec4f {
    let cell = max(mix(0.0012, 0.008, u.p0.y), fx_pxh());
    let p = centered(uv);
    let g = floor(p / cell);
    let spread = u.p0.x * 0.03;
    var acc = vec4f(0.0);
    for (var i = 0; i < 4; i = i + 1) {
        let o = (hash22(g + f32(i) * 13.7) - 0.5) * 2.0 * spread;
        acc = acc + sa(fx_uv(p + o));
    }
    let s = unpremul(acc * 0.25);
    let rgb = mix(s.rgb, vec3f(0.9, 0.94, 1.0), 0.12 * u.p0.x);
    return fx_put(rgb, s.a);
}

// ---- Color & stylize -----------------------------------------------------

// Posterize. p0.x = number of levels per channel (2..16).
fn fx_posterize(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let n = floor(clamp(u.p0.x, 2.0, 16.0) + 0.5);
    let l = luma(c.rgb);
    let sat = mix(vec3f(l), c.rgb, 1.2);
    let x = clamp(sat, vec3f(0.0), vec3f(1.0)) * (n - 1.0);
    let q = (floor(x) + smoothstep(vec3f(0.45), vec3f(0.55), fract(x))) / (n - 1.0);
    return fx_put(q, c.a);
}

// Color splash: keeps one hue (c0) and turns the rest grey.
// p0 = (tolerance, softness).
fn fx_color_splash(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let hsv = rgb2hsv(c.rgb);
    let key = rgb2hsv(u.c0.rgb);
    var dh = abs(hsv.x - key.x);
    dh = min(dh, 1.0 - dh);
    let tol = mix(0.01, 0.2, u.p0.x);
    let soft = mix(0.005, 0.15, u.p0.y);
    var keep = 1.0 - smoothstep(tol, tol + soft, dh);
    keep = keep * smoothstep(0.1, 0.3, hsv.y);
    let g = luma(c.rgb);
    let grey = vec3f((g - 0.5) * 1.1 + 0.5);
    let vivid = mix(vec3f(g), c.rgb, 1.25);
    return fx_put(mix(grey, vivid, keep), c.a);
}

// Duotone: brightness mapped from c0 (shadows) to c1 (highlights).
// p0.x = contrast.
fn fx_duotone(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    var l = luma(c.rgb);
    l = clamp((l - 0.5) * (1.0 + u.p0.x * 1.5) + 0.5, 0.0, 1.0);
    l = smoothstep(0.0, 1.0, l);
    return fx_put(mix(u.c0.rgb, u.c1.rgb, l), c.a);
}

// Pencil sketch. p0 = (strength, color).
fn fx_sketch(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let g = fx_sobel(uv, fx_edge_step(1.3));
    let e = clamp(length(g) * mix(0.8, 3.5, u.p0.x) - 0.04, 0.0, 1.0);
    let l = luma(c.rgb);
    let dark = 1.0 - l;
    let p = centered(uv);
    let h1 = fx_stripes((p.x + p.y) * 0.7071, 110.0, 0.0);
    let h2 = fx_stripes((p.x - p.y) * 0.7071, 110.0, 0.25);
    let tone1 = smoothstep(0.25, 0.65, dark) * (0.25 + 0.75 * h1);
    let tone2 = smoothstep(0.55, 0.9, dark) * (0.25 + 0.75 * h2);
    let shade = 1.0 - (tone1 * 0.35 + tone2 * 0.3) * (0.6 + u.p0.x * 0.6);
    let grain = (hash12(floor(p / max(0.0015, fx_pxh()))) - 0.5) * 0.04;
    let paper = vec3f(0.97, 0.955, 0.92);
    let graphite = paper * (shade * (1.0 - e * 0.85) + grain);
    let tinted = graphite * mix(vec3f(1.0), c.rgb / max(max(c.r, max(c.g, c.b)), 0.2), 0.75);
    return fx_put(mix(graphite, tinted, u.p0.y), c.a);
}

// Comic book: flat colors, ink outlines and halftone shading.
// p0 = (outlines, color levels).
fn fx_comic(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    var hsv = rgb2hsv(c.rgb);
    hsv.y = min(hsv.y * 1.4, 1.0);
    let n = floor(mix(3.0, 8.0, u.p0.y));
    let zx = hsv.z * n;
    hsv.z = (floor(zx) + smoothstep(0.42, 0.58, fract(zx))) / n;
    var rgb = hsv2rgb(hsv);
    // Halftone dots in the shadows.
    let cell = 0.014;
    let p = rot2(0.785) * centered(uv) / cell;
    let d = length(fract(p) - 0.5);
    let dark = 1.0 - luma(c.rgb);
    let r = smoothstep(0.35, 0.9, dark) * 0.5;
    let aa = fx_pxh() / cell;
    let dots = 1.0 - smoothstep(r - aa, r + aa, d);
    let avg = 3.14159 * r * r;
    let dm = mix(dots, avg, smoothstep(0.25, 0.5, aa));
    rgb = rgb * (1.0 - dm * 0.45);
    // Ink outlines.
    let g = fx_sobel(uv, fx_edge_step(mix(1.0, 2.5, u.p0.x)));
    let ink = smoothstep(0.12, 0.3, length(g) * mix(0.6, 1.6, u.p0.x));
    rgb = mix(rgb, vec3f(0.06, 0.05, 0.08), ink);
    return fx_put(rgb, c.a);
}

// Emboss relief. p0 = (strength, angle in degrees, color).
fn fx_emboss(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let ang = radians(u.p0.y);
    let o = fx_edge_step(2.0) * vec2f(cos(ang), sin(ang));
    let d = fx_l(uv - o) - fx_l(uv + o);
    let relief = 0.5 + d * mix(1.5, 10.0, u.p0.x);
    let grey = vec3f(clamp(relief, 0.0, 1.0)) * vec3f(1.0, 0.98, 0.95);
    let colored = c.rgb + vec3f((relief - 0.5) * 1.2);
    return fx_put(mix(grey, colored, u.p0.z), c.a);
}

// Thermal camera palette. p0.x = amount.
fn fx_thermal_ramp(t: f32) -> vec3f {
    var pal = array<vec3f, 7>(
        vec3f(0.02, 0.0, 0.12),
        vec3f(0.18, 0.02, 0.6),
        vec3f(0.62, 0.0, 0.62),
        vec3f(0.95, 0.12, 0.18),
        vec3f(1.0, 0.55, 0.0),
        vec3f(1.0, 0.92, 0.25),
        vec3f(1.0, 1.0, 0.92),
    );
    let x = clamp(t, 0.0, 1.0) * 6.0;
    let i = min(u32(x), 5u);
    return mix(pal[i], pal[i + 1u], x - f32(i));
}

fn fx_thermal(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let b = unpremul(fx_disc(uv, 0.006, 8));
    let l = smoothstep(0.05, 0.95, luma(b.rgb));
    return fx_put(mix(c.rgb, fx_thermal_ramp(l), u.p0.x), c.a);
}

// Night vision. p0 = (brightness, noise, mask: none / scope / binoculars).
fn fx_night_vision(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let frame = floor(u.time * 24.0);
    var l = luma(c.rgb);
    l = pow(l, 0.75) * mix(0.9, 1.8, u.p0.x);
    let gp = floor(centered(uv) / max(0.0018, fx_pxh()));
    let n = hash12(gp + vec2f(frame * 7.0, frame * 3.0)) - 0.5;
    l = l + n * u.p0.y * 0.35;
    l = l * (0.9 + 0.1 * fx_stripes(uv.y, 220.0, 0.0));
    l = l * (1.0 + 0.04 * sin(u.time * 31.0));
    var rgb = vec3f(0.12, 1.0, 0.3) * l + vec3f(0.0, 0.03, 0.0);
    rgb = rgb + vec3f(max(l - 0.85, 0.0)) * 0.8;
    let p = centered(uv);
    rgb = rgb * (1.0 - smoothstep(0.35, 1.0, length(p * vec2f(0.75, 1.1))) * 0.6);
    let mode = u32(u.p0.z + 0.5);
    var mask = 1.0;
    if mode == 1u {
        mask = fx_fill(length(p) - 0.47);
    } else if mode == 2u {
        let ox = min(0.27, aspect() * 0.25);
        let d = min(length(p - vec2f(-ox, 0.0)), length(p - vec2f(ox, 0.0)));
        mask = fx_fill(d - 0.4);
    }
    return vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)) * mask * c.a, mix(1.0, c.a, mask));
}

// ---- Retro ---------------------------------------------------------------

// CRT monitor: curved glass, scanlines and an RGB shadow mask.
// p0 = (scanlines, curvature).
fn fx_crt(uv: vec2f) -> vec4f {
    var q = uv * 2.0 - 1.0;
    let k = u.p0.y * 0.18;
    q = q * (1.0 + k * vec2f(q.y * q.y, q.x * q.x));
    let cuv = q * 0.5 + 0.5;
    let a = aspect();
    let box = fx_sd_box((cuv - 0.5) * vec2f(a, 1.0), vec2f(a * 0.5, 0.5), 0.04 + k * 0.2);
    let inside = fx_fill(box);
    let o = vec2f(0.0015 / a, 0.0);
    let c = fx_get(cuv);
    var rgb = (c.rgb * 2.0 + fx_get(cuv - o).rgb + fx_get(cuv + o).rgb) * 0.25;
    rgb = rgb + unpremul(blur_a(cuv, 0.01)).rgb * 0.15;
    let s = u.p0.x;
    let scan = fx_stripes(cuv.y, 200.0, 0.0);
    rgb = rgb * (1.0 - s * 0.55 * (1.0 - scan)) * (1.0 + s * 0.3);
    let xh = cuv.x * a;
    let m = vec3f(fx_stripes(xh, 220.0, 0.0), fx_stripes(xh, 220.0, 0.333), fx_stripes(xh, 220.0, 0.667));
    rgb = rgb * mix(vec3f(1.0), 0.55 + m * 0.75, s * 0.6);
    rgb = rgb * (1.0 - smoothstep(0.55, 1.15, length(q * vec2f(0.8, 1.0))) * 0.5);
    rgb = rgb * (1.0 + 0.015 * sin(u.time * 50.0));
    let col = clamp(rgb, vec3f(0.0), vec3f(1.0)) * c.a;
    return vec4f(col * inside, c.a * inside + (1.0 - inside));
}

// LED wall / dot matrix. p0 = (dot size, glow).
fn fx_led(uv: vec2f) -> vec4f {
    let cell = mix(0.008, 0.035, u.p0.x);
    let p = centered(uv);
    let g = floor(p / cell);
    let center = (g + 0.5) * cell;
    let c = unpremul(fx_disc(fx_uv(center), cell * 0.35, 4));
    let lit = c.rgb * 1.35;
    let d = length(p - center) / cell;
    let aa = fx_pxh() / cell;
    let dots = 1.0 - smoothstep(0.38 - aa, 0.38 + aa, d);
    let glow = exp(-d * d * 7.0) * u.p0.y * 0.6;
    let avg = 0.45 + u.p0.y * 0.15;
    let k = mix(dots + glow, avg, smoothstep(0.2, 0.4, aa));
    let rgb = lit * k + vec3f(0.015);
    return vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)), 1.0) * c.a + vec4f(0.0, 0.0, 0.0, 1.0 - c.a);
}

// 4x4 ordered dither threshold in 0..1.
fn fx_bayer4(p: vec2<i32>) -> f32 {
    let x = p.x & 3;
    let y = p.y & 3;
    let a = x ^ y;
    let v = ((a & 1) << 3u) | ((y & 1) << 2u) | (((a >> 1u) & 1) << 1u) | ((y >> 1u) & 1);
    return (f32(v) + 0.5) / 16.0;
}

// Pixel art: big pixels, a small palette and ordered dithering.
// p0 = (pixel size, colors).
fn fx_pixel_art(uv: vec2f) -> vec4f {
    let cell = mix(0.006, 0.045, u.p0.x);
    let p = centered(uv);
    let g = floor(p / cell);
    let c = unpremul(fx_disc(fx_uv((g + 0.5) * cell), cell * 0.3, 4));
    let n = floor(mix(3.0, 9.0, u.p0.y));
    let l = luma(c.rgb);
    let sat = clamp(mix(vec3f(l), c.rgb, 1.25), vec3f(0.0), vec3f(1.0));
    let th = fx_bayer4(vec2<i32>(g)) - 0.5;
    let q = clamp(floor(sat * (n - 1.0) + 0.5 + th * 0.3), vec3f(0.0), vec3f(n - 1.0)) / (n - 1.0);
    return fx_put(q, select(0.0, 1.0, c.a > 0.5));
}

// ---- More glitches -------------------------------------------------------

// Chromatic aberration growing towards the edges. p0.x = amount.
fn fx_chromatic(uv: vec2f) -> vec4f {
    let a = aspect();
    let diag = 0.5 * sqrt(a * a + 1.0);
    let p = centered(uv);
    let r = length(p) / diag;
    let k = u.p0.x * 0.12 * (0.25 + r * r);
    var acc = vec3f(0.0);
    var wsum = vec3f(0.0);
    var alpha = 0.0;
    for (var i = 0; i < 9; i = i + 1) {
        let t = f32(i) / 8.0;
        let c = sa(fx_uv(p * (1.0 + (t - 0.5) * k)));
        let w = vec3f(clamp(1.5 - t * 3.0, 0.0, 1.0), 1.0 - abs(t * 2.0 - 1.0), clamp(t * 3.0 - 1.5, 0.0, 1.0));
        acc = acc + c.rgb * w;
        wsum = wsum + w;
        alpha = max(alpha, c.a);
    }
    return vec4f(min(acc / wsum, vec3f(alpha)), alpha);
}

// Big glitch slices shifting sideways in bursts. p0 = (amount, speed).
fn fx_slice(uv: vec2f) -> vec4f {
    let amount = u.p0.x;
    let frame = floor(u.time * mix(3.0, 16.0, u.p0.y));
    let on = select(0.4, 1.0, hash11(frame * 1.31 + 0.5) < 0.4 + 0.5 * amount);
    let yb = uv.y + 0.06 * (vnoise(vec2f(uv.y * 4.0, frame)) - 0.5);
    let bands = mix(5.0, 12.0, hash11(frame + 3.0));
    let band = floor(yb * bands);
    let r = hash12(vec2f(band, frame));
    var p = uv;
    var tint = vec3f(1.0);
    var split = 0.002;
    if r < 0.5 {
        let shift = hash12(vec2f(frame, band)) - 0.5;
        p.x = fract(p.x + (shift + sign(shift) * 0.15) * 0.45 * amount * on);
        split = 0.015 * amount * on;
        if r < 0.1 {
            tint = vec3f(0.55, 1.1, 1.15);
        } else if r < 0.18 {
            tint = vec3f(1.15, 0.6, 1.1);
        }
    }
    let o = vec2f(split, 0.0);
    let c = sa(p);
    let rgb = vec3f(sa(p + o).r, c.g, sa(p - o).b) * tint;
    return vec4f(min(rgb, vec3f(c.a)), c.a);
}

// Bad TV signal: rolling picture, torn lines, interference and static.
// p0 = (amount, speed).
fn fx_static(uv: vec2f) -> vec4f {
    let amount = u.p0.x;
    let t = u.time * mix(0.5, 2.0, u.p0.y);
    let frame = floor(t * 24.0);
    var p = uv;
    // Vertical roll in bursts.
    let burst = floor(t * 0.7);
    var roll = 0.0;
    if hash11(burst + 0.3) < 0.3 + amount * 0.6 {
        let f = fract(t * 0.7);
        roll = f * f * (0.2 + hash11(burst) * 0.6);
    }
    p.y = fract(p.y + roll);
    // Torn bands and jittering lines.
    let band = floor(uv.y * 14.0 + t * 2.0);
    if hash12(vec2f(band, frame)) < amount * 0.35 {
        p.x = p.x + (hash12(vec2f(frame, band)) - 0.5) * 0.12 * amount;
    }
    let line = floor(uv.y * 120.0);
    if hash12(vec2f(line, frame)) < amount * 0.2 {
        p.x = p.x + (hash12(vec2f(frame, line)) - 0.5) * 0.05 * amount;
    }
    p.x = p.x + sin(uv.y * 30.0 + t * 8.0) * 0.004 * amount;
    let o = vec2f(0.006 * amount, 0.0);
    let c = fx_get(p);
    var rgb = vec3f(fx_get(p + o).r, c.g, fx_get(p - o).b);
    rgb = mix(rgb, vec3f(luma(rgb)), 0.45 * amount);
    rgb = (rgb - 0.5) * (1.0 - 0.2 * amount) + 0.5;
    // Black bar at the roll seam.
    let seam = fract(uv.y + roll);
    let bar = select(0.0, 1.0 - smoothstep(0.0, 0.04, min(seam, 1.0 - seam)), roll > 0.0);
    rgb = rgb * (1.0 - bar * 0.85);
    // Static, stronger inside a bright interference bar drifting down.
    let gp = floor(centered(uv) / max(0.0025, fx_pxh()));
    let snow = hash12(gp + vec2f(frame * 13.0, frame * 7.0));
    let bd = abs(fract(uv.y - t * 0.35) - 0.5);
    let inter = 1.0 - smoothstep(0.02, 0.08, bd);
    let streak = step(0.6, hash12(vec2f(floor(uv.y * 240.0), frame))) * inter;
    rgb = mix(rgb, vec3f(snow), amount * (0.2 + 0.45 * inter));
    rgb = rgb + vec3f(streak * amount * 0.35);
    return fx_put(rgb, c.a);
}

// Datamosh: blocks of the picture jumping around, changing colors.
// p0 = (amount, speed, block size).
fn fx_block(uv: vec2f) -> vec4f {
    let bs = mix(0.03, 0.15, u.p0.z);
    let frame = floor(u.time * mix(3.0, 15.0, u.p0.y));
    let p = centered(uv);
    let g = floor(p / vec2f(bs * 1.7, bs));
    let h = hash12(g + frame * 7.13);
    var q = p;
    var mode = 0;
    if h < u.p0.x * 0.4 {
        q = p + (hash22(g + frame) - 0.5) * vec2f(0.5, 0.2);
        mode = 1;
        if h < u.p0.x * 0.15 {
            mode = 2;
        }
    }
    // Smaller blocks smeared to flat color.
    let g2 = floor(p / (bs * 0.35));
    if hash12(g2 + frame * 3.1) < u.p0.x * 0.12 {
        q = (g2 + 0.5) * bs * 0.35;
        mode = 3;
    }
    let c = fx_get(fx_uv(q));
    var rgb = c.rgb;
    if mode == 2 {
        rgb = rgb.gbr;
    } else if mode == 3 {
        rgb = floor(rgb * 4.0) / 3.0;
    }
    let o = vec2f(0.006 * u.p0.x, 0.0);
    rgb.r = mix(rgb.r, fx_get(fx_uv(q) + o).r, f32(mode == 1));
    return fx_put(rgb, c.a);
}

// ---- Light ---------------------------------------------------------------

// Bright parts of A spread over `radius` height units (48 taps).
fn fx_glow_gather(uv: vec2f, radius: f32, threshold: f32) -> vec3f {
    let r = vec2f(radius / aspect(), radius);
    var acc = vec3f(0.0);
    var wsum = 0.0;
    for (var i = 0; i < 48; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let rad = sqrt((fi + 0.5) / 48.0);
        let c = sa(uv + vec2f(cos(ang), sin(ang)) * rad * r).rgb;
        let w = exp(-rad * rad * 3.0);
        let b = max(c - vec3f(threshold), vec3f(0.0)) / max(1.0 - threshold, 0.05);
        acc = acc + b * w;
        wsum = wsum + w;
    }
    return acc / wsum;
}

// Bloom: highlights glow. p0 = (intensity, threshold, radius).
fn fx_bloom(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let th = mix(0.3, 0.9, u.p0.y);
    let r = mix(0.01, 0.12, u.p0.z);
    let g = fx_glow_gather(uv, r, th) * 0.5 + fx_glow_gather(uv, r * 0.35, th) * 0.5;
    return fx_put(fx_screen(c.rgb, g * u.p0.x * 5.0), c.a);
}

// Soft dreamy glow over the whole picture. p0 = (amount, radius).
fn fx_soft_glow(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let b = unpremul(fx_disc(uv, mix(0.01, 0.06, u.p0.y), 40)).rgb;
    let glow = b * b * 1.2;
    var rgb = fx_screen(c.rgb, glow * u.p0.x);
    rgb = mix(rgb, mix(rgb, b, 0.5), u.p0.x * 0.35);
    rgb = rgb * vec3f(1.02, 1.0, 0.98);
    return fx_put(rgb, c.a);
}

// Light rays streaming from a point. p0 = (intensity, x, y, length),
// c0 = tint.
fn fx_light_rays(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let lp = u.p0.yz;
    let dv = uv - lp;
    let len = mix(0.2, 0.95, u.p0.w);
    let jitter = hash12(uv * u.dst_size);
    var acc = vec3f(0.0);
    var decay = 1.0;
    for (var i = 0; i < 40; i = i + 1) {
        let t = (f32(i) + jitter) / 40.0 * len;
        let s = sa(uv - dv * t).rgb;
        let b = smoothstep(0.55, 0.95, luma(s));
        acc = acc + s * b * decay;
        decay = decay * 0.96;
    }
    acc = acc / 18.0;
    let pd = centered(uv) - centered(lp);
    let ang = atan2(pd.y, pd.x);
    let beams = 0.55 + 0.45 * vnoise(vec2f(ang * 9.0 + 40.0, u.time * 0.4))
        * (0.6 + 0.4 * vnoise(vec2f(ang * 23.0, u.time * 0.7 + 9.0)));
    let rays = acc * beams * u.c0.rgb * u.p0.x * 1.6;
    return fx_put(fx_screen(c.rgb, rays), c.a);
}

// Lens flare from a light source. p0 = (intensity, x, y), c0 = tint.
fn fx_lens_flare(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let lp = centered(u.p0.yz);
    let p = centered(uv);
    let d = p - lp;
    let r = length(d);
    let tint = u.c0.rgb;
    let shimmer = 0.95 + 0.05 * sin(u.time * 7.0);
    var col = tint * (exp(-r * r * 60.0) * 1.2 + exp(-r * 5.0) * 0.18);
    col = col + vec3f(0.55, 0.75, 1.0) * exp(-abs(d.y) * 160.0) * exp(-abs(d.x) * 2.2) * 0.7;
    let ang = atan2(d.y, d.x);
    col = col + tint * pow(abs(cos(ang * 3.0 + 0.4)), 60.0) * exp(-r * 6.0) * 0.5;
    // Rainbow halo.
    let ring = exp(-pow((r - 0.3) * 28.0, 2.0));
    col = col + hsv2rgb(vec3f(fract(r * 3.0 + 0.1), 0.6, 1.0)) * ring * 0.12;
    // Ghosts along the line through the centre.
    var ghosts = array<vec4f, 5>(
        vec4f(-0.25, 0.035, 0.1, 0.7),
        vec4f(-0.55, 0.07, 0.55, 0.9),
        vec4f(-0.8, 0.025, 0.3, 0.6),
        vec4f(-1.15, 0.11, 0.75, 0.5),
        vec4f(0.45, 0.05, 0.15, 0.4),
    );
    for (var i = 0; i < 5; i = i + 1) {
        let gh = ghosts[i];
        let gc = lp * gh.x;
        let gd = length(p - gc);
        let disc = 1.0 - smoothstep(gh.y * 0.85, gh.y, gd);
        let rim = smoothstep(gh.y * 0.6, gh.y * 0.95, gd) * disc;
        let gcol = hsv2rgb(vec3f(gh.z, 0.55, 1.0));
        col = col + gcol * (disc * 0.07 + rim * 0.08) * gh.w;
    }
    return fx_put(fx_screen(c.rgb, col * u.p0.x * shimmer), c.a);
}

// Neon edges glowing over a dark picture. p0 = (glow, background, speed),
// c0/c1 = the two neon colors.
fn fx_neon_edges(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let o = fx_edge_step(1.3);
    let core = clamp(length(fx_sobel(uv, o)) * 1.2 - 0.05, 0.0, 1.0);
    let rad = 0.012;
    let rr = vec2f(rad / aspect(), rad);
    var glow = 0.0;
    for (var i = 0; i < 12; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let q = uv + vec2f(cos(ang), sin(ang)) * sqrt((fi + 0.5) / 12.0) * rr;
        let gx = fx_l(q + vec2f(o.x * 2.0, 0.0)) - fx_l(q - vec2f(o.x * 2.0, 0.0));
        let gy = fx_l(q + vec2f(0.0, o.y * 2.0)) - fx_l(q - vec2f(0.0, o.y * 2.0));
        glow = glow + clamp(length(vec2f(gx, gy)) * 1.5, 0.0, 1.0);
    }
    glow = glow / 12.0;
    let m = 0.5 + 0.5 * sin(uv.x * 4.0 + uv.y * 2.5 + u.time * mix(0.0, 4.0, u.p0.z));
    let neon = mix(u.c0.rgb, u.c1.rgb, m);
    let bg = c.rgb * mix(0.05, 0.6, u.p0.y);
    let rgb = bg + neon * (core * 1.3 + glow * u.p0.x * 2.2) + vec3f(core * core * 0.5);
    return fx_put(rgb, c.a);
}

// Film burn: hot light eating in from the edges. p0 = (amount, speed),
// c0 = burn color.
fn fx_film_burn(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let t = u.time * mix(0.1, 1.2, u.p0.y);
    let p = centered(uv);
    let n = fbm(p * 1.6 + vec2f(t * 0.5, -t * 0.3));
    let n2 = fbm(p * 4.0 - vec2f(t * 0.9, t * 0.2) + 10.0);
    let edge = length(p * vec2f(0.75, 1.15));
    let pulse = 0.5 + 0.5 * sin(t * 1.7);
    let field = n * 0.85 + n2 * 0.3 + edge * 0.7 + u.p0.x * 0.5 + pulse * 0.12 - 1.32;
    let burn = smoothstep(0.0, 0.18, field);
    let fringe = smoothstep(-0.18, 0.0, field) * (1.0 - burn);
    var col = mix(u.c0.rgb, vec3f(1.0, 0.68, 0.2), smoothstep(0.1, 0.4, field));
    col = mix(col, vec3f(1.0, 0.97, 0.88), smoothstep(0.35, 0.7, field));
    var rgb = c.rgb * mix(vec3f(1.0), vec3f(1.08, 0.96, 0.85), u.p0.x * 0.6);
    rgb = mix(rgb, rgb * vec3f(1.1, 0.55, 0.35), fringe * 0.6);
    rgb = mix(rgb, fx_screen(rgb, col * 1.2), burn);
    return fx_put(rgb, c.a);
}

// ---- Particles -----------------------------------------------------------

// Rain streaks with a darker, cooler picture. p0 = (amount, speed, wind).
fn fx_rain(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let amount = u.p0.x;
    let speed = mix(0.8, 3.0, u.p0.y);
    let slant = (u.p0.z - 0.5) * 0.7;
    var rgb = mix(c.rgb, c.rgb * vec3f(0.78, 0.84, 0.95), amount * 0.8);
    var streak = 0.0;
    for (var l = 0; l < 3; l = l + 1) {
        let fl = f32(l);
        let sc = vec2f(30.0 + fl * 22.0, 3.0 + fl * 1.5);
        var p = vec2f(uv.x * aspect(), uv.y);
        p.x = p.x - p.y * slant + fl * 0.37;
        p.y = p.y - u.time * speed * (1.0 - fl * 0.2);
        let q = p * sc;
        let g = floor(q);
        let f = q - g;
        let h = hash22(g + fl * 19.0);
        if h.x < amount * 0.75 {
            let cx = 0.2 + 0.6 * h.y;
            let dx = abs(f.x - cx) / sc.x;
            let y0 = hash12(g + 7.0) * 0.3;
            let len = 0.35 + 0.3 * hash12(g + 3.0);
            let ty = (f.y - y0) / len;
            let w = max(0.0012 - fl * 0.0003, fx_pxh() * 0.6);
            let line = 1.0 - smoothstep(w * 0.4, w, dx);
            if ty > 0.0 && ty < 1.0 {
                streak = streak + line * ty * (0.75 - fl * 0.18);
            }
        }
    }
    rgb = rgb + vec3f(0.75, 0.8, 0.88) * clamp(streak, 0.0, 1.0) * 0.55;
    return fx_put(rgb, c.a);
}

// Soap bubbles floating up. p0 = (amount, speed, size).
fn fx_bubbles(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    var rgb = c.rgb;
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.04, 0.3, u.p0.y);
    for (var l = 0; l < 2; l = l + 1) {
        let fl = f32(l);
        let sc = 4.5 - fl * 1.8;
        var q = base * sc + vec2f(fl * 5.3, 0.0);
        q.y = q.y + t * sc * (1.0 + fl * 0.5);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 29.0);
                if h.x > u.p0.x * 0.8 {
                    continue;
                }
                let h2 = hash22(cell * 1.3 + 4.1);
                let sway = sin(u.time * (0.8 + h2.x) + h.y * 6.28) * 0.15;
                let pos = cell + vec2f(0.5 + (h2.x - 0.5) * 0.5 + sway, 0.5 + (h2.y - 0.5) * 0.5);
                let r = mix(0.04, 0.12, u.p0.z) * (0.6 + 0.7 * h.y) * (1.0 + fl * 0.4);
                let d = (q - pos) / sc;
                let dist = length(d) / r;
                if dist > 1.05 {
                    continue;
                }
                let aa = fx_pxh() / r;
                let inside = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, dist);
                let rim = smoothstep(0.75, 0.98, dist) * inside;
                let irid = hsv2rgb(vec3f(fract(atan2(d.y, d.x) / TAU + dist * 0.6 + u.time * 0.1 + h.y), 0.55, 1.0));
                let hl = exp(-dot(d / r - vec2f(-0.4, -0.4), d / r - vec2f(-0.4, -0.4)) * 30.0);
                rgb = mix(rgb, fx_screen(rgb, irid * 0.6), rim * 0.8);
                rgb = rgb + vec3f(0.08) * inside + vec3f(hl * 0.9);
            }
        }
    }
    return fx_put(rgb, c.a);
}

// Falling, tumbling confetti. p0 = (amount, speed, size).
fn fx_confetti(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    var rgb = c.rgb;
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.08, 0.5, u.p0.y);
    for (var l = 0; l < 3; l = l + 1) {
        let fl = f32(l);
        let sc = 16.0 - fl * 4.5;
        var q = base * sc + vec2f(fl * 2.9, fl * 1.7);
        q.y = q.y - t * sc * (0.8 + fl * 0.25);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 31.0);
                if h.x > u.p0.x * 0.7 {
                    continue;
                }
                let h2 = hash22(cell * 1.7 + 2.3);
                let sway = sin(u.time * (1.0 + h2.x * 2.0) + h.y * 6.28) * 0.3;
                let pos = cell + vec2f(0.5 + (h2.x - 0.5) * 0.4 + sway, 0.5 + (h2.y - 0.5) * 0.4);
                let local = (q - pos) / sc;
                let spin = select(-1.0, 1.0, h.y < 0.5);
                let lr = rot2(h.y * 6.28 + u.time * (1.0 + h2.y * 2.5) * spin) * local;
                let flip = cos(u.time * (3.0 + h2.x * 4.0) + h.y * 10.0);
                let sz = mix(0.006, 0.022, u.p0.z) * (0.7 + 0.6 * h2.y);
                let hs = vec2f(sz * max(abs(flip), 0.1), sz * 0.55);
                let mask = fx_fill(fx_sd_box(lr, hs, 0.0));
                if mask <= 0.0 {
                    continue;
                }
                let hue = fract(h.x * 7.31 + h2.y * 0.37);
                let col = hsv2rgb(vec3f(hue, 0.72, 0.98)) * select(0.72, 1.0, flip > 0.0);
                rgb = mix(rgb, col, mask);
            }
        }
    }
    return fx_put(rgb, c.a);
}

// Fireflies: glowing specks drifting and blinking. p0 = (amount, speed,
// size), c0 = color.
fn fx_fireflies(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.2, 1.5, u.p0.y);
    var glow = 0.0;
    for (var l = 0; l < 2; l = l + 1) {
        let fl = f32(l);
        let sc = 5.0 + fl * 3.0;
        let q = base * sc + vec2f(fl * 3.7, fl * 1.9);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 41.0);
                if h.x > u.p0.x * 0.9 {
                    continue;
                }
                let h2 = hash22(cell * 1.7 + 9.1);
                let wander = vec2f(sin(t * (0.6 + h2.x) + h.y * 6.28), cos(t * (0.5 + h2.y) + h.x * 6.28));
                let pos = cell + 0.5 + 0.35 * wander;
                let d = length(q - pos) / sc;
                let r = mix(0.003, 0.01, u.p0.z) * (0.6 + 0.8 * h2.x);
                let blink = 0.5 + 0.5 * sin(t * (1.5 + h2.y * 3.0) + h.x * 20.0);
                let core = exp(-d * d / (r * r));
                let halo = exp(-d * d / (r * r * 16.0)) * 0.45;
                glow = glow + (core + halo) * (0.25 + 0.75 * blink * blink) * (1.0 - fl * 0.35);
            }
        }
    }
    let rgb = c.rgb + u.c0.rgb * glow + vec3f(max(glow - 1.0, 0.0) * 0.5);
    return fx_put(rgb, c.a);
}

// Floating hearts. p0 = (amount, speed, size), c0 = color.
fn fx_hearts(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    var rgb = c.rgb;
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.04, 0.35, u.p0.y);
    for (var l = 0; l < 2; l = l + 1) {
        let fl = f32(l);
        let sc = 6.0 - fl * 2.5;
        var q = base * sc + vec2f(fl * 4.1, 0.0);
        q.y = q.y + t * sc * (1.0 + fl * 0.4);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 23.0);
                if h.x > u.p0.x * 0.75 {
                    continue;
                }
                let h2 = hash22(cell * 1.9 + 5.3);
                let sway = sin(u.time * (0.8 + h2.x) + h.y * 6.28) * 0.2;
                let pos = cell + vec2f(0.5 + (h.y - 0.5) * 0.4 + sway, 0.5 + (h2.y - 0.5) * 0.4);
                let size = mix(0.2, 0.5, u.p0.z) * (0.65 + 0.5 * h2.x);
                var lq = (q - pos) / size;
                lq = rot2(sin(u.time * (0.7 + h.x) + h2.y * 6.0) * 0.3) * lq;
                let hp = vec2f(lq.x, 0.55 - lq.y);
                let d = fx_sd_heart(hp) * size / sc;
                let mask = fx_fill(d);
                if mask <= 0.0 {
                    continue;
                }
                var col = mix(u.c0.rgb, vec3f(1.0, 0.62, 0.78), h2.y * 0.7);
                let shine = exp(-dot(hp - vec2f(-0.25, 0.75), hp - vec2f(-0.25, 0.75)) * 25.0);
                col = col * (0.85 + 0.15 * hp.y) + vec3f(shine * 0.5);
                rgb = mix(rgb, col, mask * (0.92 - fl * 0.15));
            }
        }
    }
    return fx_put(rgb, c.a);
}

// Twinkling star sparkles. p0 = (amount, speed, size), c0 = color.
fn fx_sparkles(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.3, 2.5, u.p0.y);
    var light = 0.0;
    for (var l = 0; l < 2; l = l + 1) {
        let fl = f32(l);
        let sc = 6.0 + fl * 4.0;
        let q = base * sc + vec2f(fl * 2.3, fl * 7.1);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 17.0);
                if h.x > u.p0.x * 0.75 {
                    continue;
                }
                let h2 = hash22(cell * 1.3 + 3.7);
                let pos = cell + 0.2 + 0.6 * h2;
                let life = fract(t * (0.3 + 0.4 * h.y) + h2.x);
                let tw = pow(sin(life * PI), 3.0);
                let s = mix(0.012, 0.045, u.p0.z) * (0.5 + h.y) * tw;
                if s < 0.0005 {
                    continue;
                }
                let a = rot2(h2.y * 0.8 + t * 0.3) * ((q - pos) / sc) / s;
                let cross = exp(-abs(a.x) * 2.0) * exp(-abs(a.y) * 24.0) + exp(-abs(a.y) * 2.0) * exp(-abs(a.x) * 24.0);
                let core = exp(-dot(a, a) * 8.0);
                light = light + (cross * 0.9 + core) * tw;
            }
        }
    }
    let rgb = c.rgb + u.c0.rgb * light + vec3f(max(light - 0.8, 0.0) * 0.4);
    return fx_put(rgb, c.a);
}

// ---- Overlays ------------------------------------------------------------

// Vignette. p0 = (amount, size, softness, roundness), c0 = color.
fn fx_vignette(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = (uv - 0.5) * 2.0;
    let sx = mix(1.0, aspect(), u.p0.w);
    let d = length(vec2f(p.x * sx, p.y)) / length(vec2f(sx, 1.0));
    let inner = mix(0.15, 0.85, u.p0.y);
    let m = smoothstep(inner, inner + mix(0.05, 0.8, u.p0.z), d) * u.p0.x;
    return fx_put(mix(c.rgb, u.c0.rgb, m), c.a);
}

// Big soft out-of-focus lights drifting over the picture.
// p0 = (amount, speed, size), c0/c1 = colors.
fn fx_bokeh_lights(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let base = vec2f(uv.x * aspect(), uv.y);
    let t = u.time * mix(0.02, 0.25, u.p0.y);
    var light = vec3f(0.0);
    for (var l = 0; l < 2; l = l + 1) {
        let fl = f32(l);
        let sc = 2.4 + fl * 1.8;
        let q = base * sc + vec2f(t * sc * (0.6 + 0.3 * fl), -t * sc * 0.35) + vec2f(fl * 3.3, fl * 1.1);
        let g = floor(q);
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + fl * 11.0);
                if h.x > u.p0.x * 0.9 {
                    continue;
                }
                let h2 = hash22(cell * 1.1 + 2.9);
                let pos = cell + 0.2 + 0.6 * h2;
                let r = mix(0.03, 0.11, u.p0.z) * (0.6 + 0.8 * h.y) * (1.0 - fl * 0.3);
                let d = length(q - pos) / sc;
                let soft = max(r * 0.08, fx_pxh());
                let disc = 1.0 - smoothstep(r - soft, r, d);
                let rim = smoothstep(r * 0.6, r * 0.95, d) * disc;
                let pulse = 0.75 + 0.25 * sin(u.time * (0.5 + h.y) + h2.x * 6.28);
                let col = mix(u.c0.rgb, u.c1.rgb, h2.y);
                light = light + col * (disc * 0.28 + rim * 0.14) * pulse * (1.0 - fl * 0.25);
            }
        }
    }
    return fx_put(fx_screen(c.rgb, light), c.a);
}

// Line segments of the letters R, E and C in a 0.55 x 1 box (y up).
fn fx_rec_text(p: vec2f) -> f32 {
    var d = 1e5;
    // R
    d = min(d, fx_sd_segment(p, vec2f(0.0, 0.0), vec2f(0.0, 1.0)));
    d = min(d, fx_sd_segment(p, vec2f(0.0, 1.0), vec2f(0.36, 1.0)));
    d = min(d, fx_sd_segment(p, vec2f(0.36, 1.0), vec2f(0.5, 0.88)));
    d = min(d, fx_sd_segment(p, vec2f(0.5, 0.88), vec2f(0.5, 0.67)));
    d = min(d, fx_sd_segment(p, vec2f(0.5, 0.67), vec2f(0.36, 0.55)));
    d = min(d, fx_sd_segment(p, vec2f(0.36, 0.55), vec2f(0.0, 0.55)));
    d = min(d, fx_sd_segment(p, vec2f(0.22, 0.55), vec2f(0.5, 0.0)));
    // E
    let e = p - vec2f(0.75, 0.0);
    d = min(d, fx_sd_segment(e, vec2f(0.0, 0.0), vec2f(0.0, 1.0)));
    d = min(d, fx_sd_segment(e, vec2f(0.0, 1.0), vec2f(0.48, 1.0)));
    d = min(d, fx_sd_segment(e, vec2f(0.0, 0.52), vec2f(0.4, 0.52)));
    d = min(d, fx_sd_segment(e, vec2f(0.0, 0.0), vec2f(0.48, 0.0)));
    // C
    let k = p - vec2f(1.45, 0.0);
    d = min(d, fx_sd_segment(k, vec2f(0.52, 0.84), vec2f(0.38, 1.0)));
    d = min(d, fx_sd_segment(k, vec2f(0.38, 1.0), vec2f(0.14, 1.0)));
    d = min(d, fx_sd_segment(k, vec2f(0.14, 1.0), vec2f(0.0, 0.82)));
    d = min(d, fx_sd_segment(k, vec2f(0.0, 0.82), vec2f(0.0, 0.18)));
    d = min(d, fx_sd_segment(k, vec2f(0.0, 0.18), vec2f(0.14, 0.0)));
    d = min(d, fx_sd_segment(k, vec2f(0.14, 0.0), vec2f(0.38, 0.0)));
    d = min(d, fx_sd_segment(k, vec2f(0.38, 0.0), vec2f(0.52, 0.16)));
    return d;
}

// Camera viewfinder: corner marks, centre cross, blinking REC and battery.
// p0.x = opacity, c0 = line color.
fn fx_viewfinder(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let w = aspect();
    let ph = vec2f(uv.x * w, uv.y);
    let m = 0.07;
    let stroke = 0.0035;
    // Corner brackets (all four by symmetry).
    let qc = vec2f(min(ph.x, w - ph.x), min(ph.y, 1.0 - ph.y));
    var d = min(
        fx_sd_segment(qc, vec2f(m, m), vec2f(m + 0.09, m)),
        fx_sd_segment(qc, vec2f(m, m), vec2f(m, m + 0.09)),
    ) - stroke;
    // Centre cross.
    let qx = abs(ph - vec2f(w * 0.5, 0.5));
    d = min(d, min(fx_sd_segment(qx, vec2f(0.012, 0.0), vec2f(0.04, 0.0)), fx_sd_segment(qx, vec2f(0.0, 0.012), vec2f(0.0, 0.04))) - stroke * 0.8);
    // REC text.
    let th = 0.036;
    let to = vec2f(m + 0.075, m + 0.03);
    let tp = vec2f((ph.x - to.x) / th, (to.y + th - ph.y) / th);
    d = min(d, fx_rec_text(tp) * th - stroke * 1.1);
    // Battery.
    let bc = vec2f(w - m - 0.06, m + 0.048);
    let bq = ph - bc;
    d = min(d, abs(fx_sd_box(bq, vec2f(0.04, 0.018), 0.004)) - stroke * 0.8);
    d = min(d, fx_sd_box(bq - vec2f(0.046, 0.0), vec2f(0.004, 0.008), 0.001));
    for (var i = 0; i < 3; i = i + 1) {
        d = min(d, fx_sd_box(bq - vec2f(-0.024 + f32(i) * 0.024, 0.0), vec2f(0.0085, 0.01), 0.001));
    }
    let line = fx_fill(d);
    let shadow = fx_fill(d - 0.004) * 0.35;
    // Blinking red dot.
    let blink = select(0.0, 1.0, fract(u.time) < 0.6);
    let dot = fx_fill(length(ph - vec2f(m + 0.04, m + 0.048)) - 0.016) * blink;
    let k = clamp(u.p0.x, 0.0, 1.0);
    var rgb = c.rgb * (1.0 - shadow * k);
    rgb = mix(rgb, u.c0.rgb, line * k);
    rgb = mix(rgb, vec3f(0.95, 0.12, 0.1), dot * k);
    let a = max(c.a, max(line, dot) * k);
    return fx_put(rgb, a);
}

// ---- Frames --------------------------------------------------------------

// Instant photo border, thicker at the bottom. p0 = (border, vintage),
// c0 = paper color.
fn fx_polaroid(uv: vec2f) -> vec4f {
    let w = aspect();
    let ph = vec2f(uv.x * w, uv.y);
    let m = mix(0.03, 0.09, u.p0.x);
    let bottom = m * 4.2;
    let win_min = vec2f(m, m);
    let win_max = vec2f(w - m, 1.0 - bottom);
    let ws = win_max - win_min;
    let l = (ph - win_min) / ws;
    let wc = (win_min + win_max) * 0.5;
    let d = fx_sd_box(ph - wc, ws * 0.5, 0.0);
    let inside = fx_fill(d);
    let c = fx_get(fx_cover(l, ws.x / ws.y));
    var pic = c.rgb;
    pic = mix(pic, pic * vec3f(1.06, 1.0, 0.86) * 0.92 + vec3f(0.06, 0.05, 0.03), u.p0.y);
    // Soft inner shadow along the window edge.
    pic = pic * (1.0 - smoothstep(-0.012, 0.0, d) * 0.25);
    let grain = (hash12(floor(ph / max(0.002, fx_pxh()))) - 0.5) * 0.025;
    var paper = u.c0.rgb + vec3f(grain);
    paper = paper * (1.0 - (1.0 - smoothstep(0.0, 0.012, d)) * 0.12);
    return fx_put(mix(paper, pic, inside), 1.0);
}

// 35mm film strip with sprocket holes. p0.x = border, c0 = film color.
fn fx_film_strip(uv: vec2f) -> vec4f {
    let w = aspect();
    let ph = vec2f(uv.x * w, uv.y);
    let band = mix(0.08, 0.15, u.p0.x);
    let win = vec2f(w, 1.0 - band * 2.0);
    let l = vec2f(ph.x / w, (ph.y - band) / win.y);
    let pd = fx_sd_box(ph - vec2f(w * 0.5, 0.5), vec2f(w * 0.5 - band * 0.18, win.y * 0.5), band * 0.06);
    let inside = fx_fill(pd);
    let c = fx_get(fx_cover(l, win.x / win.y));
    // Sprocket holes, evenly spaced and centred in each band.
    let pitch = band * 0.8;
    let hx = ph.x - w * 0.5;
    let cx = (floor(hx / pitch) + 0.5) * pitch;
    let cy = select(1.0 - band * 0.5, band * 0.5, ph.y < 0.5);
    let hd = fx_sd_box(vec2f(hx - cx, ph.y - cy), vec2f(pitch * 0.27, band * 0.22), band * 0.06);
    let hole = fx_fill(hd);
    var film = u.c0.rgb * (0.9 + 0.1 * hash12(floor(ph * 400.0)));
    film = mix(film, vec3f(0.94, 0.93, 0.88), hole);
    let rgb = mix(film, c.rgb, inside);
    return fx_put(rgb, 1.0);
}

// Retro TV set around the picture. p0.x = curvature, c0 = cabinet color.
fn fx_tv(uv: vec2f) -> vec4f {
    let w = aspect();
    let ph = vec2f(uv.x * w, uv.y);
    let panel = select(0.0, 0.34, w > 1.3);
    let smin = vec2f(0.07, 0.08);
    let smax = vec2f(w - 0.07 - panel, 0.92);
    let ss = smax - smin;
    let sc = (smin + smax) * 0.5;
    let sd = fx_sd_box(ph - sc, ss * 0.5, 0.08);
    // Cabinet with a subtle wood grain.
    let grain = fbm(vec2f(ph.x * 3.0, ph.y * 40.0 + sin(ph.x * 5.0) * 2.0));
    var rgb = u.c0.rgb * (0.82 + 0.3 * grain);
    let edge = fx_sd_box(ph - vec2f(w * 0.5, 0.5), vec2f(w * 0.5, 0.5), 0.06);
    rgb = rgb * (1.0 - smoothstep(-0.05, 0.0, edge) * 0.45);
    // Bezel.
    let bez = fx_fill(sd - 0.03);
    rgb = mix(rgb, vec3f(0.07, 0.07, 0.075) + vec3f(0.06) * smoothstep(0.03, 0.0, abs(sd - 0.015)), bez);
    // Screen.
    var q = (ph - sc) / (ss * 0.5);
    q = q * (1.0 + u.p0.x * 0.12 * vec2f(q.y * q.y, q.x * q.x));
    let l = q * 0.5 + 0.5;
    let c = fx_get(fx_cover(l, ss.x / ss.y));
    var pic = c.rgb * (0.9 + 0.1 * fx_stripes(ph.y, 160.0, 0.0));
    pic = pic * (1.0 - smoothstep(0.5, 1.4, length(q)) * 0.55);
    let glare = smoothstep(0.35, 0.0, abs(q.x * 0.6 + q.y + 0.75)) * 0.12;
    pic = pic + vec3f(glare);
    rgb = mix(rgb, pic, fx_fill(sd));
    if panel > 0.0 {
        let px0 = w - panel * 0.5 - 0.03;
        // Two knobs.
        for (var i = 0; i < 2; i = i + 1) {
            let kc = vec2f(px0, 0.24 + f32(i) * 0.22);
            let kq = ph - kc;
            let kd = length(kq) - 0.07;
            let knob = fx_fill(kd);
            let ring = fx_fill(kd + 0.012);
            var kcol = mix(vec3f(0.1, 0.1, 0.1), vec3f(0.22, 0.21, 0.2), ring);
            kcol = kcol + vec3f(0.12) * smoothstep(0.07, 0.0, length(kq - vec2f(-0.02, -0.025))) * ring;
            let ang = 0.6 + f32(i) * 1.9;
            let tick = fx_fill(fx_sd_segment(kq, vec2f(0.0), vec2f(sin(ang), -cos(ang)) * 0.05) - 0.006);
            kcol = mix(kcol, vec3f(0.85, 0.82, 0.75), tick);
            rgb = mix(rgb * (1.0 - fx_fill(kd - 0.012) * 0.3), kcol, knob);
        }
        // Speaker grille.
        let gq = ph - vec2f(px0, 0.74);
        let gd = fx_sd_box(gq, vec2f(0.1, 0.14), 0.02);
        let slots = smoothstep(0.35, 0.65, fx_stripes(gq.y, 28.0, 0.0));
        rgb = mix(rgb, mix(vec3f(0.12, 0.11, 0.1), vec3f(0.32, 0.29, 0.25), slots), fx_fill(gd));
    }
    return fx_put(rgb, 1.0);
}

// The picture shrunk onto a blurred copy of itself.
// p0 = (size, blur, corner radius).
fn fx_blur_border(uv: vec2f) -> vec4f {
    let s = mix(0.55, 0.95, u.p0.x);
    let p = centered(uv);
    let hs = vec2f(aspect() * 0.5, 0.5) * s;
    let r = u.p0.z * 0.08;
    let d = fx_sd_box(p, hs, r);
    let bg_uv = fx_uv(p * 0.9);
    let bg = unpremul(fx_disc(bg_uv, mix(0.01, 0.06, u.p0.y), 40));
    var rgb = bg.rgb * 0.75;
    let shadow = 1.0 - smoothstep(-0.01, 0.05, d - 0.012);
    rgb = rgb * (1.0 - shadow * 0.45);
    let pic = fx_get(fx_uv(p / s));
    rgb = mix(rgb, pic.rgb, fx_fill(d));
    return fx_put(rgb, mix(bg.a, pic.a, fx_fill(d)));
}

// ---- Split screen --------------------------------------------------------

// The picture repeated in `cols` x `rows` panels, each a centre crop.
// p0.x = gap, c0 = gap color.
fn fx_split(uv: vec2f, cols: f32, rows: f32) -> vec4f {
    let gap = u.p0.x * 0.02;
    let w = aspect();
    let ph = vec2f(uv.x * w, uv.y);
    let cell = vec2f(w / cols, 1.0 / rows);
    let g = min(floor(ph / cell), vec2f(cols - 1.0, rows - 1.0));
    let cc = (g + 0.5) * cell;
    let hs = cell * 0.5 - vec2f(gap * 0.5);
    let d = fx_sd_box(ph - cc, hs, 0.0);
    let l = (ph - cc) / (hs * 2.0) + 0.5;
    let c = sa(fx_cover(l, hs.x / hs.y));
    return mix(premul(vec4f(u.c0.rgb, 1.0)), c, fx_fill(d));
}

// Effects are dispatched in three groups: one `switch` with every case in
// it is slow to compile and was seen to misbehave in software Vulkan.
fn fx_dispatch_a(uv: vec2<f32>) -> vec4<f32> {
    switch u.kind {
        case 1u: { return fx_gaussian(uv, u.p0.x); }
        case 2u: { return fx_radial_blur(uv); }
        case 3u: { return fx_zoom_blur(uv); }
        case 4u: { return fx_directional_blur(uv); }
        case 5u: { return fx_mosaic(uv); }
        case 6u: { return fx_invert(uv); }
        case 7u: { return fx_halftone(uv); }
        case 8u: { return fx_mirror(uv); }
        case 9u: { return fx_kaleidoscope(uv); }
        case 10u: { return fx_rgb_split(uv); }
        case 11u: { return fx_glitch(uv); }
        case 12u: { return fx_vhs(uv); }
        case 13u: { return fx_old_film(uv); }
        case 14u: { return fx_grain(uv); }
        case 15u: { return fx_wave(uv); }
        case 16u: { return fx_chroma_key(uv); }
        case 17u: { return fx_light_leak(uv); }
        case 18u: { return fx_snow(uv); }
        case 19u: { return fx_rounded_frame(uv); }
        case 20u: { return fx_letterbox(uv); }
        case 21u: { return fx_grid(uv, 2.0); }
        case 22u: { return fx_grid(uv, 3.0); }
        case 23u: { return fx_lens(uv); }
        case 24u: { return fx_fisheye(uv); }
        case 25u: { return fx_bulge(uv, u.p0.x); }
        case 26u: { return fx_bulge(uv, -u.p0.x); }
        case 27u: { return fx_twirl(uv); }
        case 28u: { return fx_ripple(uv); }
        case 29u: { return fx_heat(uv); }
        case 30u: { return fx_tiles(uv); }
        default: { return sa(uv); }
    }
}

fn fx_dispatch_b(uv: vec2<f32>) -> vec4<f32> {
    switch u.kind {
        case 31u: { return fx_tilt_shift(uv); }
        case 32u: { return fx_focus_blur(uv); }
        case 33u: { return fx_lens_blur(uv); }
        case 34u: { return fx_frosted(uv); }
        case 35u: { return fx_posterize(uv); }
        case 36u: { return fx_color_splash(uv); }
        case 37u: { return fx_duotone(uv); }
        case 38u: { return fx_sketch(uv); }
        case 39u: { return fx_comic(uv); }
        case 40u: { return fx_emboss(uv); }
        case 41u: { return fx_thermal(uv); }
        case 42u: { return fx_night_vision(uv); }
        case 43u: { return fx_crt(uv); }
        case 44u: { return fx_led(uv); }
        case 45u: { return fx_pixel_art(uv); }
        case 46u: { return fx_chromatic(uv); }
        case 47u: { return fx_slice(uv); }
        case 48u: { return fx_static(uv); }
        case 49u: { return fx_block(uv); }
        case 50u: { return fx_bloom(uv); }
        case 51u: { return fx_soft_glow(uv); }
        case 52u: { return fx_light_rays(uv); }
        case 53u: { return fx_lens_flare(uv); }
        case 54u: { return fx_neon_edges(uv); }
        case 55u: { return fx_film_burn(uv); }
        case 56u: { return fx_rain(uv); }
        case 57u: { return fx_bubbles(uv); }
        case 58u: { return fx_confetti(uv); }
        case 59u: { return fx_fireflies(uv); }
        case 60u: { return fx_hearts(uv); }
        default: { return sa(uv); }
    }
}

// Shockwave: a ring races out from a point, bending the picture around it
// with a colour fringe and a white-hot edge, then the next one follows.
// p0 = (strength, speed, centre x, centre y).
fn fx_shockwave(uv: vec2<f32>) -> vec4<f32> {
    let asp = aspect();
    let p = (uv - u.p0.zw) * vec2<f32>(asp, 1.0);
    let dist = length(p);
    let dir = p / max(dist, 1e-4);
    let period = mix(2.4, 0.9, u.p0.y);
    let ph = fract(u.time / period);
    let radius = ph * 1.4;
    let width = mix(0.05, 0.12, u.p0.x) * (0.5 + 0.5 * ph);
    let d = dist - radius;
    let sweep = d / width;
    // A pinch just inside the ring and a bulge just outside it.
    let push = -sweep * exp(-sweep * sweep);
    let fade = (1.0 - ph) * smoothstep(0.0, 0.05, ph);
    let off = dir * push * (u.p0.x * 0.14 * fade) * vec2<f32>(1.0 / asp, 1.0);
    let fringe = off * 0.35;
    let a = sa(uv + off).a;
    let rgb = vec3<f32>(sa(uv + off + fringe).r, sa(uv + off).g, sa(uv + off - fringe).b);
    let hot = exp(-pow((d + 0.15 * width) / (0.18 * width), 2.0)) * fade * u.p0.x;
    return vec4<f32>(min(rgb + vec3<f32>(hot * 0.7) * a, vec3<f32>(a)), a);
}

// Colour flicker family: ten styles from one shader, picked by p0.z.
// p0 = (amount, speed, style, variation).
// Styles: 0 negative, 1 colour flash, 2 RGB stroke, 3 neon swing,
// 4 hallucination, 5 exposure jitter, 6 hue cycle, 7 black-and-white
// strobe, 8 colour slice, 9 pulse flash.
fn fx_flicker(uv: vec2f) -> vec4f {
    let amount = u.p0.x;
    let tt = u.time * mix(2.0, 14.0, u.p0.y);
    let k = floor(tt);
    let f = fract(tt);
    let seed = u.p0.w * 31.0;
    let h = hash11(k + seed);
    let style = u32(max(u.p0.z, 0.0) + 0.5);
    let c = fx_get(uv);
    var rgb = c.rgb;
    switch style {
        case 0u: {
            rgb = mix(rgb, vec3f(1.0) - rgb, step(0.5, h) * amount);
        }
        case 1u: {
            let tint = hsv2rgb(vec3f(hash11(k * 1.7 + seed), 0.75, 1.0));
            let flash = pow(1.0 - f, 2.0) * step(0.35, h) * amount;
            rgb = fx_screen(rgb, tint * flash);
        }
        case 2u: {
            let off = (hash11(k * 2.3 + seed) - 0.5) * 0.04 * amount * step(0.4, h);
            let o = vec2f(off, off * 0.4);
            rgb = vec3f(fx_get(uv + o).r, rgb.g, fx_get(uv - o).b);
        }
        case 3u: {
            let hsv = rgb2hsv(rgb);
            let swing = sin(u.time * (1.0 + u.p0.y * 6.0)) * 0.5 * amount;
            rgb = hsv2rgb(vec3f(fract(hsv.x + swing), clamp(hsv.y * (1.0 + amount * 0.6), 0.0, 1.0), hsv.z));
        }
        case 4u: {
            let o = vec2f(0.012, 0.006) * amount * (0.5 + 0.5 * sin(u.time * 3.0));
            let ha = rgb2hsv(fx_get(uv + o).rgb);
            let hb = rgb2hsv(fx_get(uv - o).rgb);
            let ga = hsv2rgb(vec3f(fract(ha.x + 0.33), ha.y, ha.z));
            let gb = hsv2rgb(vec3f(fract(hb.x + 0.66), hb.y, hb.z));
            rgb = mix(rgb, max(rgb, max(ga, gb) * 0.9), 0.55 * amount + 0.1);
        }
        case 5u: {
            rgb = rgb * (1.0 + (h - 0.5) * amount * 1.2);
        }
        case 6u: {
            let hsv = rgb2hsv(rgb);
            let cyc = hsv2rgb(vec3f(fract(hsv.x + u.time * (0.1 + u.p0.y * 0.8)), clamp(hsv.y * (1.0 + 0.4 * amount), 0.0, 1.0), hsv.z));
            rgb = mix(rgb, cyc, clamp(amount * 1.4, 0.0, 1.0));
        }
        case 7u: {
            let on = step(0.5, fract(tt * 0.5)) * amount;
            rgb = mix(rgb, vec3f(luma(rgb) * 1.2), on);
        }
        case 8u: {
            let band = floor(uv.y * 9.0);
            let shift = (hash11(band * 7.0 + k + seed) - 0.5) * step(0.5, hash11(k * 3.1 + band)) * amount;
            let hsv = rgb2hsv(rgb);
            rgb = hsv2rgb(vec3f(fract(hsv.x + shift), clamp(hsv.y * (1.0 + 0.5 * abs(shift)), 0.0, 1.0), hsv.z));
        }
        default: {
            rgb = rgb + vec3f(pow(max(1.0 - f * 2.5, 0.0), 2.0) * amount * 0.6);
        }
    }
    return fx_put(rgb, c.a);
}

fn fx_dispatch_c(uv: vec2<f32>) -> vec4<f32> {
    switch u.kind {
        case 61u: { return fx_sparkles(uv); }
        case 62u: { return fx_vignette(uv); }
        case 63u: { return fx_bokeh_lights(uv); }
        case 64u: { return fx_viewfinder(uv); }
        case 65u: { return fx_polaroid(uv); }
        case 66u: { return fx_film_strip(uv); }
        case 67u: { return fx_tv(uv); }
        case 68u: { return fx_blur_border(uv); }
        case 69u: { return fx_split(uv, 2.0, 1.0); }
        case 70u: { return fx_split(uv, 1.0, 2.0); }
        case 71u: { return fx_split(uv, 3.0, 1.0); }
        case 72u: { return fx_blur_area(uv); }
        case 73u: { return fx_pixelate_area(uv); }
        case 74u: { return fx_halation(uv); }
        case 75u: { return fx_pro_mist(uv); }
        case 76u: { return fx_dream_glow(uv); }
        case 77u: { return fx_anamorphic(uv); }
        case 78u: { return fx_light_sweep(uv); }
        case 79u: { return fx_spotlight(uv); }
        case 80u: { return fx_beat_flash(uv); }
        case 81u: { return fx_rgb_pulse(uv); }
        case 82u: { return fx_speed_lines(uv); }
        case 83u: { return fx_echo(uv); }
        case 84u: { return fx_camcorder(uv); }
        case 85u: { return fx_fluid_colour(uv); }
        case 86u: { return fx_prism(uv); }
        case 87u: { return fx_liquid(uv); }
        case 88u: { return fx_dust(uv); }
        case 89u: { return fx_embers(uv); }
        case 90u: { return fx_mist(uv); }
        case 91u: { return fx_shockwave(uv); }
        case 92u: { return fx_flicker(uv); }
        default: { return sa(uv); }
    }
}

// ---- Glass, sparkle, grunge, frames, celebration, 3D layouts -------------

struct Vor {
    d1: f32,
    d2: f32,
    site: vec2f,
    id: vec2f,
}

// Nearest and second-nearest feature points of a jittered grid. `wob`
// (0 or 1) lets the points drift over time.
fn fx_voronoi(p: vec2f, wob: f32) -> Vor {
    let g = floor(p);
    var best = Vor(8.0, 8.0, vec2f(0.0), vec2f(0.0));
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let cell = g + vec2f(f32(i), f32(j));
            let h = hash22(cell);
            var site = cell + 0.15 + 0.7 * h;
            site = site + wob * 0.18 * vec2f(sin(u.time * 0.9 + h.x * TAU), cos(u.time * 1.1 + h.y * TAU));
            let d = length(site - p);
            if d < best.d1 {
                best.d2 = best.d1;
                best.d1 = d;
                best.site = site;
                best.id = cell;
            } else if d < best.d2 {
                best.d2 = d;
            }
        }
    }
    return best;
}

// Glass family: 0 shatter (tiles pop out from an impact), 1 cracked glass,
// 2 prism shards. p0 = (amount, speed, style, size).
fn fx_shatter(uv: vec2f) -> vec4f {
    let amount = u.p0.x;
    let mode = u32(max(u.p0.z, 0.0) + 0.5);
    let scale = mix(14.0, 4.0, u.p0.w);
    let p = centered(uv);
    let period = mix(4.5, 1.8, u.p0.y);
    let cyc = floor(u.time / period);
    let ph = fract(u.time / period);
    let impact = (vec2f(hash11(cyc * 3.1), hash11(cyc * 5.3)) - 0.5) * vec2f(aspect() * 0.7, 0.6);
    var wob = 0.0;
    if mode == 2u {
        wob = 1.0;
    }
    let v = fx_voronoi(p * scale, wob);
    let h = hash22(v.id + 5.0);
    let site_h = v.site / scale;
    let edge = v.d2 - v.d1;
    let line = 1.0 - smoothstep(0.0, 0.04, edge);
    if mode == 0u {
        let reach = length(site_h - impact);
        let crack_r = clamp(ph / 0.22, 0.0, 1.0) * 1.3;
        let delay = reach * 0.3;
        let k = clamp((ph - 0.26 - delay) / 0.3, 0.0, 1.0);
        let back = smoothstep(0.88, 1.0, ph);
        let kk = k * (1.0 - back);
        let s = mix(1.0, 0.5, kk * (0.5 + 0.5 * amount));
        let ang = (h.x - 0.5) * 2.0 * kk * (0.3 + amount);
        let src = site_h + (rot2(ang) * (p - site_h)) / s + (h - 0.5) * 0.06 * kk * amount;
        let vs = fx_voronoi(src * scale, 0.0);
        let inside = select(0.0, 1.0, all(vs.id == v.id));
        let c = fx_get(fx_uv(src));
        var rgb = c.rgb * (1.0 - 0.35 * kk);
        let hi = (1.0 - smoothstep(0.0, 0.06, vs.d2 - vs.d1)) * kk;
        rgb = rgb + vec3f(hi * 0.6);
        let crack = line * smoothstep(crack_r, crack_r - 0.2, reach) * (1.0 - kk);
        rgb = mix(rgb, vec3f(0.92, 0.97, 1.0), crack * 0.8);
        let a = c.a * mix(1.0, inside, smoothstep(0.0, 0.05, kk)) * (1.0 - kk * 0.35);
        return fx_put(rgb, a);
    }
    if mode == 1u {
        let crack_r = mix(0.2, 1.4, smoothstep(0.0, 0.4, ph)) * (0.5 + 0.7 * amount);
        let reach = length(site_h - impact);
        let near = smoothstep(crack_r, crack_r * 0.6, reach);
        let keep = step(h.x, 0.5 + amount * 0.45);
        let off = (h - 0.5) * 0.014 * amount * near;
        let c = fx_get(uv + off);
        var rgb = c.rgb * (1.0 + (h.y - 0.5) * 0.14 * amount * near);
        let crack = line * keep * near;
        rgb = mix(rgb, vec3f(0.04, 0.05, 0.07), crack * 0.8);
        let spark = (1.0 - smoothstep(0.0, 0.024, edge)) * keep * near;
        rgb = rgb + vec3f(spark * 0.85);
        return fx_put(rgb, c.a);
    }
    let off = (h - 0.5) * 0.03 * amount;
    let c = fx_get(uv + off);
    let tint = hsv2rgb(vec3f(fract(h.x + u.time * 0.04), 0.55, 1.0));
    let shade = 0.82 + 0.3 * sin(u.time * 0.8 + h.y * TAU);
    var rgb = c.rgb * shade;
    rgb = mix(rgb, rgb * tint * 1.5, 0.28 * amount);
    rgb = rgb + vec3f(line * 0.4 * amount);
    return fx_put(rgb, c.a);
}

// Glitter: tiny grains twinkling with little four-point glints, strongest
// where a slow band of light passes. p0 = (amount, speed, size, spread),
// c0 = tint.
fn fx_glitter(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let t = u.time * mix(0.4, 3.0, u.p0.y);
    let sc = mix(60.0, 18.0, u.p0.z);
    let q = p * sc;
    let g = floor(q);
    var light = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let cell = g + vec2f(f32(i), f32(j));
            let h = hash22(cell);
            if h.x > u.p0.x * 0.6 {
                continue;
            }
            let h2 = hash22(cell * 1.7 + 9.1);
            let pos = cell + 0.15 + 0.7 * h2;
            let tw = pow(max(sin(t * (1.0 + 2.0 * h.y) + h2.x * TAU), 0.0), 6.0);
            let a = q - pos;
            let arm = exp(-abs(a.x) * 2.0) * exp(-abs(a.y) * 16.0) + exp(-abs(a.y) * 2.0) * exp(-abs(a.x) * 16.0);
            let core = exp(-dot(a, a) * 18.0);
            light = light + (core + 0.7 * arm) * tw * (0.5 + h.y);
        }
    }
    let wide = mix(2.0, 6.0, u.p0.w);
    let band = 0.3 + 0.7 * smoothstep(0.1, 1.0, 0.5 + 0.5 * sin((p.x * 0.8 + p.y * 0.6) * wide - t * 0.35));
    light = light * band;
    let tint = mix(vec3f(1.0), u.c0.rgb, 0.7);
    return fx_put(c.rgb + tint * light * 1.9 + vec3f(max(light - 0.8, 0.0) * 0.5), c.a);
}

// Starburst: big glints that pop up at random spots, each with a number
// of long rays. p0 = (amount, speed, size, points), c0 = tint.
fn fx_starburst(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let t = u.time * mix(0.2, 1.2, u.p0.y);
    let sc = 3.0;
    let q = p * sc + vec2f(1.7, 0.4);
    let g = floor(q);
    let n = 2.0 * (2.0 + floor(u.p0.w * 3.99));
    var light = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let cell = g + vec2f(f32(i), f32(j));
            let h = hash22(cell);
            if h.x > u.p0.x {
                continue;
            }
            let h2 = hash22(cell * 1.9 + 4.2);
            let pos = cell + 0.2 + 0.6 * h2;
            let life = fract(t * (0.35 + 0.4 * h.y) + h2.x);
            let pop = pow(sin(life * PI), 2.0);
            let a = (q - pos) / sc;
            let r = length(a);
            let ang = atan2(a.y, a.x) + h.y * 3.0 + t * 0.25;
            let size = mix(0.05, 0.16, u.p0.z) * (0.6 + h2.y * 0.6);
            let rays = pow(abs(cos(ang * n * 0.5)), 30.0) * exp(-r / (size * 0.9));
            let core = exp(-pow(r / (size * 0.16), 2.0));
            light = light + (rays * 0.9 + core * 1.4) * pop;
        }
    }
    let tint = mix(vec3f(1.0), u.c0.rgb, 0.55);
    return fx_put(c.rgb + tint * light + vec3f(max(light - 1.0, 0.0) * 0.4), c.a);
}

// Grunge family: 0 dust and scratches, 1 grime, 2 crumpled paper.
// p0 = (amount, speed, style, size).
fn fx_grunge(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let mode = u32(max(u.p0.z, 0.0) + 0.5);
    let amount = u.p0.x;
    var rgb = c.rgb;
    if mode == 0u {
        let k = floor(u.time * mix(4.0, 18.0, u.p0.y));
        let sc = mix(70.0, 24.0, u.p0.w);
        let q = p * sc;
        let g = floor(q);
        var white = 0.0;
        var dark = 0.0;
        for (var j = -1; j <= 1; j = j + 1) {
            for (var i = -1; i <= 1; i = i + 1) {
                let cell = g + vec2f(f32(i), f32(j));
                let h = hash22(cell + k * 13.7);
                if h.x > amount * 0.1 {
                    continue;
                }
                let h2 = hash22(cell * 1.3 + k * 3.1);
                let pos = cell + 0.2 + 0.6 * h2;
                let d = length(q - pos) * (0.6 + h.y * 2.0);
                let spot = exp(-d * d * 12.0);
                if h.y > 0.45 {
                    white = white + spot;
                } else {
                    dark = dark + spot;
                }
            }
        }
        // A thin scratch drifting down the frame now and then.
        let sx = (hash11(k * 1.37) - 0.5) * aspect();
        let wiggle = (fbm(vec2f(p.y * 6.0, k)) - 0.5) * 0.01;
        let on = step(0.8 - amount * 0.35, hash11(k * 7.7));
        let scratch = on * exp(-pow((p.x - sx - wiggle) / (fx_pxh() * 1.2), 2.0)) * smoothstep(0.2, 0.5, fbm(vec2f(p.y * 2.0, k * 0.3)));
        rgb = rgb + vec3f(white * 0.8 + scratch * 0.7);
        rgb = rgb * (1.0 - dark * 0.55);
    } else if mode == 1u {
        let sc = mix(5.0, 1.6, u.p0.w);
        let n = fbm(p * sc + 3.0) * 0.7 + fbm(p * sc * 3.0) * 0.3;
        let blot = smoothstep(0.42, 0.7, n);
        let rim = smoothstep(0.35, 0.95, length(p * vec2f(0.8, 1.1)));
        let dirt = clamp(blot * 0.8 + rim * 0.5, 0.0, 1.0) * amount;
        rgb = rgb * (1.0 - dirt * 0.65) * mix(vec3f(1.0), vec3f(1.05, 0.95, 0.82), dirt);
    } else {
        let sc = mix(7.0, 3.0, u.p0.w);
        let v = fx_voronoi(p * sc, 0.0);
        let h = hash22(v.id + 2.0);
        let line = 1.0 - smoothstep(0.0, 0.07, v.d2 - v.d1);
        let fine = fbm(p * 40.0) - 0.5;
        let shade = (h.x - 0.5) * 0.35 + line * 0.18 * (h.y - 0.3) + fine * 0.18;
        rgb = rgb * (1.0 + shade * amount * 1.6) + vec3f(line * 0.05 * amount);
    }
    return fx_put(rgb, c.a);
}

// Neon frame glowing round the picture. p0 = (glow, speed, style, inset),
// c0, c1 = colours. Styles: 0 steady, 1 chase, 2 pulse, 3 two colours.
fn fx_neon_border(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let mode = u32(max(u.p0.z, 0.0) + 0.5);
    let inset = mix(0.015, 0.09, u.p0.w);
    let half = vec2f(aspect() * 0.5 - inset, 0.5 - inset);
    let r = min(half.y * 0.3, 0.08);
    let d = abs(fx_sd_box(p, half, r));
    let t = u.time * mix(0.25, 1.6, u.p0.y);
    let ang = atan2(p.y, p.x) / TAU + 0.5;
    var col = u.c0.rgb;
    var inten = 1.0;
    if mode == 1u {
        let s = fract(ang - t);
        inten = 0.12 + pow(1.0 - s, 4.0) * 1.6;
        col = mix(u.c0.rgb, u.c1.rgb, 1.0 - s);
    } else if mode == 2u {
        inten = 0.55 + 0.45 * sin(t * 4.0);
    } else if mode == 3u {
        col = mix(u.c0.rgb, u.c1.rgb, 0.5 + 0.5 * sin(TAU * (ang * 2.0 - t)));
    }
    let core = exp(-pow(d / 0.0035, 2.0));
    let glow = exp(-d / (0.012 + 0.04 * u.p0.x)) * 0.8 + exp(-d / 0.12) * 0.15;
    var rgb = c.rgb * (1.0 - 0.12 * glow * inten);
    rgb = rgb + col * glow * inten * (0.5 + u.p0.x) + (col * 0.35 + vec3f(0.65)) * core * inten;
    return fx_put(rgb, c.a);
}

// Fireworks: rockets climb and burst into fading sparks with trails.
// p0 = (amount, speed, size, trails), c0 unused: colours vary per burst.
fn fx_fireworks(uv: vec2f) -> vec4f {
    let c = fx_get(uv);
    let p = centered(uv);
    let t = u.time * mix(0.5, 1.6, u.p0.y);
    var light = vec3f(0.0);
    var dim = 0.0;
    for (var b = 0; b < 3; b = b + 1) {
        let fb = f32(b);
        let period = 2.6 + fb * 0.45;
        let tb = t / period + fb * 0.37;
        let cyc = floor(tb);
        let fr = fract(tb);
        let cx = (hash11(cyc * 3.1 + fb * 7.7) - 0.5) * aspect() * 0.8;
        let cy = (hash11(cyc * 5.3 + fb * 2.1) - 0.15) * 0.45;
        let hue = hash11(cyc * 1.9 + fb * 4.4);
        if fr < 0.18 {
            let k = fr / 0.18;
            let pos = vec2f(cx, mix(-0.56, cy, 1.0 - (1.0 - k) * (1.0 - k)));
            let d = length(p - pos);
            light = light + vec3f(1.0, 0.8, 0.5) * (exp(-d * d / 0.0002) * 1.4 + exp(-d * d / 0.003) * 0.15);
            dim = max(dim, 0.3);
        } else {
            let age = (fr - 0.18) / 0.82;
            let fade = pow(1.0 - age, 1.5);
            dim = max(dim, fade * 0.8);
            for (var i = 0; i < 28; i = i + 1) {
                let fi = f32(i);
                let hh = hash11(fi * 3.3 + cyc * 1.7 + fb * 9.1);
                if hh > u.p0.x * 1.15 {
                    continue;
                }
                let ang = (fi + 0.5 * hh) / 28.0 * TAU;
                let dir = vec2f(cos(ang), sin(ang));
                let spd = (0.28 + 0.2 * hash11(fi * 5.1 + cyc)) * mix(0.7, 1.5, u.p0.z);
                let colr = hsv2rgb(vec3f(fract(hue + 0.1 * hh), 0.65, 1.0));
                for (var k = 0; k < 3; k = k + 1) {
                    let a = max(age - f32(k) * 0.03, 0.0);
                    let pos = vec2f(cx, cy) + dir * spd * (1.0 - exp(-a * 3.5)) + vec2f(0.0, -0.12 * a * a);
                    let d = p - pos;
                    let rad = 0.008 * (1.0 + 0.6 * f32(k));
                    let w = select(0.4 + 0.6 * u.p0.w, 1.0, k == 0);
                    let spark = 0.7 + 0.3 * sin(age * 40.0 + fi);
                    light = light + colr * (exp(-dot(d, d) / (rad * rad)) * 1.5 + exp(-dot(d, d) / (rad * rad * 12.0)) * 0.1) * fade * w * spark;
                }
            }
        }
    }
    return fx_put(c.rgb * (1.0 - 0.45 * dim) + light, c.a);
}

// One tilted panel showing the picture: returns colour and coverage.
// `centre` and `size` in height units, `yaw` in radians.
fn fx_panel(p: vec2f, centre: vec2f, size: f32, yaw: f32) -> vec4f {
    let dist = 2.0;
    let q = (p - centre) / size;
    let s = sin(yaw);
    let cs = cos(yaw);
    let den = dist * cs - q.x * s;
    if den <= 0.05 {
        return vec4f(0.0);
    }
    let x = q.x * dist / den;
    let y = q.y * (dist + x * s) / dist;
    let half = vec2f(aspect() * 0.5, 0.5);
    let l = vec2f(x, y);
    let inside = fx_fill(fx_sd_box(l, half, 0.0) * size);
    if inside <= 0.0 {
        return vec4f(0.0);
    }
    let col = sa(vec2f(x / aspect() + 0.5, y + 0.5));
    let rim = smoothstep(0.012, 0.0, -fx_sd_box(l, half, 0.0));
    return vec4f(unpremul(col).rgb + vec3f(rim * 0.35), inside);
}

// 3D layouts: the picture shown on tilted panels over a dimmed copy of
// itself. p0 = (tilt, sway, style, gap). Styles: 0 fan of three, 1 stairs,
// 2 floating card.
fn fx_panels(uv: vec2f) -> vec4f {
    let p = centered(uv);
    let mode = u32(max(u.p0.z, 0.0) + 0.5);
    let sway = sin(u.time * mix(0.3, 1.6, u.p0.y)) * 0.12;
    let bg = blur_a(uv, 0.04).rgb * 0.32;
    var rgb = bg;
    let tilt = mix(0.25, 0.75, u.p0.x);
    let gap = mix(0.5, 0.78, u.p0.w) * aspect() * 0.5;
    if mode == 0u {
        let l = fx_panel(p, vec2f(-gap, 0.0), 0.5, tilt + sway);
        let r = fx_panel(p, vec2f(gap, 0.0), 0.5, -tilt - sway);
        let m = fx_panel(p, vec2f(0.0, 0.0), 0.68, sway * 0.4);
        rgb = mix(rgb, l.rgb * 0.8, l.a);
        rgb = mix(rgb, r.rgb * 0.8, r.a);
        rgb = mix(rgb, m.rgb, m.a);
    } else if mode == 1u {
        for (var i = 0; i < 3; i = i + 1) {
            let fi = f32(2 - i);
            let o = vec2f((fi - 1.0) * gap * 0.5, (fi - 1.0) * 0.16);
            let pn = fx_panel(p, o, 0.42 + 0.06 * (2.0 - fi), -tilt * 0.6 + sway);
            rgb = mix(rgb, pn.rgb * (0.7 + 0.15 * (2.0 - fi)), pn.a);
        }
    } else {
        let pn = fx_panel(p, vec2f((tilt - 0.15) * 0.3, 0.02), 0.72, (tilt - 0.15) * 0.8 + sway);
        let shadow = exp(-pow(length((p - vec2f(0.0, -0.4)) * vec2f(0.8, 3.0)), 2.0) * 3.0) * 0.4;
        rgb = rgb * (1.0 - shadow);
        rgb = mix(rgb, pn.rgb, pn.a);
    }
    return vec4f(clamp(rgb, vec3f(0.0), vec3f(1.0)), 1.0);
}

// Beat effects: the picture reacts to the tempo, so cuts to music feel
// tight. p0 = (beats per minute, amount, decay, style). Styles: 0 zoom pump,
// 1 shake kick, 2 colour pop, 3 blur thump, 4 strobe on alternate beats,
// 5 hue kick, 6 squash and bounce, 7 vignette pulse.
fn fx_beat_fx(uv: vec2f) -> vec4f {
    let bpm = u.p0.x;
    let amt = u.p0.y;
    let k = pow(1.0 - fx_beat(bpm), mix(5.0, 1.5, u.p0.z));
    let idx = floor(max(u.time, 0.0) * bpm / 60.0);
    let style = u32(max(u.p0.w, 0.0) + 0.5);
    let p = centered(uv);
    switch style {
        case 0u: {
            return sa(fx_uv(p / (1.0 + 0.14 * amt * k)));
        }
        case 1u: {
            let a = idx * 2.4;
            let o = vec2f(cos(a), sin(a * 1.3)) * 0.035 * amt * k;
            return sa(fx_uv(p + o));
        }
        case 2u: {
            let c = fx_get(uv);
            let hsv = rgb2hsv(c.rgb);
            return fx_put(hsv2rgb(vec3f(hsv.x, clamp(hsv.y * (1.0 + 1.1 * amt * k), 0.0, 1.0), hsv.z * (1.0 + 0.18 * amt * k))), c.a);
        }
        case 3u: {
            var acc = vec4f(0.0);
            for (var i = 0; i < 8; i = i + 1) {
                let f = f32(i) / 7.0;
                acc = acc + sa(fx_uv(p * (1.0 - 0.07 * amt * k * f)));
            }
            return acc / 8.0;
        }
        case 4u: {
            let c = fx_get(uv);
            var g = 1.0 - 0.45 * amt * k;
            if (u32(idx) % 2u) == 0u {
                g = 1.0 + 0.5 * amt * k;
            }
            return fx_put(c.rgb * g, c.a);
        }
        case 5u: {
            let c = fx_get(uv);
            let hsv = rgb2hsv(c.rgb);
            let shift = (0.12 + 0.05 * f32(u32(idx) % 3u)) * amt * k;
            return fx_put(hsv2rgb(vec3f(fract(hsv.x + shift), hsv.y, hsv.z)), c.a);
        }
        case 6u: {
            let sy = 1.0 - 0.1 * amt * k;
            let sx = 1.0 + 0.05 * amt * k;
            return sa(fx_uv(vec2f(p.x / sx, p.y / sy)));
        }
        default: {
            let c = fx_get(uv);
            let v = smoothstep(0.25, 0.95, length(p * vec2f(0.8, 1.1)));
            return fx_put(c.rgb * (1.0 - 0.85 * amt * k * v) + vec3f(0.08 * amt * k), c.a);
        }
    }
}
// Sound-reactive effects: the picture moves with the loudness of the clip's
// own sound (u.m0 = everything, bass, mid, treble, each 0..1).
// p0 = (sensitivity, amount, band 0..3, style). Styles: 0 zoom, 1 shake,
// 2 colour pop, 3 flash, 4 colour split, 5 blur, 6 glow, 7 vignette,
// 8 hue shift, 9 tear.
fn fx_react(uv: vec2f) -> vec4f {
    let band = u32(clamp(u.p0.z + 0.5, 0.0, 3.0));
    var lv = u.m0.x;
    if band == 1u {
        lv = u.m0.y;
    } else if band == 2u {
        lv = u.m0.z;
    } else if band == 3u {
        lv = u.m0.w;
    }
    let k = clamp(lv * mix(0.6, 2.2, u.p0.x), 0.0, 1.0);
    let amt = u.p0.y;
    let style = u32(max(u.p0.w, 0.0) + 0.5);
    let p = centered(uv);
    switch style {
        case 0u: {
            return sa(fx_uv(p / (1.0 + 0.16 * amt * k)));
        }
        case 1u: {
            let a = floor(u.time * 24.0) * 2.4;
            return sa(fx_uv(p + vec2f(cos(a), sin(a * 1.3)) * 0.04 * amt * k));
        }
        case 2u: {
            let c = fx_get(uv);
            let hsv = rgb2hsv(c.rgb);
            return fx_put(hsv2rgb(vec3f(hsv.x, clamp(hsv.y * (1.0 + 1.2 * amt * k), 0.0, 1.0), hsv.z * (1.0 + 0.2 * amt * k))), c.a);
        }
        case 3u: {
            let c = fx_get(uv);
            return fx_put(c.rgb * (1.0 + 0.6 * amt * k) + vec3f(0.18 * amt * k), c.a);
        }
        case 4u: {
            let o = p * 0.05 * amt * k;
            let a = sa(fx_uv(p - o));
            let g = sa(uv);
            let b = sa(fx_uv(p + o));
            return vec4f(a.r, g.g, b.b, g.a);
        }
        case 5u: {
            var acc = vec4f(0.0);
            for (var i = 0; i < 8; i = i + 1) {
                let f = f32(i) / 7.0;
                acc = acc + sa(fx_uv(p * (1.0 - 0.08 * amt * k * f)));
            }
            return acc / 8.0;
        }
        case 6u: {
            let c = fx_get(uv);
            var glow = vec3f(0.0);
            for (var i = 0; i < 8; i = i + 1) {
                let ang = f32(i) * 0.785398;
                glow = glow + fx_get(uv + vec2f(cos(ang) / aspect(), sin(ang)) * 0.02 * (0.5 + amt)).rgb;
            }
            return fx_put(c.rgb + (glow / 8.0) * 0.8 * amt * k, c.a);
        }
        case 7u: {
            let c = fx_get(uv);
            let v = smoothstep(0.2, 0.95, length(p * vec2f(0.8, 1.1)));
            return fx_put(c.rgb * (1.0 - 0.9 * amt * k * v), c.a);
        }
        case 8u: {
            let c = fx_get(uv);
            let hsv = rgb2hsv(c.rgb);
            return fx_put(hsv2rgb(vec3f(fract(hsv.x + 0.25 * amt * k), hsv.y, hsv.z)), c.a);
        }
        default: {
            let row = floor(uv.y * 24.0);
            let shift = (hash11(row * 3.7 + floor(u.time * 20.0)) - 0.5) * 0.12 * amt * k;
            return sa(vec2f(uv.x + shift, uv.y));
        }
    }
}
fn fx_dispatch_d(uv: vec2f) -> vec4f {
    switch u.kind {
        case 93u: { return fx_shatter(uv); }
        case 94u: { return fx_glitter(uv); }
        case 95u: { return fx_starburst(uv); }
        case 96u: { return fx_grunge(uv); }
        case 97u: { return fx_neon_border(uv); }
        case 98u: { return fx_fireworks(uv); }
        case 99u: { return fx_panels(uv); }
        case 100u: { return fx_beat_fx(uv); }
        case 101u: { return fx_react(uv); }
        default: { return sa(uv); }
    }
}
@fragment
fn fs_effect(in: VOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    if u.kind <= 30u {
        return fx_dispatch_a(uv);
    }
    if u.kind <= 60u {
        return fx_dispatch_b(uv);
    }
    if u.kind <= 92u {
        return fx_dispatch_c(uv);
    }
    return fx_dispatch_d(uv);
}
