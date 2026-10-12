// Shared bindings and helpers for every TidyCuts render pass.
//
// Conventions:
// - Colors are gamma-encoded (sRGB / Rec.709 transfer) and premultiplied by alpha.
// - `uv` runs 0..1 with (0,0) at the top-left of the destination.
// - Spatial parameters are relative to the picture height so effects look
//   the same in the small preview and in a 4K export.

struct Params {
    dst_size: vec2<f32>,
    src_size: vec2<f32>,
    time: f32,
    progress: f32,
    kind: u32,
    flags: u32,
    p0: vec4<f32>,
    p1: vec4<f32>,
    p2: vec4<f32>,
    p3: vec4<f32>,
    m0: vec4<f32>,
    m1: vec4<f32>,
    c0: vec4<f32>,
    c1: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Params;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex_a: texture_2d<f32>;
@group(0) @binding(3) var tex_b: texture_2d<f32>;
@group(0) @binding(4) var tex_c: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> VOut {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var out: VOut;
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;

fn sa(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(tex_a, samp, uv, 0.0);
}

fn sb(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(tex_b, samp, uv, 0.0);
}

fn sc(uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(tex_c, samp, uv, 0.0);
}

// Samples A, returning transparent outside 0..1.
fn sa_clip(uv: vec2<f32>) -> vec4<f32> {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    return sa(uv);
}

fn sb_clip(uv: vec2<f32>) -> vec4<f32> {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return vec4<f32>(0.0);
    }
    return sb(uv);
}

// Exact texel fetch from A with edge clamping.
fn la(p: vec2<i32>) -> vec4<f32> {
    let d = vec2<i32>(textureDimensions(tex_a, 0)) - vec2<i32>(1);
    return textureLoad(tex_a, clamp(p, vec2<i32>(0), d), 0);
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn unpremul(c: vec4<f32>) -> vec4<f32> {
    if c.a <= 0.00001 {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(c.rgb / c.a, c.a);
}

fn premul(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn over(top: vec4<f32>, bottom: vec4<f32>) -> vec4<f32> {
    return top + bottom * (1.0 - top.a);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn rgb2hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4<f32>(c.bg, k.wz), vec4<f32>(c.gb, k.xy), step(c.b, c.g));
    let q = mix(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), step(p.x, c.r));
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);
    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

fn hash11(p: f32) -> f32 {
    var x = fract(p * 0.1031);
    x = x * (x + 33.33);
    x = x * (x + x);
    return fract(x);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let w = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, w.x), mix(c, d, w.x), w.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i = i + 1) {
        v = v + a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.2);
        a = a * 0.5;
    }
    return v;
}

// Fractal noise whose layers slide at different speeds, so the pattern flows
// instead of popping. `t` is in seconds.
fn flow_fbm(p: vec2<f32>, t: f32) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i = i + 1) {
        v = v + a * vnoise(q + vec2<f32>(t * (0.35 + 0.2 * f32(i)), -t * 0.25));
        q = q * 2.03 + vec2<f32>(17.1, 9.2);
        a = a * 0.5;
    }
    return v;
}

// Domain-warped noise: sampled at points pushed around by two other noises,
// so it reads as drifting smoke or flame and not as clouds.
fn warp_fbm(p: vec2<f32>, t: f32) -> f32 {
    let q = vec2<f32>(flow_fbm(p, t), flow_fbm(p + vec2<f32>(5.2, 1.3), t));
    return flow_fbm(p + 3.5 * q, t * 0.8);
}

// A swirling, divergence-free vector field made from the noise. Moving
// things along it makes them curl around each other like smoke.
fn curl2(p: vec2<f32>, t: f32) -> vec2<f32> {
    let e = 0.02;
    let dx = flow_fbm(p + vec2<f32>(e, 0.0), t) - flow_fbm(p - vec2<f32>(e, 0.0), t);
    let dy = flow_fbm(p + vec2<f32>(0.0, e), t) - flow_fbm(p - vec2<f32>(0.0, e), t);
    return vec2<f32>(dy, -dx) / (2.0 * e);
}

fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

// Aspect ratio of the destination (width / height).
fn aspect() -> f32 {
    return u.dst_size.x / max(u.dst_size.y, 1.0);
}

// uv with x scaled so distances are square, centred on (0,0).
fn centered(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>((uv.x - 0.5) * aspect(), uv.y - 0.5);
}

// One destination pixel in uv units.
fn px() -> vec2<f32> {
    return 1.0 / max(u.dst_size, vec2<f32>(1.0));
}

// One source texel in uv units.
fn texel() -> vec2<f32> {
    return 1.0 / max(u.src_size, vec2<f32>(1.0));
}

// Gaussian-ish blur of A around uv with `radius` in uv-height units, few taps.
fn blur_a(uv: vec2<f32>, radius: f32) -> vec4<f32> {
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    let r = vec2<f32>(radius / aspect(), radius);
    for (var i = 0; i < 16; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let rad = sqrt((fi + 0.5) / 16.0);
        let o = vec2<f32>(cos(ang), sin(ang)) * rad * r;
        let w = 1.0 - rad * 0.5;
        acc = acc + sa(uv + o) * w;
        wsum = wsum + w;
    }
    return acc / wsum;
}

// --- Simple passes -------------------------------------------------------

