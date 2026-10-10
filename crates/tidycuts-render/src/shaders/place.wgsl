// Placing a processed layer (A) onto the canvas (C) with its transform,
// crop, opacity and blend mode.
//
// m0.xyz / m1.xyz: affine map from destination pixel to layer-local uv (0..1).
// p0: crop rect in source uv (x0, y0, x1, y1).
// p1: (opacity, blend mode, sample mode, corner radius in dst px).
// p2: (layer width in dst px, layer height in dst px, 0, 0).
// flags: bit 0 flip horizontal, bit 1 flip vertical.
// Mask (optional): p3 = (shape + 1, centre x, centre y, rotation radians),
// c0 = (width, height, roundness, feather px), c1.x = invert. Mask centre
// and size are layer-local fractions.

fn cubic_weights(f: f32) -> vec4<f32> {
    // Catmull-Rom.
    let f2 = f * f;
    let f3 = f2 * f;
    return vec4<f32>(
        -0.5 * f3 + f2 - 0.5 * f,
        1.5 * f3 - 2.5 * f2 + 1.0,
        -1.5 * f3 + 2.0 * f2 + 0.5 * f,
        0.5 * f3 - 0.5 * f2,
    );
}

fn sample_bicubic_a(uv: vec2<f32>) -> vec4<f32> {
    let size = u.src_size;
    let p = uv * size - 0.5;
    let i = floor(p);
    let f = p - i;
    let wx = cubic_weights(f.x);
    let wy = cubic_weights(f.y);
    var acc = vec4<f32>(0.0);
    for (var y = 0; y < 4; y = y + 1) {
        var row = vec4<f32>(0.0);
        for (var x = 0; x < 4; x = x + 1) {
            row = row + la(vec2<i32>(i) + vec2<i32>(x - 1, y - 1)) * wx[x];
        }
        acc = acc + row * wy[y];
    }
    // Catmull-Rom overshoots; keep premultiplied color valid.
    let a = clamp(acc.a, 0.0, 1.0);
    return vec4<f32>(clamp(acc.rgb, vec3<f32>(0.0), vec3<f32>(a)), a);
}

fn blend_channel_burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 {
        return 1.0;
    }
    if s <= 0.0 {
        return 0.0;
    }
    return 1.0 - min(1.0, (1.0 - b) / s);
}

fn blend_channel_dodge(b: f32, s: f32) -> f32 {
    if b <= 0.0 {
        return 0.0;
    }
    if s >= 1.0 {
        return 1.0;
    }
    return min(1.0, b / (1.0 - s));
}

fn hard_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let lo = b * 2.0 * s;
    let hi = 1.0 - (1.0 - b) * (1.0 - (2.0 * s - 1.0));
    return select(hi, lo, s <= vec3<f32>(0.5));
}

fn soft_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= vec3<f32>(0.25));
    let lo = b - (1.0 - 2.0 * s) * b * (1.0 - b);
    let hi = b + (2.0 * s - 1.0) * (d - b);
    return select(hi, lo, s <= vec3<f32>(0.5));
}

fn blend_rgb(mode: u32, b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 1u: { return min(b, s); }
        case 2u: { return b * s; }
        case 3u: {
            return vec3<f32>(blend_channel_burn(b.r, s.r), blend_channel_burn(b.g, s.g), blend_channel_burn(b.b, s.b));
        }
        case 4u: { return max(b + s - 1.0, vec3<f32>(0.0)); }
        case 5u: { return max(b, s); }
        case 6u: { return b + s - b * s; }
        case 7u: {
            return vec3<f32>(blend_channel_dodge(b.r, s.r), blend_channel_dodge(b.g, s.g), blend_channel_dodge(b.b, s.b));
        }
        case 8u: { return min(b + s, vec3<f32>(1.0)); }
        case 9u: { return hard_light(s, b); }
        case 10u: { return soft_light(b, s); }
        case 11u: { return hard_light(b, s); }
        case 12u: { return abs(b - s); }
        case 13u: { return b + s - 2.0 * b * s; }
        case 14u: { return max(b - s, vec3<f32>(0.0)); }
        default: { return s; }
    }
}

// Composites premultiplied `s` onto premultiplied `d` with a blend mode.
fn composite(mode: u32, s: vec4<f32>, d: vec4<f32>) -> vec4<f32> {
    if mode == 0u || s.a <= 0.0 {
        return s + d * (1.0 - s.a);
    }
    let cs = s.rgb / max(s.a, 0.00001);
    let cb = d.rgb / max(d.a, 0.00001);
    let bl = clamp(blend_rgb(mode, cb, cs), vec3<f32>(0.0), vec3<f32>(1.0));
    let rgb = (1.0 - d.a) * s.rgb + (1.0 - s.a) * d.rgb + s.a * d.a * bl;
    return vec4<f32>(rgb, s.a + d.a * (1.0 - s.a));
}

