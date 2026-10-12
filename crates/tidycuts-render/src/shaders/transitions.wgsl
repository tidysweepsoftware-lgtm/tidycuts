// Transitions. A is the outgoing picture, B the incoming one (both
// canvas-sized, premultiplied, possibly transparent), C the canvas below.
// `u.progress` runs 0..1, `u.kind` picks the transition (ids match
// catalog/transitions.rs), parameters are in p0 and colors in c0.
//
// Every transition returns exactly A at progress 0 and exactly B at
// progress 1 (no leftover offset, tint, blur or scale); the
// `transitions_sheet` example checks this. Sizes are fractions of the
// picture height so the small preview and the export look the same.

// Distance from the eye to the screen for the 3D transitions, in picture heights.
const TR_EYE: f32 = 2.2;

fn tr_ease(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

// Cubic ease in-out: slower start and end than tr_ease, faster middle.
fn tr_ease3(t: f32) -> f32 {
    if t < 0.5 {
        return 4.0 * t * t * t;
    }
    let f = 2.0 - 2.0 * t;
    return 1.0 - 0.5 * f * f * f;
}

// 0 at both ends, 1 in the middle.
fn tr_bump(t: f32) -> f32 {
    return max(sin(t * PI), 0.0);
}

// Anti-aliasing width (about 1.5 output pixels) in picture heights.
fn tr_aa() -> f32 {
    return 1.5 / max(u.dst_size.y, 1.0);
}

// Weight of A for a soft edge sweeping over x in 0..1: exactly 1 at t = 0
// and exactly 0 at t = 1. Lower x is reached first.
fn tr_sweep(x: f32, t: f32, soft: f32) -> f32 {
    let s = max(soft, 0.0001);
    let e = mix(-s, 1.0 + s, t);
    return smoothstep(e - s, e + s, x);
}

// Movement direction for the direction parameter: 0 left, 1 right, 2 up, 3 down.
fn tr_dir(code: f32) -> vec2<f32> {
    let d = u32(max(code, 0.0) + 0.5);
    if d == 1u {
        return vec2<f32>(1.0, 0.0);
    }
    if d == 2u {
        return vec2<f32>(0.0, -1.0);
    }
    if d == 3u {
        return vec2<f32>(0.0, 1.0);
    }
    return vec2<f32>(-1.0, 0.0);
}

fn tr_in01(q: vec2<f32>) -> bool {
    return all(q >= vec2<f32>(0.0)) && all(q <= vec2<f32>(1.0));
}

// How far q lies outside 0..1 (negative inside).
fn tr_outside(q: vec2<f32>) -> f32 {
    let o = max(-q, q - vec2<f32>(1.0));
    return max(o.x, o.y);
}

// Mirrored repeat, so rotated or zoomed pictures have no empty corners.
fn tr_mirror(q: vec2<f32>) -> vec2<f32> {
    let f = fract(q * 0.5) * 2.0;
    return vec2<f32>(1.0) - abs(f - vec2<f32>(1.0));
}

// Centred coordinates back to uv.
fn tr_uv(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x / aspect() + 0.5, p.y + 0.5);
}

// Darkens rgb only (alpha unchanged).
fn tr_dim(c: vec4<f32>, k: f32) -> vec4<f32> {
    return vec4<f32>(c.rgb * k, c.a);
}

// Adds light (straight rgb) to a premultiplied color.
fn tr_add(c: vec4<f32>, light: vec3<f32>) -> vec4<f32> {
    let l = max(light, vec3<f32>(0.0));
    let rgb = min(c.rgb + l, vec3<f32>(1.0));
    let a = clamp(max(c.a, max(rgb.r, max(rgb.g, rgb.b))), 0.0, 1.0);
    return vec4<f32>(rgb, a);
}

// Screen-blends light (straight rgb, 0..1) onto a premultiplied color.
fn tr_screen(c: vec4<f32>, light: vec3<f32>) -> vec4<f32> {
    let l = clamp(light, vec3<f32>(0.0), vec3<f32>(1.0));
    let rgb = c.rgb + l * (vec3<f32>(1.0) - c.rgb);
    let a = clamp(max(c.a, max(rgb.r, max(rgb.g, rgb.b))), 0.0, 1.0);
    return vec4<f32>(rgb, a);
}

// --- 3D helpers -------------------------------------------------------------
// Space: x right, y down, z into the screen, in picture heights. The screen
// is the plane z = 0 and the eye sits at z = -TR_EYE, so a picture plane at
// z = 0 with unit axes maps exactly onto the frame.

struct TrHit {
    uv: vec2<f32>,
    // The ray reached the plane in front of the eye.
    hit: bool,
    // Seen from the front.
    front: bool,
    // Cosine between the plane normal and the view axis (1 = facing the camera).
    facing: f32,
}

// Rotates v by angle a about the y axis (a horizontal turn) or the x axis.
fn tr_turn(v: vec3<f32>, a: f32, vertical: bool) -> vec3<f32> {
    let c = cos(a);
    let s = sin(a);
    if vertical {
        return vec3<f32>(v.x, v.y * c + v.z * s, -v.y * s + v.z * c);
    }
    return vec3<f32>(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
}

// Casts the ray through the screen point p (centred coordinates) onto a
// picture plane with centre o and unit axes ex, ey.
fn tr_plane(p: vec2<f32>, o: vec3<f32>, ex: vec3<f32>, ey: vec3<f32>) -> TrHit {
    var h: TrHit;
    h.uv = vec2<f32>(-1.0);
    h.hit = false;
    h.front = false;
    h.facing = 0.0;
    let eye = vec3<f32>(0.0, 0.0, -TR_EYE);
    let dir = vec3<f32>(p, TR_EYE);
    let n = cross(ex, ey);
    let den = dot(dir, n);
    if abs(den) < 0.000001 {
        return h;
    }
    let lam = dot(o - eye, n) / den;
    if lam <= 0.0 {
        return h;
    }
    let l = eye + dir * lam - o;
    h.uv = tr_uv(vec2<f32>(dot(l, ex), dot(l, ey)));
    h.hit = tr_in01(h.uv);
    h.front = den > 0.0;
    h.facing = abs(n.z);
    return h;
}

// Light falloff for a turned face: 1 facing the camera, darker edge-on.
fn tr_face(c: vec4<f32>, facing: f32) -> vec4<f32> {
    return tr_dim(c, mix(0.42, 1.0, clamp(facing, 0.0, 1.0)));
}

// --- Fade & dissolve ----------------------------------------------------------

fn tr_dissolve(uv: vec2<f32>, t: f32) -> vec4<f32> {
    return mix(sa(uv), sb(uv), t);
}

// Through a color (black by default, c0).
fn tr_dip(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let col = premul(u.c0);
    if t < 0.5 {
        return mix(sa(uv), col, tr_ease(t * 2.0));
    }
    return mix(col, sb(uv), tr_ease(t * 2.0 - 1.0));
}

// Luma fade: the bright parts of A give way first. p0.x = 1 starts with
// the dark parts, p0.y = softness.
fn tr_luma(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let a = sa(uv);
    var l = luma(unpremul(a).rgb);
    if u.p0.x > 0.5 {
        l = 1.0 - l;
    }
    let m = tr_sweep(1.0 - l, t, max(u.p0.y, 0.02));
    return mix(sb(uv), a, m);
}

// Additive dissolve: both pictures add up, overexposing the middle.
fn tr_additive(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let wa = clamp((1.0 - t) * 1.8, 0.0, 1.0);
    let wb = clamp(t * 1.8, 0.0, 1.0);
    let c = sa(uv) * wa + sb(uv) * wb;
    return vec4<f32>(min(c.rgb, vec3<f32>(1.0)), min(c.a, 1.0));
}

// Noise dissolve (soft blotches).
fn tr_noise(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = fbm(uv * vec2<f32>(aspect(), 1.0) * 4.0);
    let e = mix(-0.25, 1.25, t);
    let m = smoothstep(e - 0.12, e + 0.12, n * 1.2 - 0.1);
    return mix(sb(uv), sa(uv), m);
}

// --- Wipes --------------------------------------------------------------------

// Wipe with a soft edge. p0.x = direction (0 left→right, 1 right→left,
// 2 top→bottom, 3 bottom→top, 4 diagonal), p0.y = softness (0 = narrow).
fn tr_wipe(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let dir = u32(u.p0.x + 0.5);
    var x = uv.x;
    if dir == 1u {
        x = 1.0 - uv.x;
    } else if dir == 2u {
        x = uv.y;
    } else if dir == 3u {
        x = 1.0 - uv.y;
    } else if dir == 4u {
        x = (uv.x * aspect() + uv.y) / (aspect() + 1.0);
    }
    let soft = select(0.02, u.p0.y * 0.5, u.p0.y > 0.0);
    let m = tr_sweep(x, t, max(soft, tr_aa() * 0.5));
    return mix(sb(uv), sa(uv), m);
}

// Barn doors: B opens from the middle. p0.x = 0 vertical seam, 1
// horizontal seam; p0.y = 1 closes in from the edges instead.
fn tr_barn(uv: vec2<f32>, t: f32) -> vec4<f32> {
    var x = uv.x;
    if u.p0.x > 0.5 {
        x = uv.y;
    }
    var d = abs(x - 0.5) * 2.0;
    if u.p0.y > 0.5 {
        d = 1.0 - d;
    }
    let m = tr_sweep(d, tr_ease(t), max(0.012, tr_aa()));
    return mix(sb(uv), sa(uv), m);
}

// Venetian blinds: A's slats turn edge-on, uncovering B behind them.
// p0.x = number of slats, p0.y = 1 for vertical slats.
fn tr_blinds(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = max(round(u.p0.x), 1.0);
    let vertical = u.p0.y > 0.5;
    var x = uv.y;
    if vertical {
        x = uv.x;
    }
    let k = floor(x * n);
    let f = x * n - k;
    let lt = clamp(t * 1.4 - (k / n) * 0.4, 0.0, 1.0);
    let c = 1.0 - lt;
    let b = sb(uv) * min(t * 10.0, 1.0);
    if c < 0.0001 {
        return b;
    }
    let fs = (f - 0.5) / c + 0.5;
    if fs < 0.0 || fs > 1.0 {
        return b;
    }
    var q = vec2<f32>(uv.x, (k + fs) / n);
    if vertical {
        q = vec2<f32>((k + fs) / n, uv.y);
    }
    // Slats darken as they turn edge-on; the upper edge catches the light.
    let shade = mix(0.45, 1.0, c) + 0.45 * (1.0 - c) * (1.0 - smoothstep(0.0, 0.35, fs));
    return over(tr_dim(sa(q), shade), b);
}

// Checkerboard: alternate squares wipe in two waves. p0.x = squares per height.
fn tr_checker(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = max(u.p0.x, 1.0);
    let g = vec2<f32>(uv.x * aspect(), uv.y) * n;
    let cell = floor(g);
    let f = g - cell;
    let odd = ((i32(cell.x) + i32(cell.y)) & 1) == 1;
    let start = select(0.0, 0.5, odd);
    let lt = clamp((t - start) * 2.0, 0.0, 1.0);
    let m = tr_sweep(f.y, lt, max(0.02, tr_aa() * n));
    return mix(sb(uv), sa(uv), m);
}

// Clock wipe.
fn tr_clock(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let p = centered(uv);
    let a = fract(atan2(p.x, -p.y) / TAU + 1.0);
    let w = clamp(tr_aa() / (TAU * max(length(p), 0.0001)), 0.0005, 0.02);
    let e = mix(-0.02, 1.02, t);
    let m = smoothstep(e - w, e + w, a);
    return mix(sb(uv), sa(uv), m);
}

// --- Slides -------------------------------------------------------------------

// Push: the new picture pushes the old one out. p0.x = direction.
fn tr_push(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let d = tr_dir(u.p0.x);
    let a = sa_clip(uv - d * e);
    let b = sb_clip(uv - d * (e - 1.0));
    return a + b;
}

// Slide over: the new picture slides in on top, casting a soft shadow.
// p0.x = direction.
fn tr_cover(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let d = tr_dir(u.p0.x);
    let pb = uv - d * (e - 1.0);
    let b = sb_clip(pb);
    let shadow = 0.45 * (1.0 - smoothstep(0.0, 0.05, tr_outside(pb))) * min(t * 8.0, 1.0);
    let a = tr_dim(sa(uv), 1.0 - shadow) * (1.0 - smoothstep(0.95, 1.0, t));
    return over(b, a);
}

// Uncover: the old picture slides away, uncovering the new one.
// p0.x = direction.
fn tr_uncover(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let d = tr_dir(u.p0.x);
    let pa = uv - d * e;
    let a = sa_clip(pa);
    let shadow = 0.45 * (1.0 - smoothstep(0.0, 0.05, tr_outside(pa))) * min((1.0 - t) * 8.0, 1.0);
    let b = tr_dim(sb(uv), 1.0 - shadow) * min(t * 10.0, 1.0);
    return over(a, b);
}

// A and B side by side, B following A in direction d, with clamped edges.
fn tr_push_at(q: vec2<f32>, d: vec2<f32>, e: f32) -> vec4<f32> {
    let pa = q - d * e;
    let s = dot(pa - vec2<f32>(0.5), -d) + 0.5;
    if s < 1.0 {
        return sa(pa);
    }
    return sb(pa + d);
}

// Whip pan: a very fast push smeared by motion blur. p0.x = direction.
fn tr_whip(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let e = tr_ease3(t);
    let s = tr_bump(t);
    let len = 0.35 * s * s;
    let j = hash12(uv * u.dst_size);
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 32; i = i + 1) {
        let o = ((f32(i) + j) / 32.0 - 0.5) * len;
        acc = acc + tr_push_at(uv + d * o, d, e);
    }
    return acc / 32.0;
}

