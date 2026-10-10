// Color correction and filter looks, all in one pass.
//
// p0 = (exposure, brightness, contrast, saturation)
// p1 = (highlights, shadows, whites, blacks)
// p2 = (temperature, tint, vibrance, hue in radians)
// p3 = (vignette, fade, strength 0..1, monochrome 0/1)
// m0 = (lift r, lift g, lift b, grain)
// m1 = (gain r, gain g, gain b, gamma)
// c0 = shadow tint (rgb, amount), c1 = highlight tint (rgb, amount)
// flags = Color tab looks to apply: 1 curves, 2 HSL, 4 LUT; progress = LUT strength
// Values are roughly -1..1 with 0 meaning "unchanged".

// The Color tab's looks (see looks.rs), bound only for this pass.
// tone_tex row 0: each channel's curve at i/255 (rgb); row 1: hue shift in
// turns, saturation and luminance change for the hue i/256.
@group(1) @binding(0) var tone_tex: texture_2d<f32>;
@group(1) @binding(1) var lut_tex: texture_3d<f32>;

const TONE_N: i32 = 256;

fn curve_at(ch: i32, v: f32) -> f32 {
    let p = clamp(v, 0.0, 1.0) * f32(TONE_N - 1);
    let i0 = min(i32(floor(p)), TONE_N - 2);
    let a = textureLoad(tone_tex, vec2<i32>(i0, 0), 0)[ch];
    let b = textureLoad(tone_tex, vec2<i32>(i0 + 1, 0), 0)[ch];
    return mix(a, b, p - f32(i0));
}

fn apply_curves(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(curve_at(0, c.r), curve_at(1, c.g), curve_at(2, c.b));
}

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let mx = max(c.r, max(c.g, c.b));
    let mn = min(c.r, min(c.g, c.b));
    let l = (mx + mn) * 0.5;
    let d = mx - mn;
    if d <= 0.0 {
        return vec3<f32>(0.0, 0.0, l);
    }
    let s = d / max(1.0 - abs(2.0 * l - 1.0), 1e-5);
    var h: f32;
    if mx == c.r {
        h = (c.g - c.b) / d;
    } else if mx == c.g {
        h = (c.b - c.r) / d + 2.0;
    } else {
        h = (c.r - c.g) / d + 4.0;
    }
    return vec3<f32>(fract(h / 6.0 + 1.0), clamp(s, 0.0, 1.0), l);
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let k = fract(vec3<f32>(hsl.x) + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0;
    let rgb = clamp(abs(k - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
    let ch = (1.0 - abs(2.0 * hsl.z - 1.0)) * hsl.y;
    return vec3<f32>(hsl.z) + (rgb - 0.5) * ch;
}

fn apply_hsl(c_in: vec3<f32>) -> vec3<f32> {
    let c = clamp(c_in, vec3<f32>(0.0), vec3<f32>(1.0));
    let chroma = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
    if chroma < 1e-4 {
        return c;
    }
    var hsl = rgb_to_hsl(c);
    let p = hsl.x * f32(TONE_N);
    let i0 = i32(floor(p)) % TONE_N;
    let i1 = (i0 + 1) % TONE_N;
    let d = mix(
        textureLoad(tone_tex, vec2<i32>(i0, 1), 0).xyz,
        textureLoad(tone_tex, vec2<i32>(i1, 1), 0).xyz,
        fract(p),
    );
    // Grays have no real hue, so they're left alone.
    let w = smoothstep(0.02, 0.2, chroma);
    hsl.x = fract(hsl.x + d.x * w + 1.0);
    hsl.y = clamp(hsl.y * (1.0 + d.y * w), 0.0, 1.0);
    let dl = d.z * w * 0.6;
    hsl.z = select(hsl.z * (1.0 + dl), hsl.z + (1.0 - hsl.z) * dl, dl > 0.0);
    return hsl_to_rgb(hsl);
}

fn apply_lut(c: vec3<f32>) -> vec3<f32> {
    let n = f32(textureDimensions(lut_tex).x);
    let uvw = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) * ((n - 1.0) / n) + vec3<f32>(0.5 / n);
    return textureSampleLevel(lut_tex, samp, uvw, 0.0).rgb;
}

fn rgb_to_yiq(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(c, vec3<f32>(0.299, 0.587, 0.114)),
        dot(c, vec3<f32>(0.596, -0.274, -0.322)),
        dot(c, vec3<f32>(0.211, -0.523, 0.312)),
    );
}

fn yiq_to_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(c, vec3<f32>(1.0, 0.956, 0.621)),
        dot(c, vec3<f32>(1.0, -0.272, -0.647)),
        dot(c, vec3<f32>(1.0, -1.106, 1.703)),
    );
}

