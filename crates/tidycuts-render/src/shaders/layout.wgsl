// Split-screen cells and audio visualizers.

// Signed distance to a four-cornered polygon (negative inside); a repeated
// corner makes a triangle. After Inigo Quilez.
fn sd_quad(p: vec2<f32>, q0: vec2<f32>, q1: vec2<f32>, q2: vec2<f32>, q3: vec2<f32>) -> f32 {
    var v = array<vec2<f32>, 4>(q0, q1, q2, q3);
    var d = dot(p - v[0], p - v[0]);
    var s = 1.0;
    var j = 3;
    for (var i = 0; i < 4; i = i + 1) {
        let e = v[j] - v[i];
        let w = p - v[i];
        let b = w - e * clamp(dot(w, e) / max(dot(e, e), 1e-6), 0.0, 1.0);
        d = min(d, dot(b, b));
        let c = vec3<bool>((p.y >= v[i].y), (p.y < v[j].y), (e.x * w.y > e.y * w.x));
        if all(c) || !any(c) {
            s = -s;
        }
        j = i;
    }
    return s * sqrt(d);
}

fn sd_round_box(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// A split-screen cell: A = the layer placed on its own clear canvas, C = the
// canvas below. A shows only inside the inner corners (rounded by p2.x);
// the rest of the outer corners is the border.
// p0 = outer corners 0 and 1, p1 = outer corners 2 and 3, m0 / m1 = the
// inner ones (destination pixels); p2 = (radius px, blend mode, opacity, 0);
// c0 = border color (straight alpha).
@fragment
fn fs_cell(in: VOut) -> @location(0) vec4<f32> {
    let p = in.uv * u.dst_size;
    let dst = sc(in.uv);
    let d_out = sd_quad(p, u.p0.xy, u.p0.zw, u.p1.xy, u.p1.zw);
    let outer = clamp(0.5 - d_out, 0.0, 1.0);
    if outer <= 0.0 {
        return dst;
    }
    let d_in = sd_quad(p, u.m0.xy, u.m0.zw, u.m1.xy, u.m1.zw) - u.p2.x;
    let inner = clamp(0.5 - d_in, 0.0, 1.0) * outer;
    let below = composite(u32(u.p2.y + 0.5), sa(in.uv) * inner, dst);
    let edge = max(outer - inner, 0.0) * u.c0.a * u.p2.z;
    return over(vec4<f32>(u.c0.rgb * edge, edge), below);
}

// --- Audio visualizers ---------------------------------------------------
// A = the levels, one texel per bar (red channel).
// p0 = (style, bar count, mirror, thickness 0..1); c0 / c1 = the colors
// (straight alpha). Styles follow tidycuts_core::VISUALIZER_STYLES:
// 0 bars, 1 blocks, 2 line, 3 wave, 4 circle, 5 ring, 6 dots.

fn vis_n() -> i32 {
    return max(i32(u.p0.y + 0.5), 1);
}

fn vis_level(i: i32) -> f32 {
    return la(vec2<i32>(clamp(i, 0, vis_n() - 1), 0)).r;
}

fn vis_level_wrap(i: i32) -> f32 {
    let n = vis_n();
    return la(vec2<i32>(((i % n) + n) % n, 0)).r;
}

// The level at `x` (0..1 across the bars), smooth between bars.
fn vis_curve(x: f32, wrap: bool) -> f32 {
    let n = f32(vis_n());
    var f = x * n - 0.5;
    if !wrap {
        f = clamp(f, 0.0, n - 1.0);
    }
    let i = i32(floor(f));
    let w = cubic_weights(f - floor(f));
    var l: vec4<f32>;
    if wrap {
        l = vec4<f32>(vis_level_wrap(i - 1), vis_level_wrap(i), vis_level_wrap(i + 1), vis_level_wrap(i + 2));
    } else {
        l = vec4<f32>(vis_level(i - 1), vis_level(i), vis_level(i + 1), vis_level(i + 2));
    }
    return clamp(dot(l, w), 0.0, 1.0);
}

// A flat translucent fill, premultiplied.
fn vis_color_fill(t: f32, a: f32) -> vec4<f32> {
    let c = mix(u.c0, u.c1, clamp(t, 0.0, 1.0));
    return vec4<f32>(c.rgb, 1.0) * (c.a * clamp(a, 0.0, 1.0));
}

fn vis_rgb(t: f32) -> vec4<f32> {
    return mix(u.c0, u.c1, clamp(t, 0.0, 1.0));
}

// Light that spreads out from a shape: strongest at its edge, fading with
// the distance `d` (px) from it. Premultiplied, with less alpha than light
// so it brightens what is below.
fn vis_halo(t: f32, d: f32, radius: f32, strength: f32) -> vec4<f32> {
    let c = vis_rgb(t);
    let g = strength * exp(-max(d, 0.0) / max(radius, 1.0));
    return vec4<f32>(c.rgb * g, g * 0.6) * c.a;
}

// A filled shape at signed distance `d` (px, negative inside) with a halo;
// `lift` moves its colour towards white (bright cores and tips).
fn vis_solid(t: f32, d: f32, radius: f32, strength: f32, lift: f32) -> vec4<f32> {
    let c = vis_rgb(t);
    let cover = clamp(0.5 - d, 0.0, 1.0);
    let rgb = mix(c.rgb, vec3<f32>(1.0), clamp(lift, 0.0, 1.0));
    return over(vec4<f32>(rgb, 1.0) * (c.a * cover), vis_halo(t, d, radius, strength));
}

// A neon line: `d` is the distance (px) from its centre.
fn vis_line(d: f32, width: f32, t: f32, strength: f32) -> vec4<f32> {
    return vis_solid(t, d - width * 0.5, max(width * 2.5, 3.0), strength, 0.55);
}

// Which way the colors run around a circle: the same on both sides, so
// there is no seam where the circle closes.
fn vis_around(a01: f32, mirror: bool) -> f32 {
    if mirror {
        return a01;
    }
    return abs(2.0 * a01 - 1.0);
}

@fragment
fn fs_visualizer(in: VOut) -> @location(0) vec4<f32> {
    let size = u.dst_size;
    let p = in.uv * size;
    let style = u32(u.p0.x + 0.5);
    let n = f32(vis_n());
    let mirror = u.p0.z > 0.5;
    let thick = clamp(u.p0.w, 0.05, 1.0);
    let floor_level = 0.02;
    switch style {
        // Bars and blocks: capsules (or LED segments) standing on a floor, with
        // floating peak caps, a halo and a reflection below.
        case 0u, 1u, 6u: {
            let margin = size.x * 0.05;
            let slot = (size.x - 2.0 * margin) / n;
            var base = size.y * 0.76;
            var full = size.y * 0.64;
            if mirror {
                base = size.y * 0.5;
                full = size.y * 0.42;
            }
            var py = p.y;
            var refl = 1.0;
            if !mirror && p.y > base {
                py = 2.0 * base - p.y;
                refl = 0.3 * clamp(1.0 - (p.y - base) / (full * 0.45), 0.0, 1.0);
            }
            let hgt = abs(py - base);
            let gi = (p.x - margin) / slot;
            let ic = i32(floor(gi));
            let bw = max(slot * thick, 2.0);
            var out = vec4<f32>(0.0);
            // Blocks and dots are cut into rows.
            var segs = clamp(round(full / max(slot * 0.62, 3.0)), 6.0, 40.0);
            if style == 6u {
                segs = max(floor(full / slot), 3.0);
            }
            let seg_h = full / segs;
            let kc = floor(hgt / seg_h);
            for (var di = -1; di <= 1; di = di + 1) {
                let i = ic + di;
                if i < 0 || i >= i32(n) {
                    continue;
                }
                let cx = margin + (f32(i) + 0.5) * slot;
                let lv = max(vis_level(i), floor_level);
                let xn = (f32(i) + 0.5) / n;
                let px = p.x - cx;
                if style == 0u {
                    var hh = lv * full * 0.5;
                    var cy = base - hh;
                    if mirror {
                        hh = lv * full;
                        cy = base;
                    }
                    let r = bw * 0.5;
                    let d = sd_round_box(vec2<f32>(px, py - cy), vec2<f32>(bw * 0.5, max(hh, r)), r);
                    let yn = clamp(hgt / full, 0.0, 1.0);
                    let t = 0.55 * xn + 0.45 * yn;
                    // A bright tip and a stripe of light down the left side.
                    let tip = clamp(1.0 - (lv * full - hgt) / (bw * 2.0), 0.0, 1.0);
                    let rel = (px + bw * 0.5) / bw;
                    let stripe = smoothstep(0.08, 0.28, rel) * (1.0 - smoothstep(0.34, 0.52, rel));
                    out = over(vis_solid(t, d, bw * 0.7 + 2.0, 0.22, 0.45 * tip + 0.22 * stripe), out);
                    // The peak cap floats just above the bar.
                    let cap_h = max(bw * 0.3, 2.5);
                    let cap_c = lv * full + max(bw * 0.55, 5.0) + cap_h * 0.5;
                    if cap_c < full * 1.03 && lv > 0.05 {
                        let dc = sd_round_box(vec2<f32>(px, hgt - cap_c), vec2<f32>(bw * 0.5, cap_h * 0.5), cap_h * 0.5);
                        out = over(vis_solid(min(t + 0.15, 1.0), dc, bw * 0.6 + 2.0, 0.25, 0.7), out);
                    }
                } else if style == 1u {
                    let on = (kc + 0.5) / segs <= lv;
                    let top = (kc + 1.0) / segs > lv && on;
                    let gap = max(seg_h * 0.14, 1.0);
                    let d = sd_round_box(vec2<f32>(px, hgt - (kc + 0.5) * seg_h), vec2<f32>(bw * 0.5, seg_h * 0.5 - gap), min(bw * 0.18, seg_h * 0.3));
                    let t = 0.4 * xn + 0.6 * (kc + 0.5) / segs;
                    if on {
                        out = over(vis_solid(t, d, seg_h * 0.8, 0.16, select(0.0, 0.5, top)), out);
                    } else {
                        let dim = clamp(0.5 - d, 0.0, 1.0) * 0.13;
                        let c = vis_rgb(t);
                        out = over(vec4<f32>(c.rgb, 1.0) * (c.a * dim), out);
                    }
                } else {
                    // Dots: round beads in rows, lit up to the level.
                    let on = (kc + 0.5) / segs <= lv;
                    let top = (kc + 1.0) / segs > lv && on;
                    let rr = seg_h * 0.5 * thick * select(0.5, 0.95, on);
                    let d = length(vec2<f32>(px, hgt - (kc + 0.5) * seg_h)) - rr;
                    let t = 0.5 * xn + 0.5 * (kc + 0.5) / segs;
                    if on {
                        out = over(vis_solid(t, d, rr * 1.0 + 1.5, 0.2, select(0.1, 0.65, top)), out);
                    } else {
                        let c = vis_rgb(t);
                        out = over(vec4<f32>(c.rgb, 1.0) * (c.a * clamp(0.5 - d, 0.0, 1.0) * 0.16), out);
                    }
                }
            }
            return out * refl;
        }
        // Line and wave: neon curves with fainter echoes behind, and under
        // the wave a fill that fades away.
        case 2u, 3u: {
            let margin = size.x * 0.04;
            let span = size.x - 2.0 * margin;
            let xr = (p.x - margin) / span;
            let edge = smoothstep(0.0, 0.03, xr) * (1.0 - smoothstep(0.97, 1.0, xr));
            let xc = clamp(xr, 0.0, 1.0);
            let width = max(2.0, thick * size.y * 0.03);
            var amp = size.y * 0.58;
            var base = size.y * 0.74;
            if mirror {
                amp = size.y * 0.4;
                base = size.y * 0.5;
            }
            var py = p.y;
            var refl = 1.0;
            if !mirror && p.y > base {
                py = 2.0 * base - p.y;
                refl = 0.3 * clamp(1.0 - (p.y - base) / (amp * 0.4), 0.0, 1.0);
            }
            let dx = 1.0 / span;
            var out = vec4<f32>(0.0);
            for (var layer = 2; layer >= 0; layer = layer - 1) {
                let s = 1.0 - 0.2 * f32(layer);
                let weight = select(select(0.2, 0.42, layer == 1), 1.0, layer == 0);
                let lv = max(vis_curve(xc, false), floor_level) * s;
                let slope = (vis_curve(xc + dx, false) - vis_curve(xc - dx, false)) * 0.5 * s;
                let stretch = sqrt(1.0 + slope * amp * slope * amp);
                let y_line = base - lv * amp;
                var d = abs(py - y_line) / stretch;
                if mirror {
                    d = min(d, abs(py - (2.0 * base - y_line)) / stretch);
                }
                if style == 3u && layer == 0 {
                    // Fill from the line down to the floor, fading out.
                    var below = py - y_line;
                    var depth = (base - y_line);
                    if mirror {
                        below = lv * amp - abs(py - base);
                        depth = lv * amp;
                    }
                    let inside = clamp(below + 0.5, 0.0, 1.0);
                    let fade = 1.0 - clamp(below / max(depth, 1.0), 0.0, 1.0);
                    let k = 0.2 + 0.8 * pow(fade, 1.3);
                    let t = 0.75 * xc + 0.25 * (1.0 - fade);
                    var in_floor = 1.0;
                    if !mirror {
                        in_floor = clamp(base - py + 0.5, 0.0, 1.0);
                    }
                    out = over(vis_color_fill(t, inside * in_floor * 0.85 * k), out);
                }
                out = over(vis_line(d, width * select(0.6, 1.0, layer == 0), xc, 0.5 * weight) * weight, out);
            }
            return out * (edge * refl);
        }
        // Circle and ring.
        default: {
            let c = size * 0.5;
            let r_max = min(size.x, size.y) * 0.5;
            let r0 = r_max * 0.44;
            let len = r_max * 0.52;
            let q = p - c;
            let rad = length(q);
            // 0 at the top, going clockwise.
            let ang = atan2(q.x, -q.y);
            var a01 = fract(ang / TAU + 1.0);
            var span = TAU;
            if mirror {
                a01 = abs(ang) / PI;
                span = PI;
            }
            let sym = vis_around(a01, mirror);
            // A soft glow inside and a neon ring at the base.
            var out = vec4<f32>(0.0);
            let haze = exp(-pow(rad / (r0 * 1.05), 2.0) * 2.2) * 0.2;
            let cc = vis_rgb(0.2);
            out = over(vec4<f32>(cc.rgb * haze, haze * 0.5) * cc.a, out);
            let ring_w = max(1.5, r_max * 0.012);
            out = over(vis_line(abs(rad - r0 * 0.93), ring_w, 0.1, 0.55), out);
            if style == 4u {
                let i0 = i32(floor(a01 * n));
                let half_w = max(r0 * span / n * thick * 0.5, 1.0);
                for (var di = -1; di <= 1; di = di + 1) {
                    let iu = i0 + di;
                    var i = iu;
                    if mirror {
                        if iu < 0 || iu >= i32(n) {
                            continue;
                        }
                    } else {
                        i = ((iu % i32(n)) + i32(n)) % i32(n);
                    }
                    let lv = max(vis_level(i), floor_level);
                    let da = (a01 - (f32(iu) + 0.5) / n) * span;
                    let across = rad * sin(da);
                    // The outward bar.
                    let along = rad - (r0 + lv * len * 0.5);
                    let d = sd_round_box(vec2<f32>(along, across), vec2<f32>(lv * len * 0.5, half_w), half_w);
                    let t = 0.5 * sym + 0.5 * clamp((rad - r0) / len, 0.0, 1.0);
                    let tip = clamp(1.0 - (r0 + lv * len - rad) / (half_w * 4.0), 0.0, 1.0);
                    out = over(vis_solid(t, d, half_w * 1.5 + 2.0, 0.26, 0.5 * tip), out);
                    // A shorter, fainter bar pointing inwards.
                    let il = lv * len * 0.3;
                    let along2 = rad - (r0 * 0.86 - il * 0.5);
                    let d2 = sd_round_box(vec2<f32>(along2, across), vec2<f32>(il * 0.5, half_w * 0.8), half_w * 0.8);
                    out = over(vis_solid(sym, d2, half_w * 1.5 + 2.0, 0.2, 0.0) * 0.55, out);
                }
            } else {
                let eps = 1.0 / max(n * 8.0, 64.0);
                for (var layer = 2; layer >= 0; layer = layer - 1) {
                    let s = 1.0 - 0.2 * f32(layer);
                    let weight = select(select(0.2, 0.42, layer == 1), 1.0, layer == 0);
                    let lv = max(vis_curve(a01, !mirror), floor_level) * s;
                    let slope = (vis_curve(a01 + eps, !mirror) - vis_curve(a01 - eps, !mirror)) / (2.0 * eps * span) * s;
                    let r_line = r0 + lv * len;
                    let stretch = sqrt(1.0 + (slope * len / max(rad, 1.0)) * (slope * len / max(rad, 1.0)));
                    // Near the centre the line's slope blows up; nothing is drawn there.
                    let d = select(1.0e4, abs(rad - r_line) / stretch, rad > r0 * 0.6);
                    let width = max(2.0, thick * r_max * 0.035) * select(0.6, 1.0, layer == 0);
                    if layer == 0 {
                        // The area between the ring and the line.
                        let k = clamp(r_line - rad + 0.5, 0.0, 1.0) * clamp(rad - r0 * 0.93 + 0.5, 0.0, 1.0);
                        let fade = clamp((rad - r0 * 0.93) / max(r_line - r0 * 0.93, 1.0), 0.0, 1.0);
                        out = over(vis_color_fill(sym, k * (0.12 + 0.4 * fade * fade)), out);
                    }
                    out = over(vis_line(d, width, sym, 0.5 * weight) * weight, out);
                }
            }
            return out;
        }
    }
}