// --- Zooms --------------------------------------------------------------------

// Cross zoom: zoom into the old picture with blur, out of the new one.
fn tr_cross_zoom(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let strength = 0.4;
    let e = tr_ease(t);
    let blur = sin(e * PI) * strength;
    let c = vec2<f32>(0.5);
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 20; i = i + 1) {
        let s = f32(i) / 19.0;
        let p = c + (uv - c) * (1.0 - blur * s);
        acc = acc + mix(sa(p), sb(p), smoothstep(0.35, 0.65, e));
    }
    return acc / 20.0;
}

// Zoom in: the old picture grows and fades into the new one.
fn tr_zoom_in(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let a = sa(vec2<f32>(0.5) + (uv - 0.5) / (1.0 + e * 1.5));
    let b = sb(vec2<f32>(0.5) + (uv - 0.5) * (1.0 + (1.0 - e) * 0.3));
    return mix(a, b, smoothstep(0.2, 0.8, e));
}

// Zoom out: the old picture shrinks away while the new one settles from
// close up.
fn tr_zoom_out(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let c = vec2<f32>(0.5);
    let ka = 1.0 - 0.7 * e;
    let kb = 1.0 + 1.0 * (1.0 - e);
    let blur = 0.12 * tr_bump(t);
    let j = hash12(uv * u.dst_size);
    var b = vec4<f32>(0.0);
    for (var i = 0; i < 10; i = i + 1) {
        let k = 1.0 + blur * ((f32(i) + j) / 10.0 - 0.5);
        b = b + sb(c + (uv - c) / (kb * k));
    }
    b = b * (min(t * 10.0, 1.0) / 10.0);
    let pa = c + (uv - c) / ka;
    let fa = 1.0 - smoothstep(0.5, 0.95, t);
    // Soft shadow around the shrinking picture.
    let shadow = 0.55 * (1.0 - smoothstep(0.0, 0.05, tr_outside(pa) * ka)) * min(t * 8.0, 1.0) * fa;
    return over(sa_clip(pa) * fa, tr_dim(b, 1.0 - shadow));
}

// Spin zoom: a full turn, zooming in on A and back out of B, smeared
// along the spin.
fn tr_spin_zoom(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let ang = e * TAU;
    let zoom = 1.0 + 0.7 * tr_bump(t);
    // Angular speed of tr_ease peaks in the middle.
    let blur = 0.9 * tr_bump(t) * tr_bump(t);
    let w = smoothstep(0.44, 0.56, t);
    let p = centered(uv);
    let j = hash12(uv * u.dst_size);
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 20; i = i + 1) {
        let o = ((f32(i) + j) / 20.0 - 0.5) * blur;
        let qa = tr_mirror(tr_uv(rot2(ang + o) * p / zoom));
        let qb = tr_mirror(tr_uv(rot2(ang - TAU + o) * p / zoom));
        acc = acc + mix(sa(qa), sb(qb), w);
    }
    return acc / 20.0;
}

// --- 3D -----------------------------------------------------------------------

// Cube: the pictures sit on two faces of a turning cube. p0.x = direction.
fn tr_cube(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let dir = u32(u.p0.x + 0.5);
    let vertical = dir >= 2u;
    let sgn = select(1.0, -1.0, dir == 1u || dir == 3u);
    let side = select(aspect(), 1.0, vertical);
    let hs = side * 0.5;
    let a = sgn * tr_ease(t) * PI * 0.5;
    let c = vec3<f32>(0.0, 0.0, hs + tr_bump(t) * side * 0.35);
    let p = centered(uv);
    let ha = tr_plane(
        p,
        c + tr_turn(vec3<f32>(0.0, 0.0, -hs), a, vertical),
        tr_turn(vec3<f32>(1.0, 0.0, 0.0), a, vertical),
        tr_turn(vec3<f32>(0.0, 1.0, 0.0), a, vertical),
    );
    if ha.hit && ha.front {
        return tr_face(sa(ha.uv), ha.facing);
    }
    var ob = vec3<f32>(sgn * hs, 0.0, 0.0);
    var exb = vec3<f32>(0.0, 0.0, sgn);
    var eyb = vec3<f32>(0.0, 1.0, 0.0);
    if vertical {
        ob = vec3<f32>(0.0, sgn * hs, 0.0);
        exb = vec3<f32>(1.0, 0.0, 0.0);
        eyb = vec3<f32>(0.0, 0.0, sgn);
    }
    let hb = tr_plane(p, c + tr_turn(ob, a, vertical), tr_turn(exb, a, vertical), tr_turn(eyb, a, vertical));
    if hb.hit && hb.front {
        return tr_face(sb(hb.uv), hb.facing);
    }
    return vec4<f32>(0.0);
}

// Page curl: A peels off around a cylinder, showing its pale back.
// p0.x = 0 from the bottom-right corner, 1 from the right edge.
fn tr_curl(uv: vec2<f32>, t: f32) -> vec4<f32> {
    var d = vec2<f32>(0.70710678, 0.70710678);
    if u.p0.x > 0.5 {
        d = vec2<f32>(1.0, 0.0);
    }
    let p = centered(uv);
    let ext = abs(d.x) * aspect() * 0.5 + abs(d.y) * 0.5;
    let r = 0.1;
    let sw = 0.12;
    let line = mix(ext + 0.01, -ext - r - sw - 0.01, tr_ease(t));
    let s = dot(p, d) - line;
    let paper = vec3<f32>(0.93, 0.92, 0.9);
    if s < 0.0 {
        // The part that wrapped over lies flat on top, back side up.
        let back = tr_uv(p + d * (PI * r - 2.0 * s));
        if tr_in01(back) {
            let c = sa(back);
            return vec4<f32>(mix(c.rgb, paper * c.a, 0.78), c.a);
        }
        return sa(uv);
    }
    if s <= r {
        let al = asin(clamp(s / r, 0.0, 1.0));
        // Upper half of the cylinder: the back of the page.
        let up = tr_uv(p + d * ((PI - al) * r - s));
        if tr_in01(up) {
            let c = sa(up);
            let k = 0.72 + 0.28 * cos(al);
            return vec4<f32>(mix(c.rgb, paper * c.a, 0.78) * k, c.a);
        }
        // Lower half: the front, turning away from the light.
        let low = tr_uv(p + d * (al * r - s));
        if tr_in01(low) {
            return tr_dim(sa(low), 1.0 - 0.45 * sin(al));
        }
    }
    let shadow = 0.5 * (1.0 - smoothstep(0.0, sw, s - r));
    return tr_dim(sb(uv), 1.0 - shadow);
}

// Card flip: A turns over and B is on its back. p0.x = 1 flips vertically.
fn tr_flip(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let vertical = u.p0.x > 0.5;
    var a = tr_ease(t) * PI;
    let show_b = a > PI * 0.5;
    if show_b {
        a = a - PI;
    }
    let o = vec3<f32>(0.0, 0.0, tr_bump(t) * 0.7);
    let h = tr_plane(
        centered(uv),
        o,
        tr_turn(vec3<f32>(1.0, 0.0, 0.0), a, vertical),
        tr_turn(vec3<f32>(0.0, 1.0, 0.0), a, vertical),
    );
    if !h.hit {
        return vec4<f32>(0.0);
    }
    if show_b {
        return tr_face(sb(h.uv), h.facing);
    }
    return tr_face(sa(h.uv), h.facing);
}

// Doors: A splits down the middle and both halves swing open towards the
// viewer while the camera moves in on B.
fn tr_door(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let p = centered(uv);
    let hw = aspect() * 0.5;
    // A door's free edge leaves the frame at atan(eye / half width).
    let a = mix(t, tr_ease(t), 0.5) * min(atan(TR_EYE / hw) + 0.1, PI * 0.5);
    let left = tr_plane(
        p,
        vec3<f32>(-hw, 0.0, 0.0) + tr_turn(vec3<f32>(hw, 0.0, 0.0), a, false),
        tr_turn(vec3<f32>(1.0, 0.0, 0.0), a, false),
        vec3<f32>(0.0, 1.0, 0.0),
    );
    var door = vec4<f32>(0.0);
    var found = false;
    if left.hit && left.uv.x <= 0.5 {
        door = sa(left.uv);
        if left.front {
            door = tr_face(door, left.facing);
        } else {
            door = tr_dim(door, 0.25);
        }
        found = true;
    }
    let right = tr_plane(
        p,
        vec3<f32>(hw, 0.0, 0.0) + tr_turn(vec3<f32>(-hw, 0.0, 0.0), -a, false),
        tr_turn(vec3<f32>(1.0, 0.0, 0.0), -a, false),
        vec3<f32>(0.0, 1.0, 0.0),
    );
    if !found && right.hit && right.uv.x >= 0.5 {
        door = sa(right.uv);
        if right.front {
            door = tr_face(door, right.facing);
        } else {
            door = tr_dim(door, 0.25);
        }
    }
    let k = mix(1.15, 1.0, tr_ease(t));
    let b = sb(vec2<f32>(0.5) + (uv - 0.5) / k) * min(t * 10.0, 1.0);
    return over(door, b);
}

// Fold: A folds up like a paper fan towards the left edge. p0.x = folds.
fn tr_fold(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = max(round(u.p0.x), 1.0);
    let a = tr_ease(t) * PI * 0.5;
    let width = cos(a);
    let b = sb(uv) * min(t * 10.0, 1.0);
    if uv.x >= width {
        return b;
    }
    let g = uv.x / max(width, 0.00001) * n;
    let k = floor(g);
    let f = g - k;
    let odd = (i32(k) & 1) == 1;
    // Alternate panels tilt away from the viewer, shrinking with depth.
    let depth = select(f, 1.0 - f, odd) * sin(a) * aspect() / n;
    let y = (uv.y - 0.5) * (TR_EYE + depth) / TR_EYE + 0.5;
    if y < 0.0 || y > 1.0 {
        return b;
    }
    let shade = select(1.0 - 0.2 * sin(a), 1.0 - 0.55 * sin(a), odd);
    let c = tr_dim(sa(vec2<f32>((k + f) / n, y)), shade);
    return over(c * (1.0 - smoothstep(0.85, 1.0, t)), b);
}

// --- Blurs --------------------------------------------------------------------

// Blur dissolve.
fn tr_blur(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let r = tr_bump(t) * 0.03;
    let j = hash12(uv * u.dst_size) * TAU;
    var a = vec4<f32>(0.0);
    var b = vec4<f32>(0.0);
    for (var i = 0; i < 16; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996 + j;
        let rad = sqrt((fi + 0.5) / 16.0) * r;
        let o = vec2<f32>(cos(ang) / aspect(), sin(ang)) * rad;
        a = a + sa(uv + o);
        b = b + sb(uv + o);
    }
    return mix(a, b, tr_ease(t)) / 16.0;
}