fn grade_color(c_in: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    var c = clamp(c_in, vec3<f32>(0.0), vec3<f32>(1.0));
    let exposure = u.p0.x;
    let brightness = u.p0.y;
    let contrast = u.p0.z;
    let saturation = u.p0.w;
    let highlights = u.p1.x;
    let shadows = u.p1.y;
    let whites = u.p1.z;
    let blacks = u.p1.w;
    let temperature = u.p2.x;
    let tint = u.p2.y;
    let vibrance = u.p2.z;
    let hue = u.p2.w;

    // Exposure and white balance work on light, so do them in linear.
    if exposure != 0.0 || temperature != 0.0 || tint != 0.0 {
        var lin = srgb_to_linear(c);
        lin = lin * exp2(exposure * 2.0);
        let wb = vec3<f32>(1.0 + 0.35 * temperature - 0.1 * tint, 1.0 + 0.25 * tint, 1.0 - 0.35 * temperature - 0.1 * tint);
        lin = lin * wb;
        c = linear_to_srgb(lin);
    }

    // Levels: black and white points.
    let bp = -0.12 * blacks;
    let wp = 1.0 - 0.18 * whites;
    c = (c - vec3<f32>(bp)) / max(wp - bp, 0.05);

    // Highlights and shadows: move only the bright or dark parts.
    let l0 = clamp(luma(c), 0.0, 1.0);
    let sh_w = pow(1.0 - l0, 2.0);
    let hi_w = pow(l0, 2.0);
    c = c + vec3<f32>(shadows * 0.35 * sh_w + highlights * 0.3 * hi_w);

    // Brightness as a midtone gamma shift.
    if brightness != 0.0 {
        c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(exp2(-brightness * 0.9)));
    }
    // Contrast as an S-curve around the middle.
    if contrast != 0.0 {
        let k = 1.0 + contrast * (select(0.6, 1.2, contrast > 0.0));
        c = (c - 0.5) * k + 0.5;
    }
    // Gamma (m1.w), lift and gain.
    if u.m1.w != 0.0 {
        c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(exp2(-u.m1.w)));
    }
    c = c * (vec3<f32>(1.0) + u.m1.xyz) + u.m0.xyz * (vec3<f32>(1.0) - c);

    // Split toning.
    let l1 = clamp(luma(c), 0.0, 1.0);
    let sw = 1.0 - smoothstep(0.0, 0.55, l1);
    let hw = smoothstep(0.45, 1.0, l1);
    c = c + (u.c0.rgb - vec3<f32>(luma(u.c0.rgb))) * u.c0.a * sw;
    c = c + (u.c1.rgb - vec3<f32>(luma(u.c1.rgb))) * u.c1.a * hw;

    // Hue rotation.
    if hue != 0.0 {
        var yiq = rgb_to_yiq(c);
        let r = rot2(hue);
        let iq = r * yiq.yz;
        yiq = vec3<f32>(yiq.x, iq.x, iq.y);
        c = yiq_to_rgb(yiq);
    }

    // Saturation and vibrance.
    let l2 = luma(c);
    var sat = 1.0 + saturation;
    if vibrance != 0.0 {
        let s = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
        sat = sat * (1.0 + vibrance * (1.0 - clamp(s, 0.0, 1.0)));
    }
    c = mix(vec3<f32>(l2), c, max(sat, 0.0));
    if u.p3.w > 0.5 {
        c = vec3<f32>(l2);
    }

    // Fade lifts the blacks and softens the whites.
    let fade = u.p3.y;
    if fade != 0.0 {
        c = c * (1.0 - fade * 0.22) + vec3<f32>(fade * 0.14);
    }

    // Color tab looks: the LUT, then curves, then HSL.
    if u.flags != 0u {
        if (u.flags & 4u) != 0u {
            c = mix(c, apply_lut(c), u.progress);
        }
        if (u.flags & 1u) != 0u {
            c = apply_curves(c);
        }
        if (u.flags & 2u) != 0u {
            c = apply_hsl(c);
        }
    }

    // Vignette.
    let vig = u.p3.x;
    if vig != 0.0 {
        let p = (uv - 0.5) * 2.0;
        let d = length(p * vec2<f32>(1.0, 0.85));
        let v = smoothstep(0.35, 1.45, d);
        c = c * (1.0 - vig * v * 0.9);
    }

    // Grain.
    let grain = u.m0.w;
    if grain > 0.0 {
        let n = hash12(floor(uv * u.dst_size) + vec2<f32>(fract(u.time * 7.3) * 917.0, fract(u.time * 3.1) * 451.0)) - 0.5;
        c = c + vec3<f32>(n * grain * 0.25);
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_grade(in: VOut) -> @location(0) vec4<f32> {
    let src = unpremul(sa(in.uv));
    if src.a <= 0.0 {
        return vec4<f32>(0.0);
    }
    let graded = grade_color(src.rgb, in.uv);
    let rgb = mix(src.rgb, graded, u.p3.z);
    return premul(vec4<f32>(rgb, src.a));
}
