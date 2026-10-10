// Picture clean-up: denoise, deblock, sharpen and upscaling.
// All read A at its own resolution (src_size).

// Edge-preserving (bilateral) denoise. p0.x = strength 0..1.
@fragment
fn fs_denoise(in: VOut) -> @location(0) vec4<f32> {
    let strength = clamp(u.p0.x, 0.0, 1.0);
    let center = sa(in.uv);
    let radius = 1.0 + 4.0 * strength;
    let stride = max(radius / 3.0, 1.0);
    let sigma_s = radius * 0.6 + 0.5;
    let sigma_r = 0.025 + 0.13 * strength;
    let t = texel();
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var y = -3; y <= 3; y = y + 1) {
        for (var x = -3; x <= 3; x = x + 1) {
            let o = vec2<f32>(f32(x), f32(y)) * stride;
            let s = sa(in.uv + o * t);
            let dc = s.rgb - center.rgb;
            let ws = exp(-dot(o, o) / (2.0 * sigma_s * sigma_s));
            let wr = exp(-dot(dc, dc) / (2.0 * sigma_r * sigma_r));
            let w = ws * wr;
            acc = acc + s * w;
            wsum = wsum + w;
        }
    }
    return acc / max(wsum, 0.00001);
}

fn deblock_axis(uv: vec2<f32>, pos: f32, dir: vec2<f32>, block: f32, thr: f32, c: vec4<f32>) -> vec4<f32> {
    let into = fract(pos / block) * block;
    let dist = min(into, block - into);
    if dist > 1.6 {
        return c;
    }
    let t = texel() * dir;
    let l2 = sa(uv - 2.0 * t);
    let l1 = sa(uv - t);
    let r1 = sa(uv + t);
    let r2 = sa(uv + 2.0 * t);
    let jump = abs(luma(r1.rgb) - luma(l1.rgb));
    let inner = max(abs(luma(l1.rgb) - luma(l2.rgb)), abs(luma(r2.rgb) - luma(r1.rgb)));
    // Smooth only small steps in otherwise flat areas: those are block edges,
    // not real detail.
    if jump < thr && inner < thr * 0.6 {
        let w = (1.0 - dist / 1.6) * 0.75;
        let avg = (l2 + l1 * 2.0 + c * 2.0 + r1 * 2.0 + r2) / 8.0;
        return mix(c, avg, w);
    }
    return c;
}

// Removes the 8x8 grid left by heavy compression.
// p0 = (strength 0..1, block size in texels).
@fragment
fn fs_deblock(in: VOut) -> @location(0) vec4<f32> {
    let strength = clamp(u.p0.x, 0.0, 1.0);
    let block = max(u.p0.y, 2.0);
    let thr = 0.02 + 0.1 * strength;
    let pos = in.uv * u.src_size;
    var c = sa(in.uv);
    c = deblock_axis(in.uv, pos.x, vec2<f32>(1.0, 0.0), block, thr, c);
    c = deblock_axis(in.uv, pos.y, vec2<f32>(0.0, 1.0), block, thr, c);
    // Light ringing clean-up in flat areas.
    let t = texel();
    let n = (sa(in.uv + vec2<f32>(t.x, 0.0)) + sa(in.uv - vec2<f32>(t.x, 0.0)) + sa(in.uv + vec2<f32>(0.0, t.y)) + sa(in.uv - vec2<f32>(0.0, t.y))) * 0.25;
    let var_l = abs(luma(n.rgb) - luma(c.rgb));
    if var_l < thr * 0.5 {
        c = mix(c, n, strength * 0.5);
    }
    return c;
}

// Contrast-adaptive sharpening. p0.x = amount 0..1.
@fragment
fn fs_sharpen(in: VOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(in.uv * u.src_size);
    let b = la(p + vec2<i32>(0, -1));
    let d = la(p + vec2<i32>(-1, 0));
    let e = la(p);
    let f = la(p + vec2<i32>(1, 0));
    let h = la(p + vec2<i32>(0, 1));
    let mn = min(min(min(d.rgb, e.rgb), min(f.rgb, b.rgb)), h.rgb);
    let mx = max(max(max(d.rgb, e.rgb), max(f.rgb, b.rgb)), h.rgb);
    let amp = sqrt(clamp(min(mn, vec3<f32>(e.a) - mx) / max(mx, vec3<f32>(0.0001)), vec3<f32>(0.0), vec3<f32>(1.0)));
    let peak = -1.0 / mix(8.0, 4.0, clamp(u.p0.x, 0.0, 1.0));
    let w = amp * peak;
    let rgb = (b.rgb * w + d.rgb * w + f.rgb * w + h.rgb * w + e.rgb) / (1.0 + 4.0 * w);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(e.a)), e.a);
}

fn lanczos3(x: f32) -> f32 {
    let ax = abs(x);
    if ax < 0.0001 {
        return 1.0;
    }
    if ax >= 3.0 {
        return 0.0;
    }
    let px = PI * x;
    return 3.0 * sin(px) * sin(px / 3.0) / (px * px);
}

// Lanczos-3 resampling of the crop rect p0 (uv) of A to the destination
// size, with the result clamped to its nearest four texels so edges stay
// crisp without halos.
@fragment
fn fs_upscale(in: VOut) -> @location(0) vec4<f32> {
    let src_uv = mix(u.p0.xy, u.p0.zw, in.uv);
    let pos = src_uv * u.src_size - 0.5;
    let base = floor(pos);
    let f = pos - base;
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    var mn = vec4<f32>(1.0e9);
    var mx = vec4<f32>(-1.0e9);
    for (var y = -2; y <= 3; y = y + 1) {
        let wy = lanczos3(f32(y) - f.y);
        for (var x = -2; x <= 3; x = x + 1) {
            let wx = lanczos3(f32(x) - f.x);
            let s = la(vec2<i32>(base) + vec2<i32>(x, y));
            let w = wx * wy;
            acc = acc + s * w;
            wsum = wsum + w;
            if x >= 0 && x <= 1 && y >= 0 && y <= 1 {
                mn = min(mn, s);
                mx = max(mx, s);
            }
        }
    }
    let r = acc / max(wsum, 0.0001);
    return clamp(r, mn, mx);
}