// Directional blur dissolve with a little drift. p0.x = angle in degrees.
fn tr_dir_blur(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let ang = radians(u.p0.x);
    let d = vec2<f32>(cos(ang) / aspect(), sin(ang));
    let len = 0.22 * tr_bump(t);
    let e = tr_ease(t);
    var a = vec4<f32>(0.0);
    var b = vec4<f32>(0.0);
    for (var i = 0; i < 20; i = i + 1) {
        let o = d * ((f32(i) / 19.0 - 0.5) * len);
        a = a + sa(uv + o + d * (0.1 * e));
        b = b + sb(uv + o - d * (0.1 * (1.0 - e)));
    }
    return mix(a, b, e) / 20.0;
}

// Defocus: the lens racks out of focus (breathing a little), bright spots
// open into discs, and B racks back in.
fn tr_defocus(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let r = 0.06 * s;
    let w = smoothstep(0.4, 0.6, t);
    let q = vec2<f32>(0.5) + (uv - vec2<f32>(0.5)) / (1.0 + 0.05 * s);
    // A random turn of the tap spiral per pixel trades ghost copies for fine grain.
    let j = hash12(uv * u.dst_size) * TAU;
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var i = 0; i < 40; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996 + j;
        let rad = sqrt((fi + 0.5) / 40.0) * r;
        let o = vec2<f32>(cos(ang) / aspect(), sin(ang)) * rad;
        let c = mix(sa(q + o), sb(q + o), w);
        let l = luma(c.rgb);
        let l3 = l * l * l;
        let wt = 1.0 + 14.0 * s * l3 * l;
        acc = acc + c * wt;
        wsum = wsum + wt;
    }
    let c = acc / wsum;
    return vec4<f32>(min(c.rgb * (1.0 + 0.2 * s), vec3<f32>(c.a)), c.a);
}

// --- Glitch -------------------------------------------------------------------

// Glitch cut.
fn tr_glitch(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = sin(t * PI);
    let band = floor(uv.y * 30.0);
    let k = floor(t * 20.0);
    var p = uv;
    p.x = p.x + (hash12(vec2<f32>(band, k)) - 0.5) * 0.2 * s * step(0.5, hash12(vec2<f32>(k, band)));
    let o = vec2<f32>(0.02 * s, 0.0);
    let pick = step(0.5, t + (hash12(vec2<f32>(band, k * 3.0)) - 0.5) * 0.3 * s);
    let a = vec4<f32>(sa(p + o).r, sa(p).g, sa(p - o).b, sa(p).a);
    let b = vec4<f32>(sb(p + o).r, sb(p).g, sb(p - o).b, sb(p).a);
    return mix(a, b, pick);
}

// RGB split: the color channels drift apart and cross over at different times.
fn tr_rgb_split(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let o = vec2<f32>(0.06 * s / aspect(), 0.012 * s);
    let c = vec2<f32>(0.5);
    let zr = 1.0 + 0.04 * s;
    let ur = c + (uv + o - c) / zr;
    let ub = c + (uv - o - c) * zr;
    let r = mix(sa(ur).r, sb(ur).r, smoothstep(0.2, 0.6, t));
    let g = mix(sa(uv), sb(uv), smoothstep(0.3, 0.7, t));
    let b = mix(sa(ub).b, sb(ub).b, smoothstep(0.4, 0.8, t));
    return vec4<f32>(r, g.g, b, g.a);
}

// Pixelate out and back in.
fn tr_pixelate(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = sin(t * PI);
    let cell = max(s * 0.06 * u.dst_size.y, 1.0);
    let p = (floor(uv * u.dst_size / cell) + 0.5) * cell / u.dst_size;
    return mix(sa(p), sb(p), smoothstep(0.4, 0.6, t));
}

// Digital noise: torn rows, blocks of static and scan lines while the
// blocks flip to B.
fn tr_digital(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let k = floor(t * 20.0 + u.time * 12.0);
    let row = floor(uv.y * 40.0);
    let tear = (hash12(vec2<f32>(row, k)) - 0.5) * 0.15 * s * select(0.0, 1.0, hash12(vec2<f32>(k, row * 1.3)) > 0.7);
    let q = vec2<f32>(uv.x + tear, uv.y);
    let blocks = vec2<f32>(aspect() * 12.0, 12.0);
    let cell = floor(q * blocks);
    let flip = 0.3 + 0.4 * hash12(cell + vec2<f32>(5.3, 1.7));
    var c = select(sa(q), sb(q), t > flip);
    let grain = hash12(floor(q * vec2<f32>(aspect(), 1.0) * 180.0) + vec2<f32>(k * 13.1, k * 7.7));
    let noisy = select(0.0, 1.0, hash12(cell * 1.31 + vec2<f32>(k * 0.37, k)) < 0.45 * s * s);
    let tint = vec3<f32>(grain * 0.85, grain, grain * 1.1);
    c = mix(c, vec4<f32>(min(tint, vec3<f32>(1.0)), 1.0), noisy * 0.9);
    let scan = 1.0 - 0.3 * s * step(0.5, fract(uv.y * 70.0));
    return tr_dim(c, scan);
}

// --- Shapes -------------------------------------------------------------------

// Circle opening from the centre.
fn tr_iris(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let p = centered(uv);
    let aa = tr_aa();
    let rmax = length(vec2<f32>(aspect() * 0.5, 0.5));
    let r = mix(-aa, rmax + aa, tr_ease(t));
    let m = smoothstep(r - aa, r + aa, length(p));
    return mix(sb(uv), sa(uv), m);
}

// Distance-like value for a unit shape, negative inside: 0 diamond,
// 1 star, 2 heart, 3 hexagon, other circle.
fn tr_shape_sdf(q: vec2<f32>, shape: u32) -> f32 {
    switch shape {
        case 0u: {
            return (abs(q.x) + abs(q.y) - 1.0) * 0.70710678;
        }
        case 1u: {
            let seg = TAU / 5.0;
            let ang = atan2(q.x, -q.y);
            let a = abs(ang - seg * round(ang / seg));
            let pt = length(q) * vec2<f32>(cos(a), sin(a));
            let tip = vec2<f32>(1.0, 0.0);
            let inner = 0.45 * vec2<f32>(cos(seg * 0.5), sin(seg * 0.5));
            let e = inner - tip;
            let nrm = normalize(vec2<f32>(e.y, -e.x));
            return dot(pt - tip, nrm);
        }
        case 2u: {
            // A square on its corner with a circle on each upper edge.
            let x = abs(q.x) / 1.25;
            let y = 0.07 - q.y / 1.25;
            let sq = (x + abs(y) - 0.70710678) * 0.70710678;
            let lobe = length(vec2<f32>(x, y) - vec2<f32>(0.35355339)) - 0.5;
            return min(sq, lobe) * 1.25;
        }
        case 3u: {
            let a = abs(q);
            return max(a.x * 0.8660254 + a.y * 0.5, a.y) - 1.0;
        }
        default: {
            return length(q) - 1.0;
        }
    }
}

// Smallest distance from the centre to the edge of each unit shape.
fn tr_shape_inradius(shape: u32) -> f32 {
    switch shape {
        case 0u: {
            return 0.70710678;
        }
        case 1u: {
            return 0.45;
        }
        case 2u: {
            return 0.68;
        }
        default: {
            return 1.0;
        }
    }
}

// Shape reveal: B appears inside a growing shape. p0.x = shape (see
// tr_shape_sdf), p0.y = turns while growing, p0.z = border width (picture
// heights) drawn in c0.
fn tr_shape(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let shape = u32(max(u.p0.x, 0.0) + 0.5);
    let aa = tr_aa();
    let rmax = length(vec2<f32>(aspect() * 0.5, 0.5));
    let bw = max(u.p0.z, 0.0) * tr_bump(t);
    let full = (rmax + 3.0 * aa) / tr_shape_inradius(shape) * 1.03;
    let e = tr_ease(t);
    let size = full * pow(e, 1.3);
    if size < 0.0001 {
        return sa(uv);
    }
    let p = centered(uv);
    let q = rot2(u.p0.y * TAU * (1.0 - e)) * p / size;
    let d = tr_shape_sdf(q, shape) * size;
    let grow = clamp(size / (6.0 * aa), 0.0, 1.0);
    let inside = (1.0 - smoothstep(-aa, aa, d)) * grow;
    var m = mix(sa(uv), sb(uv), inside);
    if bw > 0.0 {
        let ring = smoothstep(-aa, aa, d) * (1.0 - smoothstep(bw - aa, bw + aa, d)) * grow;
        m = mix(m, premul(vec4<f32>(u.c0.rgb, 1.0)), ring);
    }
    return m;
}

// --- Splits -------------------------------------------------------------------

// Doors: the old picture splits open from the middle. p0.x = 0 vertical, 1 horizontal.
fn tr_split(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    var x = uv.x;
    if u.p0.x > 0.5 {
        x = uv.y;
    }
    let open = e * 0.5;
    let b = sb(uv) * min(t * 10.0, 1.0);
    if abs(x - 0.5) < open {
        return b;
    }
    var shift = vec2<f32>(sign(x - 0.5) * open, 0.0);
    if u.p0.x > 0.5 {
        shift = vec2<f32>(0.0, sign(x - 0.5) * open);
    }
    // The inner edges turn darker as the halves part.
    let edge = 1.0 - smoothstep(0.0, 0.06, abs(x - 0.5) - open);
    let a = tr_dim(sa_clip(uv - shift), 1.0 - 0.35 * edge * min(t * 8.0, 1.0));
    return over(a, b);
}

// Four corners: A breaks into quarters that fly out to the corners.
fn tr_corners(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let e = tr_ease(t);
    let qd = select(vec2<f32>(-1.0), vec2<f32>(1.0), uv >= vec2<f32>(0.5));
    let src = uv - qd * (e * 0.5);
    let sq = select(vec2<f32>(-1.0), vec2<f32>(1.0), src >= vec2<f32>(0.5));
    var a = vec4<f32>(0.0);
    if all(sq == qd) && tr_in01(src) {
        a = sa(src);
    }
    let k = 1.0 - 0.25 * (1.0 - e);
    let b = sb(vec2<f32>(0.5) + (uv - 0.5) * k) * min(t * 10.0, 1.0);
    return over(a, b);
}

// Stripes: A's stripes slide out in alternating directions, B's follow
// them in. p0.x = number of stripes, p0.y = 1 for horizontal stripes.
fn tr_stripes(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = max(round(u.p0.x), 1.0);
    let horizontal = u.p0.y > 0.5;
    var x = uv.x;
    var y = uv.y;
    if horizontal {
        x = uv.y;
        y = uv.x;
    }
    let k = floor(x * n);
    let dir = select(-1.0, 1.0, (i32(k) & 1) == 1);
    let lt = tr_ease(clamp(t * 1.5 - (k / max(n - 1.0, 1.0)) * 0.5, 0.0, 1.0));
    let ya = y - dir * lt;
    let yb = y - dir * (lt - 1.0);
    var qa = vec2<f32>(uv.x, ya);
    var qb = vec2<f32>(uv.x, yb);
    if horizontal {
        qa = vec2<f32>(ya, uv.y);
        qb = vec2<f32>(yb, uv.y);
    }
    return sa_clip(qa) + sb_clip(qb);
}