// Y, U and V planes (A, B, C) to premultiplied RGBA.
// p0 = (y offset, y scale, c scale, alpha), p1 = (r_v, g_u, g_v, b_u).
@fragment
fn fs_source_yuv(in: VOut) -> @location(0) vec4<f32> {
    let y = (sa(in.uv).r - u.p0.x) * u.p0.y;
    let cb = (sb(in.uv).r - 0.50196) * u.p0.z;
    let cr = (sc(in.uv).r - 0.50196) * u.p0.z;
    let rgb = vec3<f32>(y + u.p1.x * cr, y - u.p1.y * cb - u.p1.z * cr, y + u.p1.w * cb);
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// RGBA texture to premultiplied RGBA. flags bit 0: input is straight alpha.
@fragment
fn fs_source_rgba(in: VOut) -> @location(0) vec4<f32> {
    let c = sa(in.uv);
    if (u.flags & 1u) != 0u {
        return premul(c);
    }
    return c;
}

// Solid color (c0) or linear gradient c0 -> c1 along angle p0.x (radians).
@fragment
fn fs_solid(in: VOut) -> @location(0) vec4<f32> {
    if (u.flags & 1u) == 0u {
        return premul(u.c0);
    }
    let d = vec2<f32>(cos(u.p0.x), sin(u.p0.x));
    let p = centered(in.uv);
    let ext = 0.5 * (abs(d.x) * aspect() + abs(d.y));
    let t = clamp(dot(p, d) / max(ext * 2.0, 0.0001) + 0.5, 0.0, 1.0);
    return premul(mix(u.c0, u.c1, t));
}

// Plain copy of A.
@fragment
fn fs_copy(in: VOut) -> @location(0) vec4<f32> {
    return sa(in.uv);
}

// Mix of A (after) over B (before) by p0.x, used for adjustment-layer opacity.
@fragment
fn fs_mix(in: VOut) -> @location(0) vec4<f32> {
    return mix(sb(in.uv), sa(in.uv), u.p0.x);
}

// A over B, both premultiplied.
@fragment
fn fs_over(in: VOut) -> @location(0) vec4<f32> {
    return over(sa(in.uv), sb(in.uv));
}

// Drop shadow: the alpha of A (a layer placed on a clear canvas), moved by
// p0.xy (uv) and tinted with c0 (straight alpha, a = opacity).
@fragment
fn fs_shadow(in: VOut) -> @location(0) vec4<f32> {
    let a = sa_clip(in.uv - u.p0.xy).a * u.c0.a;
    return vec4<f32>(u.c0.rgb * a, a);
}

// Track matte: A = the layer, B = the matte (both placed on clear canvases),
// C = the canvas. kind: 1 alpha, 2 alpha inverted, 3 luma, 4 luma inverted.
@fragment
fn fs_matte(in: VOut) -> @location(0) vec4<f32> {
    let m = sb(in.uv);
    var f = m.a;
    if u.kind >= 3u {
        f = luma(unpremul(m).rgb) * m.a;
    }
    if u.kind == 2u || u.kind == 4u {
        f = 1.0 - f;
    }
    return over(sa(in.uv) * clamp(f, 0.0, 1.0), sc(in.uv));
}

// Final pass: canvas to the output texture. flags bit 0: target is sRGB,
// so write linear values and let the hardware encode them.
@fragment
fn fs_output(in: VOut) -> @location(0) vec4<f32> {
    let c = sa(in.uv);
    let rgb = clamp(c.rgb + (1.0 - c.a) * u.c0.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if (u.flags & 1u) != 0u {
        return vec4<f32>(srgb_to_linear(rgb), 1.0);
    }
    return vec4<f32>(rgb, 1.0);
}

// Final passes for video files (see Compositor::submit_nv12): 8-bit limited
// range BT.709, which is what the encoders are told the picture is.
fn canvas_rgb(uv: vec2<f32>) -> vec3<f32> {
    let c = sa(uv);
    return clamp(c.rgb + (1.0 - c.a) * u.c0.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
}

// The brightness plane, one value per pixel.
@fragment
fn fs_output_y(in: VOut) -> @location(0) vec4<f32> {
    let rgb = canvas_rgb(in.uv);
    let y = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>((16.0 + 219.0 * y) / 255.0, 0.0, 0.0, 1.0);
}

// The colour plane at half size: blue and red differences side by side.
// Each value comes from the left pixel of a 2 by 2 block, with its
// neighbours either side weighted 1-2-1 and the two rows averaged, which is
// where video players expect the colour to sit.
@fragment
fn fs_output_uv(in: VOut) -> @location(0) vec4<f32> {
    let block = floor(in.pos.xy);
    let size = u.src_size;
    let x = block.x * 2.0 + 0.5;
    let y = block.y * 2.0 + 1.0;
    let rgb = 0.25 * canvas_rgb(vec2<f32>((x - 1.0) / size.x, y / size.y))
        + 0.5 * canvas_rgb(vec2<f32>(x / size.x, y / size.y))
        + 0.25 * canvas_rgb(vec2<f32>((x + 1.0) / size.x, y / size.y));
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let cb = (rgb.b - luma) / 1.8556;
    let cr = (rgb.r - luma) / 1.5748;
    return vec4<f32>((128.0 + 224.0 * cb) / 255.0, (128.0 + 224.0 * cr) / 255.0, 0.0, 1.0);
}
