"""Generate the Lockin logo SVGs from the designs on the logo canvas.

Usage: python3 -I gen_logo.py <space-grotesk-700.woff> <output-dir>

The wordmark text is converted to outlines (Space Grotesk Bold, shaped with
HarfBuzz so kerning matches the browser), so the SVGs need no font installed.
"""

import sys
from pathlib import Path

import uharfbuzz as hb
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

FONT_FILE, OUT = sys.argv[1], Path(sys.argv[2])

INK_ON_DARK = "#F2F2F3"
INK_ON_LIGHT = "#111113"
BG_DARK = "#0E0E10"
BG_DARK_EDGE = "#2A2A2F"
BG_LIGHT = "#F4F4F5"

# ---------------------------------------------------------------- the mark
# Locked book, viewBox 0 0 64 64: padlock shackle on top of an open book.


def book_mark(shackle_width=8, top=18, left_page=(30.5, 5, 57, 59.5), right_page=(33.5, 59, 57, 59.5),
              outer_bottom=(55, 55.5)):
    """SVG elements of the locked book in the 64x64 box (fill and stroke = currentColor)."""
    return (
            f'<path d="M20 34 V{top} A12 12 0 0 1 44 {top} V34" fill="none" stroke="currentColor" '
            f'stroke-width="{shackle_width}" stroke-linecap="round"/>'
            f'<path d="M{left_page[0]} 31 C23 27 14 26.5 {left_page[1]} 28.5 V{left_page[2]} '
            f'C14 {outer_bottom[0]} 23 {outer_bottom[1]} {left_page[0]} {left_page[3]} Z"/>'
            f'<path d="M{right_page[0]} 31 C41 27 50 26.5 {right_page[1]} 28.5 V{right_page[2]} '
            f'C50 {outer_bottom[0]} 41 {outer_bottom[1]} {right_page[0]} {right_page[3]} Z"/>'
        )


# The tray sizes get a heavier shackle and a wider gap between the pages.
MARK = book_mark()
MARK_SIZES = {
    48: book_mark(shackle_width=8.5),
    32: book_mark(shackle_width=9.5, left_page=(30, 5, 57, 59.5), right_page=(34, 59, 57, 59.5)),
    22: book_mark(shackle_width=10.5, top=17, left_page=(29.5, 4, 58, 60), right_page=(34.5, 60, 58, 60),
                  outer_bottom=(56, 56.5)),
}
# Ink bounds of the mark in its 64 box (shackle top 18-12-4 = 2, pages down to 59.5).
MARK_INK = (5, 2, 59, 59.5)

# ------------------------------------------------------------- the padlock O
PADLOCK_O = (
    '<path d="M19 36 V19 A13 13 0 0 1 45 19 V36" fill="none" stroke="currentColor" stroke-width="11" '
    'stroke-linecap="round"/>'
    '<circle cx="32" cy="56" r="23.5" fill="none" stroke="currentColor" stroke-width="15"/>'
    '<circle cx="32" cy="53" r="4.5"/>'
    '<rect x="30" y="55" width="4" height="8" rx="2"/>'
)

# -------------------------------------------------------------------- text
font = TTFont(FONT_FILE)
glyph_set = font.getGlyphSet()
UPM = font["head"].unitsPerEm
ASCENT = font["OS/2"].sTypoAscender  # USE_TYPO_METRICS is set in this font
DESCENT = -font["OS/2"].sTypoDescender
hb_face = hb.Face(Path(FONT_FILE).read_bytes()) if FONT_FILE.endswith(".ttf") else None


def shape(text):
    """Glyph names and advances (font units), with kerning, like a browser does."""
    if hb_face is None:
        # HarfBuzz wants sfnt data; save a decompressed copy of the woff.
        tmp = OUT / ".font.ttf"
        f = TTFont(FONT_FILE)
        f.flavor = None
        f.save(tmp)
        face = hb.Face(tmp.read_bytes())
        tmp.unlink()
    else:
        face = hb_face
    hb_font = hb.Font(face)
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(hb_font, buf, {"kern": True, "liga": True})
    order = font.getGlyphOrder()
    return [(order[i.codepoint], p.x_advance, p.x_offset) for i, p in zip(buf.glyph_infos, buf.glyph_positions)]


def text_paths(text, size, x, baseline, letter_spacing_em):
    """One SVG path for `text`, starting at x on the given baseline. Returns (d, end_x, bounds)."""
    scale = size / UPM
    pen = SVGPathPen(glyph_set, ntos=lambda v: f"{v:.2f}".rstrip("0").rstrip("."))
    bounds_pen = BoundsPen(glyph_set)
    for name, advance, offset in shape(text):
        transform = (scale, 0, 0, -scale, x + offset * scale, baseline)
        glyph_set[name].draw(TransformPen(pen, transform))
        glyph_set[name].draw(TransformPen(bounds_pen, transform))
        x += advance * scale + letter_spacing_em * size
    return pen.getCommands(), x, bounds_pen.bounds


def svg(view_box, body, title, width=None, height=None):
    vb = " ".join(f"{v:g}" for v in view_box)
    size = f' width="{width:g}" height="{height:g}"' if width else ""
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb}"{size} role="img">\n'
        f"<title>{title}</title>\n{body}\n</svg>\n"
    )


def rounded(v):
    return round(v, 2)


def write(name, content):
    path = OUT / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)
    print("wrote", path)


# ------------------------------------------------------- 1. mark on its own
for variant, ink in (("on-dark", INK_ON_DARK), ("on-light", INK_ON_LIGHT)):
    write(f"svg/lockin-mark-{variant}.svg",
          svg((0, 0, 64, 64), f'<g fill="{ink}" color="{ink}">{MARK}</g>', "Lockin"))