// Mosaic: square tiles of A shrink and turn away one by one, uncovering B.
// p0.x = tiles per height.
fn tr_tiles(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = max(u.p0.x, 1.0);
    let g = vec2<f32>(uv.x * aspect(), uv.y) * n;
    let cell = floor(g);
    let f = g - cell - vec2<f32>(0.5);
    let delay = hash12(cell + vec2<f32>(3.1, 7.7)) * 0.55;
    let lt = clamp((t - delay) / 0.45, 0.0, 1.0);
    // Quick start so the gaps open at once, gentle finish.
    let e = 1.0 - (1.0 - lt) * (1.0 - lt);
    let k = 1.0 - e;
    if k < 0.0001 {
        return sb(uv);
    }
    // Turning only once the tile has shrunk keeps its corners inside the cell.
    let turn = select(-1.0, 1.0, hash12(cell + vec2<f32>(1.3, 4.9)) > 0.5) * e * e * 1.4;
    let q = rot2(turn) * f / k;
    let edge = max(abs(q.x), abs(q.y));
    // B, darkened by the soft shadow the lifting tile casts.
    let shadow = (1.0 - smoothstep(0.5, 0.5 + 0.12 / k, edge)) * 0.5 * min(e * 6.0, 1.0) * k;
    let b = tr_dim(sb(uv), 1.0 - shadow) * min(t * 10.0, 1.0);
    if edge > 0.5 {
        return b;
    }
    let src = vec2<f32>((cell.x + 0.5 + q.x) / n / aspect(), (cell.y + 0.5 + q.y) / n);
    return over(tr_dim(sa_clip(src), 1.0 - 0.35 * e), b);
}

// --- Warps --------------------------------------------------------------------

// Ripple: rings spread from the centre while the pictures cross.
fn tr_ripple(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let p = centered(uv);
    let r = length(p);
    let wave = sin(r * 45.0 - t * 40.0);
    let dirv = p / max(r, 0.0001);
    let amp = 0.025 * s * smoothstep(0.0, 0.15, r);
    let q = uv + tr_uv(dirv * wave * amp) - vec2<f32>(0.5);
    let c = mix(sa(q), sb(q), smoothstep(0.3, 0.7, t));
    return tr_dim(c, 1.0 + 0.12 * s * wave);
}

// Swirl into the new picture.
fn tr_swirl(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = sin(t * PI);
    var p = centered(uv);
    let r = length(p);
    let ang = s * 6.0 * max(0.8 - r, 0.0);
    p = rot2(ang) * p;
    let q = vec2<f32>(p.x / aspect() + 0.5, p.y + 0.5);
    return mix(sa(q), sb(q), smoothstep(0.3, 0.7, t));
}

// Liquid: both pictures flow along a noise field while B seeps through A.
fn tr_liquid(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let pq = vec2<f32>(uv.x * aspect(), uv.y) * 1.6;
    let flow = vec2<f32>(fbm(pq + vec2<f32>(t * 0.8, 0.0)), fbm(pq + vec2<f32>(5.2, 1.3 - t * 0.8))) - vec2<f32>(0.5);
    let e = tr_ease(t);
    let qa = tr_mirror(uv + flow * (0.6 * e));
    let qb = tr_mirror(uv - flow * (0.6 * (1.0 - e)));
    let n = fbm(pq * 0.8 + vec2<f32>(9.0, 4.0));
    let m = tr_sweep(n, t, 0.25);
    return mix(sb(qb), sa(qa), m);
}

// Dreamy: wavy, hazy and glowing in the middle.
fn tr_dreamy(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let q = vec2<f32>(uv.x + sin(uv.y * 16.0 + t * 10.0) * 0.02 * s, uv.y);
    let w = smoothstep(0.25, 0.75, t);
    let r = 0.025 * s;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 10; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let rad = sqrt((fi + 0.5) / 10.0) * r;
        let o = vec2<f32>(cos(ang) / aspect(), sin(ang)) * rad;
        acc = acc + mix(sa(q + o), sb(q + o), w);
    }
    let c = acc / 10.0;
    let glow = c.rgb + c.rgb * c.rgb * (0.6 * s);
    let haze = mix(glow, vec3<f32>(1.0, 0.96, 0.98) * c.a, 0.28 * s);
    return vec4<f32>(min(haze, vec3<f32>(1.0)), c.a);
}

// --- Light --------------------------------------------------------------------

// Film burn: hot orange edges eat the picture.
fn tr_burn(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let n = fbm(uv * vec2<f32>(aspect(), 1.0) * 3.0 + vec2<f32>(t * 0.5, 0.0));
    let edge = n * 1.3 - 0.15;
    let e = mix(-0.3, 1.3, t);
    let m = smoothstep(e - 0.12, e, edge);
    let glow = (1.0 - smoothstep(0.0, 0.12, abs(edge - e + 0.06))) * tr_bump(t);
    return tr_add(mix(sb(uv), sa(uv), m), vec3<f32>(1.0, 0.55, 0.15) * glow * 1.4);
}

// Bright flash in between.
fn tr_flash(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let base = mix(sa(uv), sb(uv), smoothstep(0.45, 0.55, t));
    let f = pow(tr_bump(t), 3.0);
    return mix(base, vec4<f32>(1.0), f);
}

// Light leak: soft glows in c0 sweep across while the pictures cross.
// p0.x = path (0, 1 or 2).
fn tr_leak(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let base = mix(sa(uv), sb(uv), smoothstep(0.3, 0.7, t));
    let path = u32(max(u.p0.x, 0.0) + 0.5);
    let p = centered(uv);
    let asp = aspect();
    var c1 = vec2<f32>(mix(-0.9, 0.5, t) * asp, -0.15);
    var c2 = vec2<f32>(mix(0.9, -0.2, t) * asp * 0.6, 0.35);
    var dir = normalize(vec2<f32>(1.0, -0.35));
    if path == 1u {
        c1 = vec2<f32>(0.55 * asp, mix(-0.9, 0.4, t));
        c2 = vec2<f32>(mix(-0.7, 0.1, t) * asp, mix(0.6, -0.1, t));
        dir = normalize(vec2<f32>(0.4, 1.0));
    } else if path == 2u {
        c1 = vec2<f32>(mix(0.9, -0.4, t) * asp, mix(0.5, -0.2, t));
        c2 = vec2<f32>(mix(-0.8, 0.2, t) * asp, -0.45);
        dir = normalize(vec2<f32>(1.0, 0.6));
    }
    let d1 = p - c1;
    let g1 = exp(-dot(d1, d1) / 0.22);
    // A long streak through the second glow.
    let d2 = p - c2;
    let along = dot(d2, dir);
    let across = dot(d2, vec2<f32>(-dir.y, dir.x));
    let g2 = exp(-across * across / 0.03 - along * along / 0.5);
    let i = (g1 + 0.8 * g2) * s;
    let light = u.c0.rgb * i * 1.3 + vec3<f32>(1.0, 0.95, 0.85) * i * i * 0.7;
    return tr_screen(base, light);
}

// Lens flare: a light source sweeps across with a streak and ghosts.
// c0 tints the streak.
fn tr_flare(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let s = tr_bump(t);
    let base = mix(sa(uv), sb(uv), smoothstep(0.4, 0.6, t));
    let p = centered(uv);
    let asp = aspect();
    let src = vec2<f32>(mix(-0.8, 0.8, t) * asp * 0.6, mix(-0.28, -0.12, t));
    let d = p - src;
    let dist = length(d);
    let core = exp(-dist * dist / 0.0015) + exp(-dist * 9.0) * 0.5;
    let streak = exp(-abs(d.y) * 90.0) * exp(-abs(d.x) * 1.4);
    var ghosts = vec3<f32>(0.0);
    for (var i = 0; i < 4; i = i + 1) {
        let fi = f32(i);
        let k = -0.35 - fi * 0.45;
        let gp = src * k;
        let rad = 0.03 + 0.03 * hash11(fi * 3.7 + 1.0);
        let gd = length(p - gp);
        let disk = (1.0 - smoothstep(rad * 0.7, rad, gd)) * 0.18 + (1.0 - smoothstep(0.0, rad * 0.25, abs(gd - rad))) * 0.12;
        let tint = vec3<f32>(0.5 + 0.5 * sin(fi * 2.1 + 0.3), 0.5 + 0.5 * sin(fi * 2.1 + 2.2), 0.6 + 0.4 * sin(fi * 2.1 + 4.1));
        ghosts = ghosts + tint * disk;
    }
    let light = (vec3<f32>(1.0, 0.96, 0.88) * core + u.c0.rgb * streak * 1.2 + ghosts + vec3<f32>(0.12)) * s * 1.3;
    return tr_screen(base, light);
}

// Light wipe: a bright soft band sweeps across and leaves the new picture
// behind it. p0.x = direction, c0 = light color.
fn tr_light_wipe(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let c = dot(uv - vec2<f32>(0.5), d) + 0.5;
    let pos = mix(-0.2, 1.2, tr_ease(t));
    let behind = smoothstep(pos + 0.02, pos - 0.02, c);
    let base = mix(sa(uv), sb(uv), behind);
    let g = exp(-pow((c - pos) / 0.05, 2.0));
    let on = smoothstep(0.0, 0.12, t) * smoothstep(1.0, 0.88, t);
    return tr_add(base, u.c0.rgb * (g * 0.75 + g * g * 0.5) * on);
}

// Film roll: both pictures roll through the frame like a strip of film, a
// dark gap between them, smeared while they move. p0.x = direction.
fn tr_film_roll(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let gap = 0.06;
    let s = tr_ease3(t) * (1.0 + gap);
    let len = 0.06 * tr_bump(t);
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 12; i = i + 1) {
        let o = ((f32(i) + 0.5) / 12.0 - 0.5) * len;
        let a = uv - d * (s + o);
        let b = uv + d * (1.0 + gap - s - o);
        if tr_in01(a) {
            acc = acc + sa(a);
        } else if tr_in01(b) {
            acc = acc + sb(b);
        } else {
            acc = acc + vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }
    return acc / 12.0;
}

// Zoom punch: the old picture is punched towards the viewer with a radial
// smear and split colors, the new one snaps back from just as close.
fn tr_zoom_punch(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let c = vec2<f32>(0.5);
    let h = clamp(t * 2.0, 0.0, 1.0);
    let h2 = clamp(t * 2.0 - 1.0, 0.0, 1.0);
    var scale = 1.0 + 0.5 * h * h;
    if t >= 0.5 {
        scale = 1.0 + 0.5 * (1.0 - h2) * (1.0 - h2);
    }
    let bump = pow(tr_bump(t), 1.5);
    let blur = 0.2 * bump;
    let which = smoothstep(0.47, 0.53, t);
    var rgb = vec3<f32>(0.0);
    var alpha = 0.0;
    for (var i = 0; i < 14; i = i + 1) {
        let k = f32(i) / 13.0;
        let z = scale * (1.0 + blur * k);
        let pr = c + (uv - c) / (z * (1.0 + 0.02 * bump));
        let pg = c + (uv - c) / z;
        let pb = c + (uv - c) / (z * (1.0 - 0.02 * bump));
        let r = mix(sa(pr), sb(pr), which);
        let g = mix(sa(pg), sb(pg), which);
        let b = mix(sa(pb), sb(pb), which);
        rgb = rgb + vec3<f32>(r.r, g.g, b.b);
        alpha = alpha + g.a;
    }
    return vec4<f32>(rgb / 14.0, alpha / 14.0);
}

// Shadow fade: the picture sinks into darkness from its edges and the next
// one rises out of it.
fn tr_shadow_fade(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let base = mix(sa(uv), sb(uv), smoothstep(0.46, 0.54, t));
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0);
    let r = length(p) / (0.5 * sqrt(aspect() * aspect() + 1.0));
    let k = clamp(pow(tr_bump(t), 2.2) * (1.0 + 0.9 * r), 0.0, 1.0);
    return vec4<f32>(base.rgb * (1.0 - k), base.a);
}

// --- Motion graphics -----------------------------------------------------------

