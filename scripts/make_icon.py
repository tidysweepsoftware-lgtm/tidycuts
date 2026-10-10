#!/usr/bin/env python3
"""Draws the TidyCuts app icon and writes every size the installers need.

A teal rounded square with a white play triangle that has been cut in two by
a diagonal slice. Needs Pillow (pip install pillow) and ImageMagick's
`convert` for the Windows .ico. Run it from the repository root:

    python3 scripts/make_icon.py

Writes packaging/icons/: tidycuts-1024.png (master), tidycuts-256.png (used
inside the app), tidycuts.ico (Windows) and tidycuts.iconset/ (macOS: the
build script turns it into an .icns with `iconutil`).
"""
import math
import os
import subprocess
from PIL import Image, ImageChops, ImageDraw, ImageFilter

OUT = "packaging/icons"
BIG = 2048  # drawn large, then shrunk, for smooth edges


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def gradient(size, top, bottom, angle=90):
    """A linear gradient filling a square, `top` -> `bottom`; `angle` is the
    direction it runs in, in degrees (90 = downwards)."""
    n = 256  # computed on a small grid and scaled up
    dx, dy = math.cos(math.radians(angle)), math.sin(math.radians(angle))
    ext = (abs(dx) + abs(dy)) / 2
    t = Image.new("L", (n, n))
    px = t.load()
    for j in range(n):
        for i in range(n):
            u = ((i + 0.5) / n - 0.5) * dx + ((j + 0.5) / n - 0.5) * dy
            px[i, j] = max(0, min(255, round((u / (2 * ext) + 0.5) * 255)))
    t = t.resize((size, size), Image.BILINEAR)
    return Image.composite(
        Image.new("RGB", (size, size), bottom), Image.new("RGB", (size, size), top), t
    )


def rounded_mask(size, radius):
    m = Image.new("L", (size, size), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, size - 1, size - 1], radius, fill=255)
    return m


def play_mask(size):
    """A play triangle with rounded corners, nudged right of the middle so it
    looks centred. Rounded by drawing a smaller triangle and thickening it
    with a round pen."""
    cx, cy = size * 0.515, size * 0.5
    rad = size * 0.045  # corner radius
    r = size * 0.245 - rad  # circumscribed radius of the inner triangle
    pts = [
        (cx + r, cy),
        (cx - r * 0.5, cy - r * math.sqrt(3) / 2),
        (cx - r * 0.5, cy + r * math.sqrt(3) / 2),
    ]
    m = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(m)
    d.polygon(pts, fill=255)
    for i in range(3):
        a, b = pts[i], pts[(i + 1) % 3]
        d.line([a, b], fill=255, width=round(rad * 2))
        d.ellipse([a[0] - rad, a[1] - rad, a[0] + rad, a[1] + rad], fill=255)
    return m


def slice_masks(size, gap, shift):
    """Two half-planes either side of a diagonal cut through the middle."""
    a = math.radians(-62)  # the cut leans like a slash
    nx, ny = math.cos(a + math.pi / 2), math.sin(a + math.pi / 2)
    side = Image.new("L", (size, size), 0)
    px = side.load()
    # Signed distance to the line through the centre, in a coarse grid that
    # is then scaled up (cheap, and the edge stays straight).
    step = 8
    small = Image.new("L", (size // step, size // step), 0)
    sp = small.load()
    for j in range(size // step):
        for i in range(size // step):
            d = ((i * step - size / 2) * nx + (j * step - size / 2) * ny)
            sp[i, j] = max(0, min(255, int(128 + d)))
    d = small.resize((size, size), Image.BILINEAR)
    lo = d.point(lambda v: 255 if v < 128 - gap / 2 else 0)
    hi = d.point(lambda v: 255 if v > 128 + gap / 2 else 0)
    # Shift each half away from the cut.
    lo = ImageChops.offset(lo, round(-nx * shift), round(-ny * shift))
    hi = ImageChops.offset(hi, round(nx * shift), round(ny * shift))
    return lo, hi


def draw():
    s = BIG
    bg = gradient(s, (0x3d, 0xe8, 0xc0), (0x14, 0x9a, 0x80), 60)
    # A soft light in the top-left corner.
    glow = Image.new("L", (s, s), 0)
    ImageDraw.Draw(glow).ellipse([-s * 0.2, -s * 0.35, s * 0.75, s * 0.5], fill=70)
    glow = glow.filter(ImageFilter.GaussianBlur(s * 0.08))
    bg = Image.composite(Image.new("RGB", (s, s), (255, 255, 255)), bg, glow)

    play = play_mask(s)
    lo, hi = slice_masks(s, gap=s * 0.045, shift=s * 0.01)
    left = ImageChops.multiply(play, lo)
    right = ImageChops.multiply(play, hi)
    shape = ImageChops.lighter(left, right)

    # A soft shadow under the mark.
    shadow = ImageChops.offset(shape, 0, round(s * 0.018)).filter(
        ImageFilter.GaussianBlur(s * 0.02)
    )
    shadow = shadow.point(lambda v: int(v * 0.33))
    img = Image.composite(Image.new("RGB", (s, s), (0x05, 0x40, 0x34)), bg, shadow)
    img = Image.composite(Image.new("RGB", (s, s), (255, 255, 255)), img, shape)
    # The two halves differ slightly in tone so the cut reads at small sizes.
    tint = Image.composite(Image.new("RGB", (s, s), (0xd8, 0xfb, 0xf0)), img, right)
    img = Image.composite(tint, img, right)

    # Rounded-square outline of the app tile, with a margin like other icons.
    margin = round(s * 0.05)
    tile = img.crop((0, 0, s, s))
    mask = rounded_mask(s - 2 * margin, round((s - 2 * margin) * 0.225))
    out = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    out.paste(tile.resize((s - 2 * margin, s - 2 * margin)).convert("RGBA"), (margin, margin), mask)
    return out


def main():
    os.makedirs(OUT, exist_ok=True)
    master = draw().resize((1024, 1024), Image.LANCZOS)
    master.save(f"{OUT}/tidycuts-1024.png")
    master.resize((256, 256), Image.LANCZOS).save(f"{OUT}/tidycuts-256.png")

    iconset = f"{OUT}/tidycuts.iconset"
    os.makedirs(iconset, exist_ok=True)
    for base in (16, 32, 128, 256, 512):
        master.resize((base, base), Image.LANCZOS).save(f"{iconset}/icon_{base}x{base}.png")
        master.resize((base * 2, base * 2), Image.LANCZOS).save(
            f"{iconset}/icon_{base}x{base}@2x.png"
        )

    sizes = [16, 24, 32, 48, 64, 128, 256]
    tmp = [f"{OUT}/.ico-{n}.png" for n in sizes]
    for n, path in zip(sizes, tmp):
        master.resize((n, n), Image.LANCZOS).save(path)
    subprocess.run(["convert", *tmp, f"{OUT}/tidycuts.ico"], check=True)
    for path in tmp:
        os.remove(path)
    print("Icons written to", OUT)


if __name__ == "__main__":
    main()