// Distance (in dst pixels) from a point inside the layer to its edge,
// with optional rounded corners.
fn edge_coverage(local: vec2<f32>) -> f32 {
    let size = u.p2.xy;
    let p = (local - 0.5) * size;
    let half_size = size * 0.5;
    let r = min(u.p1.w, min(half_size.x, half_size.y));
    let q = abs(p) - (half_size - vec2<f32>(r));
    let dist = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    return clamp(0.5 - dist, 0.0, 1.0);
}

fn mask_star(p_in: vec2<f32>, r: f32) -> f32 {
    // Five-pointed star distance (after Inigo Quilez).
    let k1 = vec2<f32>(0.809016994, -0.587785252);
    let k2 = vec2<f32>(-k1.x, k1.y);
    var p = vec2<f32>(abs(p_in.x), -p_in.y);
    p = p - 2.0 * max(dot(k1, p), 0.0) * k1;
    p = p - 2.0 * max(dot(k2, p), 0.0) * k2;
    p.x = abs(p.x);
    p.y = p.y - r;
    let rf = 0.45;
    let ba = rf * vec2<f32>(-k1.y, k1.x) - vec2<f32>(0.0, 1.0);
    let h = clamp(dot(p, ba) / dot(ba, ba), 0.0, r);
    return length(p - ba * h) * sign(p.y * ba.x - p.x * ba.y);
}

fn mask_heart(p_in: vec2<f32>, r: f32) -> f32 {
    // Heart distance (after Inigo Quilez), unit size scaled by r.
    var p = vec2<f32>(abs(p_in.x), -p_in.y) / (r * 1.6) + vec2<f32>(0.0, 0.55);
    var d: f32;
    if p.y + p.x > 1.0 {
        d = sqrt(dot(p - vec2<f32>(0.25, 0.75), p - vec2<f32>(0.25, 0.75))) - sqrt(2.0) / 4.0;
    } else {
        let a = dot(p - vec2<f32>(0.0, 1.0), p - vec2<f32>(0.0, 1.0));
        let m = 0.5 * max(p.x + p.y, 0.0);
        let b = dot(p - vec2<f32>(m), p - vec2<f32>(m));
        d = sqrt(min(a, b)) * sign(p.x - p.y);
    }
    return d * r * 1.6;
}

// Coverage of the mask at a layer-local point (1 = shown).
fn mask_coverage(local: vec2<f32>) -> f32 {
    if u.p3.x < 0.5 {
        return 1.0;
    }
    let shape = u32(u.p3.x - 0.5);
    let ext = u.p2.xy;
    let q0 = (local - u.p3.yz) * ext;
    let q = rot2(-u.p3.w) * q0;
    let hs = max(u.c0.xy * ext * 0.5, vec2<f32>(0.5));
    let m = min(hs.x, hs.y);
    var d: f32;
    switch shape {
        case 0u: { d = q.y; }
        case 1u: { d = abs(q.y) - hs.y; }
        case 2u: {
            let r = clamp(u.c0.z, 0.0, 1.0) * m;
            let b = abs(q) - (hs - vec2<f32>(r));
            d = length(max(b, vec2<f32>(0.0))) + min(max(b.x, b.y), 0.0) - r;
        }
        case 3u: { d = (length(q / hs) - 1.0) * m; }
        case 4u: { d = mask_star(q / hs * m, m); }
        default: { d = mask_heart(q / hs * m, m); }
    }
    let w = max(u.c0.w, 0.75);
    var c = 1.0 - smoothstep(-w, w, d);
    if u.c1.x > 0.5 {
        c = 1.0 - c;
    }
    return c;
}

@fragment
fn fs_place(in: VOut) -> @location(0) vec4<f32> {
    let pix = in.uv * u.dst_size;
    var local = vec2<f32>(
        dot(u.m0.xy, pix) + u.m0.z,
        dot(u.m1.xy, pix) + u.m1.z,
    );
    let dst = sc(in.uv);
    let cover = edge_coverage(local) * mask_coverage(local);
    if cover <= 0.0 {
        return dst;
    }
    if (u.flags & 1u) != 0u {
        local.x = 1.0 - local.x;
    }
    if (u.flags & 2u) != 0u {
        local.y = 1.0 - local.y;
    }
    let uv = mix(u.p0.xy, u.p0.zw, clamp(local, vec2<f32>(0.0), vec2<f32>(1.0)));
    var s: vec4<f32>;
    if u.p1.z > 0.5 {
        s = sample_bicubic_a(uv);
    } else {
        s = sa(uv);
    }
    s = s * (u.p1.x * cover);
    return composite(u32(u.p1.y + 0.5), s, dst);
}