// Reveal bars: the frame is cut into bars across the direction of travel.
// Each bar sweeps over the old picture carrying the new one, which drifts in
// with a little parallax and is smeared by the speed. A thin accent line with
// a bright core rides the front, a soft shadow falls ahead of it, and later
// bars start a little after earlier ones so the whole thing cascades.
// p0 = (direction, number of bars), c0 = accent colour.
fn tr_mg_bars(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let perp = vec2<f32>(-d.y, d.x);
    let n = max(floor(u.p0.y + 0.5), 2.0);
    let c = dot(uv - vec2<f32>(0.5), d) + 0.5;
    let q = dot(uv - vec2<f32>(0.5), perp) + 0.5;
    let i = clamp(floor(q * n), 0.0, n - 1.0);
    let v = fract(q * n);
    let delay = (i / max(n - 1.0, 1.0)) * 0.42;
    let p = clamp((t - delay) / 0.58, 0.0, 1.0);
    let e = tr_ease3(p);
    let lead = mix(-0.08, 1.08, e);
    let aa = max(tr_aa(), 0.0005);
    // 1 where the old picture still shows, 0 where the bar has passed.
    let ahead = smoothstep(lead - aa, lead + aa, c);
    let live = smoothstep(0.0, 0.02, p) * smoothstep(1.0, 0.98, p);

    // Speed of this bar right now: the smear grows and fades with it.
    let speed = tr_bump(p) * live;
    let len = 0.1 * speed;
    // The new picture drifts in behind the front and settles exactly.
    let drift = d * (lead - c) * 0.18 * (1.0 - e);
    var a_sum = vec4<f32>(0.0);
    var b_sum = vec4<f32>(0.0);
    for (var k = 0; k < 6; k = k + 1) {
        let o = (f32(k) + 0.5) / 6.0 * len;
        a_sum = a_sum + sa(uv + d * o);
        b_sum = b_sum + sb(uv + drift - d * o);
    }
    let near_a = smoothstep(lead + 0.16, lead, c);
    let near_b = smoothstep(lead - 0.2, lead, c);
    let pic_a = mix(sa(uv), a_sum / 6.0, near_a * min(speed * 1.5, 1.0));
    let pic_b = mix(sb(uv + drift), b_sum / 6.0, near_b * min(speed * 1.5, 1.0));
    var m = mix(pic_b, pic_a, ahead);

    // A soft shadow falls on the old picture ahead of the front.
    let sh = (1.0 - smoothstep(0.0, 0.07, c - lead)) * ahead;
    m = tr_dim(m, 1.0 - 0.35 * live * sh);
    // A faint dark seam between neighbouring bars while they move.
    let seam = 1.0 - smoothstep(0.0, 0.012, min(v, 1.0 - v));
    m = tr_dim(m, 1.0 - 0.3 * seam * live * (1.0 - ahead));
    // The accent line, its trailing glow and the bright core on the front.
    let line = 1.0 - smoothstep(0.003, 0.011, abs(c - lead));
    let trail = exp(-max(lead - c, 0.0) / 0.05) * (1.0 - ahead);
    m = mix(m, vec4<f32>(u.c0.rgb, 1.0), line * live);
    m = tr_add(m, u.c0.rgb * 0.3 * trail * live);
    let g = exp(-pow((c - lead) / 0.006, 2.0));
    return tr_add(m, vec3<f32>(g * 0.6 * live));
}
// --- More motion graphics, brush, paper, dust, camera, luxury, glass, cards ---

// Position along the travel direction in 0..1.
fn tr_axis(uv: vec2<f32>, d: vec2<f32>) -> f32 {
    return dot(uv - vec2<f32>(0.5), d) + 0.5;
}

// One circle of the grid: how deep the point is inside it, and what it
// needs for shading.
struct Circ {
    depth: f32,
    e: f32,
    r: f32,
    l: vec2<f32>,
    cc: vec2<f32>,
}

fn tr_circ(g: vec2<f32>, p: vec2<f32>, asp: f32, size: f32, order: u32, style: u32, t: f32) -> Circ {
    let cc = (g + vec2<f32>(0.5)) * size - vec2<f32>(asp * 0.5, 0.5);
    let l = p - cc;
    let nx = clamp((cc.x + asp * 0.5) / asp, 0.0, 1.0);
    let ny = clamp(cc.y + 0.5, 0.0, 1.0);
    var delay = nx;
    if order == 1u {
        delay = length(vec2<f32>((nx - 0.5) * asp, ny - 0.5)) / (0.5 * sqrt(asp * asp + 1.0));
    } else if order == 2u {
        delay = hash12(g + 3.0);
    } else if order == 3u {
        delay = (nx + ny) * 0.5;
    }
    let pt = clamp((t - delay * 0.5) / 0.5, 0.0, 1.0);
    let e = tr_ease3(pt);
    let aa = max(tr_aa(), 0.0005);
    let full = 0.7072 * size + 2.0 * aa;
    var wob = 1.0;
    if style == 1u {
        let ang = atan2(l.y, l.x);
        let h = hash12(g + 7.0);
        wob = 1.0 + (0.1 * sin(3.0 * ang + h * TAU + t * 4.0) + 0.06 * sin(5.0 * ang - h * 9.0 - t * 3.0)) * (1.0 - e);
    }
    let r = e * full * wob;
    var depth = r - length(l);
    if e < 0.001 {
        depth = -1.0;
    }
    return Circ(depth, e, r, l, cc);
}

// Circles grow out of a grid, merge, and carry the new picture, with a ring
// of accent colour on the edge. Styles: 0 flat, 1 wobbly blobs, 2 glass
// balls that refract the new picture and catch a highlight.
// p0 = (order: 0 across, 1 from the centre, 2 scattered, 3 diagonal;
// columns; style), c0 = accent colour.
fn tr_mg_circles(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let order = u32(max(u.p0.x, 0.0) + 0.5);
    let cols = max(floor(u.p0.y + 0.5), 2.0);
    let style = u32(max(u.p0.z, 0.0) + 0.5);
    let size = asp / cols;
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let g0 = floor((p + vec2<f32>(asp * 0.5, 0.5)) / size);
    var best = tr_circ(g0, p, asp, size, order, style, t);
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            if i != 0 || j != 0 {
                let c = tr_circ(g0 + vec2<f32>(f32(i), f32(j)), p, asp, size, order, style, t);
                if c.depth > best.depth {
                    best = c;
                }
            }
        }
    }
    let aa = max(tr_aa(), 0.0005);
    let e = best.e;
    let inside = smoothstep(-aa, aa, best.depth) * smoothstep(0.0, 0.08, e);
    let cuv = tr_uv(best.cc);
    var uvb = cuv + (uv - cuv) * mix(0.8, 1.0, e);
    let sw = 1.0 - e;
    var pb = sb(uvb);
    var a_pic = sa(uv);
    var ring = 0.0;
    if style == 2u {
        let nrm = best.l / max(best.r, 0.0001);
        let z = sqrt(max(1.0 - dot(nrm, nrm), 0.0));
        let f = mix(1.0, -0.8, pow(1.0 - e, 1.5));
        uvb = tr_uv(best.cc + best.l * f);
        pb = sb(uvb);
        let spec = pow(max(dot(vec3<f32>(nrm, z), normalize(vec3<f32>(-0.45, -0.55, 0.7))), 0.0), 50.0);
        let rimk = pow(1.0 - z, 2.5);
        pb = tr_dim(pb, 1.0 - 0.35 * rimk * sw);
        pb = tr_add(pb, vec3<f32>(spec * sw * 0.9));
        let sh = 1.0 - smoothstep(best.r * 0.9, best.r * 1.4, length(best.l - vec2<f32>(size * 0.04, size * 0.07)));
        a_pic = tr_dim(a_pic, 1.0 - 0.4 * sh * sw * smoothstep(0.0, 0.1, e));
    } else {
        let live = smoothstep(0.0, 0.06, e) * (1.0 - smoothstep(0.9, 1.0, e));
        ring = (1.0 - smoothstep(0.0, aa * 2.5, abs(best.depth))) * live;
    }
    var m = mix(a_pic, pb, inside);
    m = mix(m, vec4<f32>(u.c0.rgb, 1.0), ring * 0.9);
    return m;
}
// Concentric rings, each swept open by a turning edge, alternating in
// direction. p0 = (1 outward / 0 inward, number of rings), c0 = accent.
fn tr_mg_rings(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let rmax = 0.5 * sqrt(asp * asp + 1.0);
    let n = max(floor(u.p0.y + 0.5), 2.0);
    let rr = clamp(length(p) / rmax, 0.0, 0.9999);
    let i = floor(rr * n);
    var order = i / max(n - 1.0, 1.0);
    if u.p0.x < 0.5 {
        order = 1.0 - order;
    }
    let pt = clamp((t - order * 0.4) / 0.6, 0.0, 1.0);
    let e = tr_ease3(pt);
    var af = fract(atan2(p.y, p.x) / TAU + 1.25);
    if (u32(i) % 2u) == 1u {
        af = 1.0 - af;
    }
    let aw = tr_sweep(af, e, 0.015);
    var m = mix(sb(uv), sa(uv), aw);
    let live = sqrt(tr_bump(pt));
    let bf = fract(rr * n);
    let wline = min(bf, 1.0 - bf) / n * rmax;
    let line = (1.0 - smoothstep(0.002, 0.006, wline)) * live;
    let tick = exp(-pow((af - e) / 0.012, 2.0)) * live * step(0.001, e);
    m = mix(m, vec4<f32>(u.c0.rgb, 1.0), clamp(line + tick, 0.0, 1.0) * 0.85);
    return m;
}

// A band of colour flows across with wavy edges, the new picture
// following right behind it. p0 = (direction), c0 = colour.
fn tr_mg_wave(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let perp = vec2<f32>(-d.y, d.x);
    let cn = tr_axis(uv, d);
    let q = dot((uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0), perp);
    let gap = 0.24;
    let e = tr_ease3(t);
    let lead = mix(-0.2, 1.2 + gap, e);
    let w1 = 0.035 * sin(q * 9.0 + t * 5.0) + 0.018 * sin(q * 21.0 - t * 8.0);
    let w2 = 0.035 * sin(q * 9.0 + t * 5.0 + 1.3) + 0.018 * sin(q * 19.0 - t * 7.0);
    let f1 = cn + w1;
    let f2 = cn + w2;
    let aa = max(tr_aa(), 0.0005);
    let a_w = smoothstep(lead - aa, lead + aa, f1);
    let b_w = 1.0 - smoothstep(lead - gap - aa, lead - gap + aa, f2);
    let band_k = clamp((f1 - (lead - gap)) / gap, 0.0, 1.0);
    var band = vec4<f32>(mix(u.c0.rgb * 0.7, min(u.c0.rgb * 1.2, vec3<f32>(1.0)), band_k), 1.0);
    band = tr_add(band, vec3<f32>(exp(-pow((f1 - lead) / 0.012, 2.0)) * 0.35));
    var a = sa(uv);
    a = tr_dim(a, 1.0 - 0.3 * exp(-max(f1 - lead, 0.0) / 0.05) * step(0.0, f1 - lead));
    let base = mix(band, a, a_w);
    return mix(base, sb(uv), b_w);
}

// Brush: strokes of a loaded brush paint the new picture over the old, each
// stroke leading with a rounded front, with bristle texture and a wet dark
// rim. p0 = (direction, bristle fineness, ink amount), c0 = ink colour.
fn tr_brush(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let perp = vec2<f32>(-d.y, d.x);
    let pa = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0);
    let cn = tr_axis(uv, d);
    let qq = dot(pa, perp);
    let across = abs(perp.x) * aspect() + abs(perp.y);
    let strokes = 6.0;
    let su = (qq / across + 0.5) * strokes;
    let sid = floor(su);
    let sv = fract(su);
    let lead_off = (hash11(sid * 7.3 + 1.0) - 0.5) * 0.3;
    let bulge = 0.12 * (1.0 - pow(abs(sv - 0.5) * 2.0, 2.0));
    let e = tr_ease3(t);
    let bristle = mix(30.0, 90.0, u.p0.y);
    let n1 = fbm(vec2<f32>(qq * bristle, cn * 3.0));
    let n2 = fbm(vec2<f32>(qq * 5.0 + 5.0, cn * 4.0));
    let f = cn + lead_off - bulge + (n1 - 0.5) * 0.12 + (n2 - 0.5) * 0.1;
    let lead = mix(-0.45, 1.35, e);
    let rev = 1.0 - smoothstep(lead - 0.006, lead + 0.006, f);
    let rim = smoothstep(lead - 0.06, lead - 0.004, f) * rev;
    var m = mix(sa(uv), sb(uv), rev);
    let ink = clamp(rim * (0.6 + 0.4 * n1) * u.p0.z, 0.0, 1.0);
    m = vec4<f32>(mix(m.rgb, u.c0.rgb * m.a, ink), m.a);
    return m;
}
// Paper tear: the old picture rips along a ragged line and the two halves
// slide away with a pale torn rim, a shadow falling on the new picture.
// p0 = (direction the leading half moves, ragged edge).
fn tr_tear(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let perp = vec2<f32>(-d.y, d.x);
    let cn = tr_axis(uv, d);
    let qs = dot((uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0), perp);
    let jag = mix(0.5, 1.4, u.p0.y);
    let ed = 0.5 + 0.04 * jag * (fbm(vec2<f32>(qs * 7.0, 3.0)) * 2.0 - 1.0) + 0.012 * jag * (fbm(vec2<f32>(qs * 31.0, 9.0)) * 2.0 - 1.0);
    let e = tr_ease3(t);
    let gate = smoothstep(0.0, 0.04, e);
    let off = e * 0.64;
    let c1 = cn - off;
    let c2 = cn + off;
    let paper = vec3<f32>(0.95, 0.93, 0.88);
    let fibre = 0.78 + 0.22 * hash12(floor(uv * u.dst_size / 1.5));
    if c1 >= ed && c1 <= 1.0 {
        var m = sa(uv - d * off);
        let rim = (1.0 - smoothstep(0.0, 0.014 * jag, c1 - ed)) * gate;
        m = vec4<f32>(mix(m.rgb, paper * fibre * m.a, rim * 0.95), m.a);
        return m;
    }
    if c2 <= ed && c2 >= 0.0 {
        var m = sa(uv + d * off);
        let rim = (1.0 - smoothstep(0.0, 0.014 * jag, ed - c2)) * gate;
        m = vec4<f32>(mix(m.rgb, paper * fibre * m.a, rim * 0.95), m.a);
        return m;
    }
    let dist = min(max(cn - (ed - off), 0.0), max((ed + off) - cn, 0.0));
    return tr_dim(sb(uv), 1.0 - 0.45 * exp(-dist / 0.05) * gate);
}

