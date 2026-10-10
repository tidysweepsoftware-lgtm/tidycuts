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

@fragment
fn fs_transition(in: VOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = clamp(u.progress, 0.0, 1.0);
    var m: vec4<f32>;
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