# -------------------------------------------- 2. locked book + "Lockin" text
# From the canvas: mark 128 px, gap 32 px, text 132 px Space Grotesk Bold,
# letter-spacing -0.04em, line-height 1, both centred vertically.
FONT_SIZE = 132
MARK_PX = 128
line_box = FONT_SIZE
baseline = (line_box - (ASCENT + DESCENT) / UPM * FONT_SIZE) / 2 + ASCENT / UPM * FONT_SIZE
mark_top = (line_box - MARK_PX) / 2
d, _, (tx0, ty0, tx1, ty1) = text_paths("Lockin", FONT_SIZE, MARK_PX + 32, baseline, -0.04)
s = MARK_PX / 64
ink = (MARK_INK[0] * s, min(mark_top + MARK_INK[1] * s, ty0), tx1, max(mark_top + MARK_INK[3] * s, ty1))
view = tuple(rounded(v) for v in (ink[0], ink[1], ink[2] - ink[0], ink[3] - ink[1]))
for variant, colour in (("on-dark", INK_ON_DARK), ("on-light", INK_ON_LIGHT)):
    body = (
        f'<g fill="{colour}" color="{colour}">\n'
        f'<g transform="translate(0 {mark_top:g}) scale({s:g})">{MARK}</g>\n'
        f'<path d="{d}"/>\n</g>'
    )
    write(f"svg/lockin-logo-{variant}.svg", svg(view, body, "Lockin"))

# ------------------------------------------------ 3. padlock-O wordmark (alt)
# From the canvas: 150 px text, "L", then the padlock as an inline SVG of
# 0.524em x 0.72em sitting on the baseline (margin 0.1em left, 0.035em right), then "ckin".
FONT_SIZE = 150
baseline = 200.0  # any value; the viewBox is fitted to the ink afterwards
d_l, x, (lx0, ly0, lx1, ly1) = text_paths("L", FONT_SIZE, 0, baseline, -0.04)
lock_w, lock_h = 0.524 * FONT_SIZE, 0.72 * FONT_SIZE
lock_x = x + 0.1 * FONT_SIZE
lock_y = baseline - lock_h
lock_scale = lock_w / 64  # viewBox 64 x 88, same aspect ratio as 0.524 x 0.72
d_rest, _, (rx0, ry0, rx1, ry1) = text_paths("ckin", FONT_SIZE, lock_x + lock_w + 0.035 * FONT_SIZE, baseline, -0.04)
# Padlock ink in its 64x88 box: shackle top 19-13-5.5 = 0.5, ring bottom 56+23.5+7.5 = 87.
top = min(ly0, ry0, lock_y + 0.5 * lock_scale)
bottom = max(ly1, ry1, lock_y + 87 * lock_scale)
view = tuple(rounded(v) for v in (lx0, top, rx1 - lx0, bottom - top))
for variant, colour in (("on-dark", INK_ON_DARK), ("on-light", INK_ON_LIGHT)):
    body = (
        f'<g fill="{colour}" color="{colour}">\n'
        f'<path d="{d_l}"/>\n'
        f'<g transform="translate({lock_x:.2f} {lock_y:.2f}) scale({lock_scale:.5f})">{PADLOCK_O}</g>\n'
        f'<path d="{d_rest}"/>\n</g>'
    )
    write(f"svg/lockin-wordmark-padlock-{variant}.svg", svg(view, body, "Lockin"))

# ------------------------------------------------------------- 4. app icons
# From the canvas: 180 px rounded square, radius 42, mark 112 px centred;
# the dark one has a 1 px inner edge so it doesn't vanish on dark panels.
# Drawn here on a 1024 grid.
N = 1024
k = N / 180
radius = 42 * k
mark_px = 112 * k
offset = (N - mark_px) / 2
for variant, bg, colour, edge in (("dark", BG_DARK, INK_ON_DARK, BG_DARK_EDGE),
                                  ("light", BG_LIGHT, INK_ON_LIGHT, None)):
    edge_w = 1 * k
    edge_el = (
        f'<rect x="{edge_w / 2:.2f}" y="{edge_w / 2:.2f}" width="{N - edge_w:.2f}" height="{N - edge_w:.2f}" '
        f'rx="{radius - edge_w / 2:.2f}" fill="none" stroke="{edge}" stroke-width="{edge_w:.2f}"/>\n'
        if edge else ""
    )
    body = (
        f'<rect width="{N}" height="{N}" rx="{radius:.2f}" fill="{bg}"/>\n{edge_el}'
        f'<g fill="{colour}" color="{colour}" transform="translate({offset:.2f} {offset:.2f}) '
        f'scale({mark_px / 64:.5f})">{MARK}</g>'
    )
    write(f"svg/lockin-icon-{variant}.svg", svg((0, 0, N, N), body, "Lockin"))

# ------------------------------------------------------------- 5. tray icons
for px, mark in MARK_SIZES.items():
    for variant, colour in (("on-dark", INK_ON_DARK), ("on-light", INK_ON_LIGHT)):
        write(f"svg/tray/lockin-tray-{px}-{variant}.svg",
              svg((0, 0, 64, 64), f'<g fill="{colour}" color="{colour}">{mark}</g>', "Lockin", px, px))

# The app's own copy: currentColor, so CSS decides the colour.
write("app/lockin-mark.svg", svg((0, 0, 64, 64), f'<g fill="currentColor">{MARK}</g>', "Lockin"))