// Dust: the old picture crumbles into grains along a ragged front and the
// new one shows through. p0 = (direction, noise scale, grain size),
// c0 = colour of the glowing dust.
fn tr_dust(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let pa = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0);
    let cn = tr_axis(uv, d);
    let n = fbm(pa * mix(3.0, 8.0, u.p0.y) + 4.0);
    let f = cn * 0.55 + n * 0.45;
    let e = tr_ease(t);
    let thresh = mix(-0.14, 1.06, e);
    let x = (f - thresh) / 0.12;
    let a_part = clamp(x, 0.0, 1.0);
    let cell = floor(uv * u.dst_size / mix(1.5, 3.5, u.p0.z));
    let gr = hash12(cell + vec2<f32>(floor(t * 24.0) * 7.1, 0.0));
    let keep = step(gr, pow(a_part, 0.8));
    let uva = uv - d * (1.0 - a_part) * 0.05 * (0.5 + gr);
    let a_kept = sa(uva) * keep;
    var m = a_kept + sb(uv) * (1.0 - a_kept.a);
    let speck = (1.0 - a_part) * a_part * 4.0 * step(0.94, hash12(cell * 1.7 + 3.0));
    let edgeg = exp(-pow((x - 0.15) / 0.25, 2.0)) * 0.12 * smoothstep(1.0, 0.8, x) * smoothstep(0.0, 0.1, x);
    m = tr_add(m, u.c0.rgb * (speck * 0.8 + edgeg));
    return m;
}

// Camera shutter: the blades close to black on the old picture, then open
// on the new one. p0 = (number of blades).
fn tr_shutter(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let n = max(floor(u.p0.x + 0.5), 3.0);
    let seg = TAU / n;
    let half_diag = 0.5 * sqrt(asp * asp + 1.0);
    let rmax = half_diag * 1.06 / cos(seg * 0.5);
    let closing = t < 0.5;
    var rad = rmax * tr_ease(t * 2.0 - 1.0);
    if closing {
        rad = rmax * (1.0 - tr_ease(t * 2.0));
    }
    let a = atan2(p.y, p.x) + t * 1.2 + 3.0;
    let la = (a - seg * floor(a / seg)) - seg * 0.5;
    let sd = length(p) * cos(la) - rad * cos(seg * 0.5);
    let aa = max(tr_aa(), 0.0005);
    let open_w = 1.0 - smoothstep(-aa, aa, sd);
    var pic = sb(uv);
    if closing {
        pic = sa(uv);
    }
    let blade = vec3<f32>(0.03, 0.032, 0.04) + vec3<f32>(0.05) * (0.5 + 0.5 * cos(a * n)) * smoothstep(0.0, 0.25, sd);
    var m = mix(vec4<f32>(blade, 1.0), pic, open_w);
    m = tr_add(m, vec3<f32>(exp(-pow(sd / 0.004, 2.0)) * 0.5));
    return m;
}

// Speed lines: the old picture rushes at the viewer under streaks that burst
// from the centre, a flash hides the cut, and the new picture settles back.
// p0 = (style: 0 white lines, 1 colour lines, 2 dark lines and an inverted
// flash; number of lines), c0 = line colour.
fn tr_speed(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let style = u32(max(u.p0.x, 0.0) + 0.5);
    let asp = aspect();
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let first = t < 0.5;
    var k = 1.0 - tr_ease(t * 2.0 - 1.0);
    if first {
        k = tr_ease(t * 2.0);
    }
    let s = 1.0 + 0.55 * k;
    var acc = vec4<f32>(0.0);
    for (var i = 0; i < 10; i = i + 1) {
        let f = f32(i) / 9.0;
        let q = vec2<f32>(0.5) + (uv - vec2<f32>(0.5)) / mix(1.0, s, f);
        if first {
            acc = acc + sa(q);
        } else {
            acc = acc + sb(q);
        }
    }
    var m = acc / 10.0;
    let n = max(u.p0.y, 24.0);
    let idf = (atan2(p.y, p.x) / TAU + 0.5) * n;
    let id = floor(idf);
    let fr = fract(idf);
    let seed = id + floor(u.time * 14.0) * 17.0;
    let h = hash11(seed);
    let h2 = hash11(seed * 1.7 + 3.0);
    let width = mix(0.1, 0.45, h2);
    let shape = 1.0 - smoothstep(width * 0.5 - 0.05, width * 0.5 + 0.02, abs(fr - 0.5));
    let start = 0.12 + 0.25 * hash11(seed * 2.3);
    let prof = smoothstep(start, start + 0.3, length(p));
    let lines = step(0.4, h) * shape * prof * pow(tr_bump(t), 1.4);
    let flash = exp(-pow((t - 0.5) / 0.05, 2.0)) * 0.8;
    if style == 0u {
        m = tr_screen(m, vec3<f32>(lines * 0.9 + flash));
    } else if style == 1u {
        m = tr_screen(m, u.c0.rgb * lines * 0.95 + vec3<f32>(flash));
    } else {
        m = tr_dim(m, 1.0 - lines * 0.9);
        let neg = exp(-pow((t - 0.5) / 0.03, 2.0));
        m = vec4<f32>(mix(m.rgb, vec3<f32>(m.a) - m.rgb, neg * 0.9), m.a);
    }
    return m;
}

// Light sweep: a slanted band of metallic light crosses the frame, the new
// picture appearing behind it, glitter flickering inside the band.
// p0 = (direction, band width), c0 = metal colour.
fn tr_gold(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let perp = vec2<f32>(-d.y, d.x);
    let cn = tr_axis(uv, d);
    let q = dot((uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0), perp);
    let c = cn + q * 0.35;
    let e = tr_ease3(t);
    let lead = mix(-0.5, 1.5, e);
    let a_w = smoothstep(lead - 0.004, lead + 0.004, c);
    var m = mix(sb(uv), sa(uv), a_w);
    let w = mix(0.05, 0.12, u.p0.y);
    let core = exp(-pow((c - lead) / (w * 0.35), 2.0));
    let band = exp(-pow((c - lead) / w, 2.0));
    let col = mix(u.c0.rgb, vec3<f32>(1.0), core);
    let grain = step(0.965, hash12(floor(uv * u.dst_size / 2.5) + vec2<f32>(floor(u.time * 20.0) * 3.1, 0.0)));
    let near = exp(-abs(c - lead) / 0.07) * (1.0 - a_w);
    let live = smoothstep(0.0, 0.03, t) * smoothstep(1.0, 0.97, t);
    let light = col * band * 0.9 + vec3<f32>(core * 0.6) + col * grain * band * 2.0 + u.c0.rgb * near * 0.12;
    return tr_add(m, light * live);
}

// Photo freeze: a flash, the old picture becomes a framed photo that tilts
// over the dimmed new picture, then drops away. p0 = (frame: 0 white border,
// 1 instant-photo with a deep foot, 2 dark border; tilt direction).
fn tr_freeze(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let style = u32(max(u.p0.x, 0.0) + 0.5);
    let k1 = tr_ease(clamp(t / 0.18, 0.0, 1.0));
    let k3 = tr_ease3(clamp((t - 0.62) / 0.38, 0.0, 1.0));
    let s = 1.0 - 0.2 * k1;
    var tilt = 1.0;
    if u.p0.y > 0.5 {
        tilt = -1.0;
    }
    let rot = tilt * (0.07 * k1 * (1.0 + 0.2 * sin(t * 8.0)) + 0.25 * k3);
    let off = vec2<f32>(0.1 * k3 * tilt, 1.6 * k3);
    let bw = 0.035 * k1;
    var bwb = bw;
    if style == 1u {
        bwb = bw * 3.2;
    }
    let grow = 0.01 * (1.0 - k1);
    var p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0) - off;
    let cs = cos(rot);
    let sn = sin(rot);
    p = vec2<f32>(cs * p.x + sn * p.y, -sn * p.x + cs * p.y) / s;
    let y0 = (bwb - bw) * 0.5;
    let outer = fx_sd_box(p - vec2<f32>(0.0, y0), vec2<f32>(asp * 0.5 + bw + grow, 0.5 + (bw + bwb) * 0.5 + grow), 0.0) * s;
    let pic_box = fx_sd_box(p, vec2<f32>(asp * 0.5 + grow, 0.5 + grow), 0.0) * s;
    let aa = max(tr_aa(), 0.0005);
    let card = 1.0 - smoothstep(-aa, aa, outer);
    let on_pic = 1.0 - smoothstep(-aa, aa, pic_box);
    var border = vec3<f32>(0.97, 0.96, 0.94);
    if style == 2u {
        border = vec3<f32>(0.07, 0.07, 0.08);
    }
    let photo = sa(vec2<f32>(p.x / asp + 0.5, p.y + 0.5));
    let face = mix(vec4<f32>(border, 1.0), photo, on_pic);
    var bg = tr_dim(sb(uv), mix(0.25, 1.0, k3));
    let sh = exp(-max(outer + 0.02, 0.0) / 0.05) * 0.45 * k1 * (1.0 - k3);
    bg = tr_dim(bg, 1.0 - sh);
    var m = mix(bg, face, card);
    let flash = smoothstep(0.0, 0.04, t) * exp(-t / 0.06) * 0.8;
    return tr_screen(m, vec3<f32>(flash));
}

// Card push: the new picture slides in as a rounded card with a soft
// shadow while the old one recedes, and the card squares off as it settles.
// p0 = (direction, -, style: 0 slide, 1 rise from below).
fn tr_cardpush(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    var d = tr_dir(u.p0.x);
    let style = u32(max(u.p0.z, 0.0) + 0.5);
    if style == 1u {
        d = vec2<f32>(0.0, -1.0);
    }
    let e = 1.0 - pow(1.0 - t, 3.5);
    let sa_k = 1.0 - 0.07 * e;
    let off_a = -d * 0.05 * e;
    let uva = vec2<f32>(0.5) + (uv - vec2<f32>(0.5) - off_a) / sa_k;
    var a = sa_clip(uva);
    a = tr_dim(a, 1.0 - 0.4 * e);
    let sb_k = mix(0.9, 1.0, e);
    let cb = vec2<f32>(0.5) - d * (1.0 - e);
    let uvb = vec2<f32>(0.5) + (uv - cb) / sb_k;
    let pc = (uv - cb) * vec2<f32>(asp, 1.0);
    let rr = mix(0.07, 0.0, smoothstep(0.75, 1.0, e));
    let sd = fx_sd_box(pc, vec2<f32>(asp * 0.5, 0.5) * sb_k * mix(1.0, 1.015, e), rr);
    let aa = max(tr_aa(), 0.0005);
    let card = 1.0 - smoothstep(-aa, aa, sd);
    let sh = exp(-max(sd, 0.0) / 0.07) * 0.5 * smoothstep(0.0, 0.05, e) * (1.0 - smoothstep(0.9, 1.0, e));
    a = tr_dim(a, 1.0 - sh);
    return mix(a, sb(uvb), card);
}

// Frosted glass sweep: a pane of frosted glass crosses the frame; inside
// it both pictures are soft, and behind it the new one is sharp.
// p0 = (direction, pane width, frost amount).
fn tr_frost(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let d = tr_dir(u.p0.x);
    let cn = tr_axis(uv, d);
    let e = tr_ease3(t);
    let hw = mix(0.12, 0.3, u.p0.y);
    let lead = mix(-hw - 0.05, 1.0 + hw + 0.05, e);
    let side = smoothstep(lead - 0.003, lead + 0.003, cn);
    let sharp = mix(sb(uv), sa(uv), side);
    let inband = 1.0 - smoothstep(hw - 0.01, hw, abs(cn - lead));
    var ba = vec4<f32>(0.0);
    var bb = vec4<f32>(0.0);
    let rad = mix(0.012, 0.04, u.p0.z);
    for (var i = 0; i < 12; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996;
        let rr = sqrt((fi + 0.5) / 12.0);
        let o = vec2<f32>(cos(ang), sin(ang)) * rr * vec2<f32>(rad / asp, rad);
        ba = ba + sa(uv + o);
        bb = bb + sb(uv + o);
    }
    var frosted = mix(bb / 12.0, ba / 12.0, side);
    frosted = tr_add(frosted, vec3<f32>(0.07));
    var m = mix(sharp, frosted, inband);
    let edge = exp(-pow((abs(cn - lead) - hw) / 0.004, 2.0));
    return tr_add(m, vec3<f32>(edge * 0.35));
}

// Shatter: the old picture breaks into shards that turn and shrink away,
// starting at a point of impact. p0 = (shards across the height,
// impact x, impact y in 0..1).
fn tr_shatter(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let scale = max(u.p0.x, 3.0);
    let v = fx_voronoi(p * scale, 0.0);
    let h = hash22(v.id + 11.0);
    let site_h = v.site / scale;
    let imp = (vec2<f32>(u.p0.y, u.p0.z) - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let dn = clamp(length(site_h - imp) / (0.6 * sqrt(asp * asp + 1.0)), 0.0, 1.0);
    let pc = clamp((t - dn * 0.4) / 0.6, 0.0, 1.0);
    let kk = tr_ease(pc);
    let s = mix(1.0, 0.45, kk);
    let ang = (h.x - 0.5) * 2.2 * kk;
    let src = site_h + (rot2(ang) * (p - site_h)) / s;
    let vs = fx_voronoi(src * scale, 0.0);
    let inside = select(0.0, 1.0, all(vs.id == v.id));
    let aw = inside * (1.0 - smoothstep(0.55, 1.0, kk));
    var a = sa(tr_uv(src));
    a = tr_dim(a, 1.0 - 0.3 * kk);
    a = tr_add(a, vec3<f32>((1.0 - smoothstep(0.0, 0.06, vs.d2 - vs.d1)) * kk * 0.6));
    let a_kept = a * aw;
    return a_kept + sb(uv) * (1.0 - a_kept.a);
}

// Glass slabs: the picture is cut into strips that turn over one after
// another in 3D, the old picture on the front and the new one on the back,
// lifting towards the viewer as they go and catching an iridescent sheen.
// p0 = (number of strips, order: 0 along, 1 from the middle, 2 scattered,
// direction: 0 vertical strips, 1 horizontal strips).
fn tr_slabs(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let n = max(floor(u.p0.x + 0.5), 2.0);
    let order = u32(max(u.p0.y, 0.0) + 0.5);
    let horiz = u.p0.z > 0.5;
    let p = centered(uv);
    var extent = asp;
    var pos = p.x;
    if horiz {
        extent = 1.0;
        pos = p.y;
    }
    let sw = extent / n;
    let i0 = floor((pos + extent * 0.5) / sw);
    var out = vec4<f32>(0.0);
    var best = -1.0;
    var found = false;
    for (var j = -1; j <= 1; j = j + 1) {
        let ij = i0 + f32(j);
        if ij < 0.0 || ij >= n {
            continue;
        }
        let cj = -extent * 0.5 + (ij + 0.5) * sw;
        let k = ij / max(n - 1.0, 1.0);
        var delay = k;
        if order == 1u {
            delay = abs(k - 0.5) * 2.0;
        } else if order == 2u {
            delay = hash11(ij * 7.1 + 1.0);
        }
        let pt = clamp((t - delay * 0.4) / 0.6, 0.0, 1.0);
        let a = tr_ease3(pt) * PI;
        let show_b = a > PI * 0.5;
        var ang = a;
        if show_b {
            ang = a - PI;
        }
        let lift = tr_bump(pt);
        var o = vec3<f32>(cj, 0.0, -0.3 * lift);
        var ex = tr_turn(vec3<f32>(1.0, 0.0, 0.0), ang, false);
        var ey = tr_turn(vec3<f32>(0.0, 1.0, 0.0), ang, false);
        if horiz {
            o = vec3<f32>(0.0, cj, -0.3 * lift);
            ex = tr_turn(vec3<f32>(1.0, 0.0, 0.0), ang, true);
            ey = tr_turn(vec3<f32>(0.0, 1.0, 0.0), ang, true);
        }
        let h = tr_plane(p, o, ex, ey);
        let l = vec2<f32>((h.uv.x - 0.5) * asp, h.uv.y - 0.5);
        let m = 0.0015;
        var inside = abs(l.x) <= sw * 0.5 + m && abs(l.y) <= 0.5 + m;
        if horiz {
            inside = abs(l.y) <= sw * 0.5 + m && abs(l.x) <= asp * 0.5 + m;
        }
        if !inside || lift < best {
            continue;
        }
        best = lift;
        var src = vec2<f32>(cj + l.x, l.y);
        if horiz {
            src = vec2<f32>(l.x, cj + l.y);
        }
        let tex = tr_uv(src);
        var pic = sa(tex);
        if show_b {
            pic = sb(tex);
        }
        var c = tr_face(pic, h.facing);
        let irid = hsv2rgb(vec3<f32>(fract(l.x * 2.5 + l.y * 0.7 + ang * 0.8), 0.55, 1.0));
        let sheen = pow(1.0 - clamp(h.facing, 0.0, 1.0), 1.4) * 0.4 * smoothstep(0.0, 0.1, pt) * smoothstep(1.0, 0.9, pt);
        c = tr_add(c, irid * sheen);
        out = c;
        found = true;
    }
    if !found {
        // Behind the slabs: both pictures, dimmed, so the gaps are not holes.
        let back = mix(sa(uv), sb(uv), smoothstep(0.2, 0.8, t));
        return tr_dim(back, 0.32);
    }
    return out;
}
// Tile flip: a grid of square tiles turns over one after another in 3D,
// the old picture on the front and the new one on the back, with a lift and
// a light sheen as each one turns. p0 = (columns, order: 0 diagonal stairs,
// 1 from the middle, 2 scattered, 3 row by row, axis: 0 turns about the
// vertical, 1 about the horizontal).
fn tr_tileflip(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let cols = max(floor(u.p0.x + 0.5), 2.0);
    let order = u32(max(u.p0.y, 0.0) + 0.5);
    let about_x = u.p0.z > 0.5;
    let p = centered(uv);
    let size = asp / cols;
    let rows = ceil(1.0 / size);
    let g0 = floor((p + vec2<f32>(asp * 0.5, 0.5)) / size);
    var out = vec4<f32>(0.0);
    var best = -1.0;
    var found = false;
    for (var jy = -1; jy <= 1; jy = jy + 1) {
        for (var jx = -1; jx <= 1; jx = jx + 1) {
            let g = g0 + vec2<f32>(f32(jx), f32(jy));
            let cc = (g + vec2<f32>(0.5)) * size - vec2<f32>(asp * 0.5, 0.5);
            let fx = clamp(g.x / max(cols - 1.0, 1.0), 0.0, 1.0);
            let fy = clamp(g.y / max(rows - 1.0, 1.0), 0.0, 1.0);
            var delay = (fx + fy) * 0.5;
            if order == 1u {
                delay = length(vec2<f32>((fx - 0.5) * asp, fy - 0.5)) / (0.5 * sqrt(asp * asp + 1.0));
            } else if order == 2u {
                delay = hash12(g + 5.0);
            } else if order == 3u {
                delay = fy * 0.8 + 0.2 * hash12(g + 1.0);
            }
            let pt = clamp((t - delay * 0.45) / 0.55, 0.0, 1.0);
            let a = tr_ease3(pt) * PI;
            let show_b = a > PI * 0.5;
            var ang = a;
            if show_b {
                ang = a - PI;
            }
            let lift = tr_bump(pt);
            let o = vec3<f32>(cc.x, cc.y, -0.25 * lift);
            let ex = tr_turn(vec3<f32>(1.0, 0.0, 0.0), ang, about_x);
            let ey = tr_turn(vec3<f32>(0.0, 1.0, 0.0), ang, about_x);
            let h = tr_plane(p, o, ex, ey);
            let l = vec2<f32>((h.uv.x - 0.5) * asp, h.uv.y - 0.5);
            let m = 0.0015;
            if abs(l.x) > size * 0.5 + m || abs(l.y) > size * 0.5 + m || lift < best {
                continue;
            }
            best = lift;
            let tex = tr_uv(cc + l);
            var pic = sa(tex);
            if show_b {
                pic = sb(tex);
            }
            var c = tr_face(pic, h.facing);
            let irid = hsv2rgb(vec3<f32>(fract(l.x * 3.0 + l.y * 2.0 + ang * 0.7), 0.5, 1.0));
            let sheen = pow(1.0 - clamp(h.facing, 0.0, 1.0), 1.5) * 0.35 * smoothstep(0.0, 0.1, pt) * smoothstep(1.0, 0.9, pt);
            c = tr_add(c, irid * sheen);
            out = c;
            found = true;
        }
    }
    if !found {
        let back = mix(sa(uv), sb(uv), smoothstep(0.2, 0.8, t));
        return tr_dim(back, 0.3);
    }
    return out;
}
// Torus rings: a glossy ring swells from the middle, the new picture
// filling its hole, and further rings follow it out. The ring is lit like a
// real tube (a bright side, a soft shadow side, a highlight); it can be seen
// at an angle, turning slowly. Styles: 0 glossy, 1 neon, 2 glass that bends
// what lies behind it. p0 = (rings 1..3, tilt, style, thickness), c0 = colour.
fn tr_torus(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let n = clamp(floor(u.p0.x + 0.5), 1.0, 3.0);
    let tilt = clamp(u.p0.y, 0.0, 1.0);
    let style = u32(max(u.p0.z, 0.0) + 0.5);
    let thick = mix(0.035, 0.1, clamp(u.p0.w, 0.0, 1.0));
    var p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let ang = t * 1.4 * tilt;
    p = rot2(ang) * p;
    p.y = p.y / (1.0 - 0.35 * tilt);
    let rmax = 0.5 * sqrt(asp * asp + 1.0) / (1.0 - 0.35 * tilt) + thick * 3.0;
    let d = length(p);
    let radial = p / max(d, 0.0001);
    let aa = max(tr_aa(), 0.0005);
    let r0 = tr_ease3(t) * rmax;
    var m = mix(sb(uv), sa(uv), smoothstep(r0 - aa, r0 + aa, d));
    let light = normalize(vec3<f32>(-0.5, -0.6, 0.62));
    for (var k = 0; k < 3; k = k + 1) {
        if f32(k) >= n {
            break;
        }
        let tk = clamp((t - f32(k) * 0.1) / (1.0 - (n - 1.0) * 0.1), 0.0, 1.0);
        let ek = tr_ease3(tk);
        let rk = ek * rmax;
        let gate = smoothstep(0.0, 0.08, ek) * smoothstep(1.0, 0.92, ek);
        let wk = thick * gate * (1.0 - 0.18 * f32(k));
        if wk <= 0.0005 {
            continue;
        }
        let q = (d - rk) / wk;
        let edge = 1.0 - smoothstep(0.9, 1.0, abs(q));
        if edge <= 0.0 {
            continue;
        }
        let z = sqrt(max(1.0 - q * q, 0.0));
        let nrm = normalize(vec3<f32>(radial * q, z));
        if style == 0u {
            let diff = max(dot(nrm, light), 0.0);
            let spec = pow(max(dot(nrm, normalize(light + vec3<f32>(0.0, 0.0, 1.0))), 0.0), 36.0);
            let rim = pow(1.0 - z, 3.0) * 0.35;
            let col = u.c0.rgb * (0.28 + 0.8 * diff) + vec3<f32>(spec * 0.9 + rim);
            m = mix(m, vec4<f32>(min(col, vec3<f32>(1.0)), 1.0), edge);
        } else if style == 1u {
            let glow = exp(-q * q * 2.2);
            let core = pow(max(1.0 - abs(q), 0.0), 4.0);
            m = tr_add(m, u.c0.rgb * glow * 1.1 * gate + vec3<f32>(core * 0.9 * gate));
        } else {
            let off = radial * q * wk * 0.9;
            let behind = mix(sb(uv - off / vec2<f32>(asp, 1.0)), sa(uv + off / vec2<f32>(asp, 1.0)), step(rk, d));
            let spec = pow(max(dot(nrm, normalize(light + vec3<f32>(0.0, 0.0, 1.0))), 0.0), 30.0);
            var g = tr_dim(behind, 0.82);
            g = tr_add(g, u.c0.rgb * 0.12 * (1.0 - z) + vec3<f32>(spec * 0.8 + pow(1.0 - z, 3.0) * 0.3));
            m = mix(m, g, edge);
        }
    }
    return m;
}
// Cracking ice: the old picture frosts over and cracks spread from a point
// of impact, then the ice breaks into shards that turn and shrink away,
// showing the new picture. p0 = (shards across the height, impact x,
// impact y in 0..1), c0 = tint of the ice.
fn tr_ice(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let asp = aspect();
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let scale = max(u.p0.x, 3.0);
    let v = fx_voronoi(p * scale, 0.0);
    let h = hash22(v.id + 21.0);
    let site_h = v.site / scale;
    let imp = (vec2<f32>(u.p0.y, u.p0.z) - vec2<f32>(0.5)) * vec2<f32>(asp, 1.0);
    let reach = length(site_h - imp);
    let dn = clamp(reach / (0.6 * sqrt(asp * asp + 1.0)), 0.0, 1.0);
    // Cracks first (up to 0.4), then the break.
    let spread = clamp(t / 0.4, 0.0, 1.0) * 1.4 * sqrt(asp * asp + 1.0) * 0.5;
    let pc = clamp((t - 0.35 - dn * 0.25) / 0.4, 0.0, 1.0);
    let kk = tr_ease(pc);
    let s = mix(1.0, 0.5, kk);
    let ang = (h.x - 0.5) * 1.6 * kk;
    let src = site_h + (rot2(ang) * (p - site_h)) / s;
    let vs = fx_voronoi(src * scale, 0.0);
    let inside = select(0.0, 1.0, all(vs.id == v.id));
    let aw = inside * (1.0 - smoothstep(0.5, 1.0, kk));
    var a = sa(tr_uv(src));
    let frost = smoothstep(0.0, 0.35, t) * (1.0 - kk);
    let tint = u.c0.rgb;
    a = vec4<f32>(mix(a.rgb, a.rgb * 0.7 + tint * 0.32 * a.a, frost * 0.7), a.a);
    // A cheap texture of tiny ice crystals.
    let grain = fbm(p * 60.0) - 0.5;
    a = tr_add(a, tint * grain * 0.1 * frost);
    let line = 1.0 - smoothstep(0.0, 0.05, vs.d2 - vs.d1);
    let grown = smoothstep(spread, spread - 0.25, reach);
    let kept = step(h.y, 0.75);
    let crack = line * grown * kept * (1.0 - smoothstep(0.55, 1.0, kk));
    a = tr_add(a, vec3<f32>(crack * 0.55) + tint * crack * 0.3);
    a = tr_dim(a, 1.0 - 0.45 * crack * 0.5);
    // Shards catch the light as they turn.
    let edge = (1.0 - smoothstep(0.0, 0.06, vs.d2 - vs.d1)) * kk;
    a = tr_add(a, vec3<f32>(edge * 0.6));
    let a_kept = a * aw;
    return a_kept + sb(uv) * (1.0 - a_kept.a);
}

// Stepped blocks: columns of B rise like stair steps, each with a lit top
// edge, a shaded side where it stands taller than its neighbour, and a
// shadow falling on the old picture. p0 = (direction, steps, order:
// 0 ascending, 1 from the middle, 2 scattered).
fn tr_steps(uv: vec2<f32>, t: f32) -> vec4<f32> {
    let d = tr_dir(u.p0.x);
    let perp = vec2<f32>(-d.y, d.x);
    let n = max(floor(u.p0.y + 0.5), 2.0);
    let order = u32(max(u.p0.z, 0.0) + 0.5);
    let c = tr_axis(uv, d);
    let across = abs(perp.x) * aspect() + abs(perp.y);
    let qn = dot((uv - vec2<f32>(0.5)) * vec2<f32>(aspect(), 1.0), perp) / across + 0.5;
    let i = clamp(floor(qn * n), 0.0, n - 1.0);
    let vv = fract(qn * n);
    let lead_of = function_lead(i, n, order, t);
    let lead = lead_of;
    let lip = 0.03;
    let aa = max(tr_aa(), 0.0005);
    // Where B shows: behind the front of this column.
    let b_mask = 1.0 - smoothstep(lead - aa, lead + aa, c);
    var m = mix(sa(uv), sb(uv), b_mask);
    // The lit top face along the front.
    let top = (1.0 - smoothstep(0.0, lip, lead - c)) * b_mask * smoothstep(0.0, 0.06, t) * smoothstep(1.0, 0.94, t);
    m = tr_add(m, vec3<f32>(top * 0.22));
    // The shaded side where the neighbour stands lower.
    let side_w = 0.18;
    var shade = 0.0;
    let left_lead = function_lead(max(i - 1.0, 0.0), n, order, t);
    let right_lead = function_lead(min(i + 1.0, n - 1.0), n, order, t);
    let near_left = 1.0 - smoothstep(0.0, side_w / n, vv / n);
    let near_right = 1.0 - smoothstep(0.0, side_w / n, (1.0 - vv) / n);
    if c > left_lead && c < lead {
        shade = max(shade, near_left);
    }
    if c > right_lead && c < lead {
        shade = max(shade, near_right);
    }
    m = tr_dim(m, 1.0 - 0.38 * shade * b_mask);
    // A shadow on the old picture just ahead of the front.
    let sh = (1.0 - smoothstep(0.0, 0.07, c - lead)) * (1.0 - b_mask) * smoothstep(0.0, 0.06, t) * smoothstep(1.0, 0.94, t);
    m = tr_dim(m, 1.0 - 0.4 * sh);
    return m;
}

// Where the front of column `i` of `n` is at time t (0..1 across the frame).
fn function_lead(i: f32, n: f32, order: u32, t: f32) -> f32 {
    let k = i / max(n - 1.0, 1.0);
    var delay = k;
    if order == 1u {
        delay = abs(k - 0.5) * 2.0;
    } else if order == 2u {
        delay = hash11(i * 5.3 + 2.0);
    }
    let e = tr_ease3(clamp((t - delay * 0.45) / 0.55, 0.0, 1.0));
    return mix(-0.06, 1.08, e);
}
fn tr_dispatch_b(uv: vec2<f32>, t: f32) -> vec4<f32> {
    if t <= 0.0 {
        return sa(uv);
    }
    if t >= 1.0 {
        return sb(uv);
    }
    switch u.kind {
        case 49u: { return tr_mg_bars(uv, t); }
        case 50u: { return tr_mg_circles(uv, t); }
        case 51u: { return tr_mg_rings(uv, t); }
        case 52u: { return tr_mg_wave(uv, t); }
        case 53u: { return tr_brush(uv, t); }
        case 54u: { return tr_tear(uv, t); }
        case 55u: { return tr_dust(uv, t); }
        case 56u: { return tr_shutter(uv, t); }
        case 57u: { return tr_speed(uv, t); }
        case 58u: { return tr_gold(uv, t); }
        case 59u: { return tr_freeze(uv, t); }
        case 60u: { return tr_cardpush(uv, t); }
        case 61u: { return tr_frost(uv, t); }
        case 62u: { return tr_shatter(uv, t); }
        case 63u: { return tr_slabs(uv, t); }
        case 64u: { return tr_tileflip(uv, t); }
        case 65u: { return tr_torus(uv, t); }
        case 66u: { return tr_ice(uv, t); }
        case 67u: { return tr_steps(uv, t); }
        default: { return tr_dissolve(uv, t); }
    }
}

@fragment
fn fs_transition(in: VOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = clamp(u.progress, 0.0, 1.0);
    var m: vec4<f32>;
    if u.kind >= 49u {
        m = tr_dispatch_b(uv, t);
        return over(m, sc(uv));
    }
    switch u.kind {
        case 1u: { m = tr_dissolve(uv, t); }
        case 2u: { m = tr_dip(uv, t); }
        case 3u: { m = tr_wipe(uv, t); }
        case 4u: { m = tr_push(uv, t); }
        case 5u: { m = tr_cover(uv, t); }
        case 6u: { m = tr_iris(uv, t); }
        case 7u: { m = tr_clock(uv, t); }
        case 8u: { m = tr_cross_zoom(uv, t); }
        case 9u: { m = tr_zoom_in(uv, t); }
        case 10u: { m = tr_noise(uv, t); }
        case 11u: { m = tr_burn(uv, t); }
        case 12u: { m = tr_pixelate(uv, t); }
        case 13u: { m = tr_split(uv, t); }
        case 14u: { m = tr_glitch(uv, t); }
        case 15u: { m = tr_swirl(uv, t); }
        case 16u: { m = tr_flash(uv, t); }
        case 17u: { m = tr_blur(uv, t); }
        case 18u: { m = tr_luma(uv, t); }
        case 19u: { m = tr_additive(uv, t); }
        case 20u: { m = tr_barn(uv, t); }
        case 21u: { m = tr_blinds(uv, t); }
        case 22u: { m = tr_checker(uv, t); }
        case 23u: { m = tr_uncover(uv, t); }
        case 24u: { m = tr_whip(uv, t); }
        case 25u: { m = tr_zoom_out(uv, t); }
        case 26u: { m = tr_spin_zoom(uv, t); }
        case 27u: { m = tr_cube(uv, t); }
        case 28u: { m = tr_curl(uv, t); }
        case 29u: { m = tr_flip(uv, t); }
        case 30u: { m = tr_door(uv, t); }
        case 31u: { m = tr_fold(uv, t); }
        case 32u: { m = tr_dir_blur(uv, t); }
        case 33u: { m = tr_defocus(uv, t); }
        case 34u: { m = tr_rgb_split(uv, t); }
        case 35u: { m = tr_digital(uv, t); }
        case 36u: { m = tr_shape(uv, t); }
        case 37u: { m = tr_corners(uv, t); }
        case 38u: { m = tr_stripes(uv, t); }
        case 39u: { m = tr_tiles(uv, t); }
        case 40u: { m = tr_ripple(uv, t); }
        case 41u: { m = tr_liquid(uv, t); }
        case 42u: { m = tr_dreamy(uv, t); }
        case 43u: { m = tr_leak(uv, t); }
        case 44u: { m = tr_flare(uv, t); }
        case 45u: { m = tr_light_wipe(uv, t); }
        case 46u: { m = tr_film_roll(uv, t); }
        case 47u: { m = tr_zoom_punch(uv, t); }
        case 48u: { m = tr_shadow_fade(uv, t); }
        default: { m = tr_dissolve(uv, t); }
    }
    return over(m, sc(uv));
}
