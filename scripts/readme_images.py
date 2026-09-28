#!/usr/bin/env python3
"""Frames the TUI screens shown in the README.

The screens come from an ignored test that renders the TUI into a test
backend and writes each screen as ANSI text:

    PINOUT_README_SCREENS=target/readme-screens \\
        cargo test --example pinout readme_screens -- --ignored
    python3 scripts/readme_images.py target/readme-screens assets/tui

Every image follows the framed layout of the author's other projects: a
hand-drawn border, a title and subtitle top-left in an 8x8 bitmap font and a
wordmark bottom-right, here in the colors of pinout's board and pin types.

Needs Pillow and a monospace font with box drawing characters (Menlo on
macOS, DejaVu Sans Mono elsewhere).
"""

import math
import random
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

# Title and subtitle of every screen, in the characters of the bitmap font
SCREENS = {
    "front": ("Front view", "The diagram redrawn on every edit, with its legend"),
    "editor": ("Editor", "YAML with syntax highlighting and vim keys"),
    "pins": ("Pins", "Every pin with its labels in aligned columns"),
    "columns": ("Columns", "Choose the label columns to show"),
}
WORDMARK = "PINOUT"

# Everything is drawn at twice the size for sharp text on high density screens
SCALE = 2
MARGIN = 22 * SCALE
INSET = 24 * SCALE
TITLE_H = 76 * SCALE
FOOTER_H = 34 * SCALE
FONT_SIZE = 12 * SCALE
LINE_HEIGHT = 15 * SCALE

# Colors of the board and pad in the diagrams, and of the built-in pin types
CANVAS = (36, 44, 56)
BORDER = (255, 193, 7)
TITLE = (235, 235, 235)
SUBTITLE = (150, 158, 170)
WORDMARK_COLOR = (121, 188, 60)
PIN_TYPES = [
    (121, 188, 60),  # gpio
    (204, 50, 45),  # power
    (51, 51, 51),  # gnd
    (65, 178, 140),  # i2c
    (99, 113, 129),  # uart
    (119, 94, 232),  # spi
    (227, 128, 34),  # analog
]
TERMINAL_FG = (220, 220, 220)

# font8x8_basic by Daniel Hepper (public domain), glyphs used by the titles
FONT8X8 = {
    " ": [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    ",": [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x06],
    "-": [0x00, 0x00, 0x00, 0x3F, 0x00, 0x00, 0x00, 0x00],
    ".": [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00],
    "/": [0x60, 0x30, 0x18, 0x0C, 0x06, 0x03, 0x01, 0x00],
    "A": [0x0C, 0x1E, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x00],
    "B": [0x3F, 0x66, 0x66, 0x3E, 0x66, 0x66, 0x3F, 0x00],
    "C": [0x3C, 0x66, 0x03, 0x03, 0x03, 0x66, 0x3C, 0x00],
    "D": [0x1F, 0x36, 0x66, 0x66, 0x66, 0x36, 0x1F, 0x00],
    "E": [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x46, 0x7F, 0x00],
    "F": [0x7F, 0x46, 0x16, 0x1E, 0x16, 0x06, 0x0F, 0x00],
    "G": [0x3C, 0x66, 0x03, 0x03, 0x73, 0x66, 0x7C, 0x00],
    "H": [0x33, 0x33, 0x33, 0x3F, 0x33, 0x33, 0x33, 0x00],
    "I": [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    "J": [0x78, 0x30, 0x30, 0x30, 0x33, 0x33, 0x1E, 0x00],
    "K": [0x67, 0x66, 0x36, 0x1E, 0x36, 0x66, 0x67, 0x00],
    "L": [0x0F, 0x06, 0x06, 0x06, 0x46, 0x66, 0x7F, 0x00],
    "M": [0x63, 0x77, 0x7F, 0x7F, 0x6B, 0x63, 0x63, 0x00],
    "N": [0x63, 0x67, 0x6F, 0x7B, 0x73, 0x63, 0x63, 0x00],
    "O": [0x1C, 0x36, 0x63, 0x63, 0x63, 0x36, 0x1C, 0x00],
    "P": [0x3F, 0x66, 0x66, 0x3E, 0x06, 0x06, 0x0F, 0x00],
    "Q": [0x1E, 0x33, 0x33, 0x33, 0x3B, 0x1E, 0x38, 0x00],
    "R": [0x3F, 0x66, 0x66, 0x3E, 0x36, 0x66, 0x67, 0x00],
    "S": [0x1E, 0x33, 0x07, 0x0E, 0x38, 0x33, 0x1E, 0x00],
    "T": [0x3F, 0x2D, 0x0C, 0x0C, 0x0C, 0x0C, 0x1E, 0x00],
    "U": [0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x3F, 0x00],
    "V": [0x33, 0x33, 0x33, 0x33, 0x33, 0x1E, 0x0C, 0x00],
    "W": [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00],
    "X": [0x63, 0x63, 0x36, 0x1C, 0x1C, 0x36, 0x63, 0x00],
    "Y": [0x33, 0x33, 0x33, 0x1E, 0x0C, 0x0C, 0x1E, 0x00],
    "Z": [0x7F, 0x63, 0x31, 0x18, 0x4C, 0x66, 0x7F, 0x00],
}

MONOSPACE_FONTS = [
    ("/System/Library/Fonts/Menlo.ttc", 0, 1),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", 0, 0),
]


def load_fonts():
    for path, regular, bold in MONOSPACE_FONTS:
        if Path(path).exists():
            if path.endswith(".ttc"):
                return (
                    ImageFont.truetype(path, FONT_SIZE, index=regular),
                    ImageFont.truetype(path, FONT_SIZE, index=bold),
                )
            bold_path = path.replace("Mono.ttf", "Mono-Bold.ttf")
            return (
                ImageFont.truetype(path, FONT_SIZE),
                ImageFont.truetype(bold_path if Path(bold_path).exists() else path, FONT_SIZE),
            )
    sys.exit("no monospace font found, add one to MONOSPACE_FONTS")


def text_width(text, scale):
    return 0 if not text else len(text) * 9 * scale - scale


def bitmap_text(draw, text, x, y, scale, color):
    """Draws text in the 8x8 bitmap font, each font pixel scale wide."""
    for char in text.upper():
        for row, bits in enumerate(FONT8X8.get(char, FONT8X8[" "])):
            for col in range(8):
                if bits & (1 << col):
                    draw.rectangle(
                        [x + col * scale, y + row * scale,
                         x + (col + 1) * scale - 1, y + (row + 1) * scale - 1],
                        fill=color,
                    )
        x += 9 * scale


def rough_rectangle(image, box, color, width, seed):
    """A hand-drawn rectangle: every side bows a little and misses the corners."""
    rng = random.Random(seed)
    # Drawn at four times the size and scaled down for smooth edges
    factor = 4
    big = Image.new("RGBA", (image.width * factor, image.height * factor), (0, 0, 0, 0))
    draw = ImageDraw.Draw(big)
    x0, y0, x1, y1 = (v * factor for v in box)
    corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    jitter = 1.5 * factor * SCALE
    for (ax, ay), (bx, by) in zip(corners, corners[1:] + corners[:1]):
        ax, ay = ax + rng.uniform(-jitter, jitter), ay + rng.uniform(-jitter, jitter)
        bx, by = bx + rng.uniform(-jitter, jitter), by + rng.uniform(-jitter, jitter)
        length = math.hypot(bx - ax, by - ay)
        bow = rng.uniform(-1, 1) * min(length * 0.004, 4 * factor * SCALE)
        nx, ny = -(by - ay) / length, (bx - ax) / length
        points = []
        for step in range(65):
            t = step / 64
            wave = math.sin(t * math.pi) * bow + rng.uniform(-0.3, 0.3) * factor
            points.append((ax + (bx - ax) * t + nx * wave, ay + (by - ay) * t + ny * wave))
        draw.line(points, fill=color + (255,), width=int(width * factor), joint="curve")
    small = big.resize(image.size, Image.Resampling.LANCZOS)
    image.alpha_composite(small)


ANSI = re.compile(r"\x1b\[([0-9;]*)m")

# Box drawing characters by the cell edges they connect to (up, right, down,
# left). They are drawn as lines rather than glyphs so borders stay unbroken
# whatever the line height.
BOX = {
    "─": (0, 1, 0, 1), "│": (1, 0, 1, 0),
    "╭": (0, 1, 1, 0), "╮": (0, 0, 1, 1), "╰": (1, 1, 0, 0), "╯": (1, 0, 0, 1),
    "┌": (0, 1, 1, 0), "┐": (0, 0, 1, 1), "└": (1, 1, 0, 0), "┘": (1, 0, 0, 1),
    "├": (1, 1, 1, 0), "┤": (1, 0, 1, 1),
}


def box_char(draw, char, left, top, cell, color):
    up, right, down, left_edge = BOX[char]
    cx, cy = left + cell // 2, top + LINE_HEIGHT // 2
    half = SCALE // 2
    if up:
        draw.rectangle([cx - half, top, cx + half, cy], fill=color)
    if down:
        draw.rectangle([cx - half, cy, cx + half, top + LINE_HEIGHT - 1], fill=color)
    if left_edge:
        draw.rectangle([left, cy - half, cx, cy + half], fill=color)
    if right:
        draw.rectangle([cx, cy - half, left + cell - 1, cy + half], fill=color)


def render_terminal(path, fonts):
    """Renders an ANSI screen dump with truecolor codes into an image."""
    regular, bold = fonts
    cell = regular.getbbox("M")[2]
    lines = [l for l in path.read_text(encoding="utf-8").split("\n") if l]
    width = max(len(ANSI.sub("", l)) for l in lines)
    image = Image.new("RGB", (width * cell, len(lines) * LINE_HEIGHT), CANVAS)
    draw = ImageDraw.Draw(image)
    for y, line in enumerate(lines):
        fg, bg, is_bold, x, pos = TERMINAL_FG, None, False, 0, 0
        for match in list(ANSI.finditer(line)) + [None]:
            segment = line[pos:match.start()] if match else line[pos:]
            for char in segment:
                left, top = x * cell, y * LINE_HEIGHT
                if bg:
                    draw.rectangle([left, top, left + cell - 1, top + LINE_HEIGHT - 1], fill=bg)
                if char in BOX:
                    box_char(draw, char, left, top, cell, fg)
                else:
                    draw.text((left, top + SCALE), char, fill=fg, font=bold if is_bold else regular)
                x += 1
            if not match:
                break
            pos = match.end()
            codes = [int(c) for c in match.group(1).split(";") if c] or [0]
            i = 0
            while i < len(codes):
                code = codes[i]
                if code == 0:
                    fg, bg, is_bold = TERMINAL_FG, None, False
                elif code == 1:
                    is_bold = True
                elif code in (38, 48) and codes[i + 1] == 2:
                    color = tuple(codes[i + 2:i + 5])
                    fg, bg = (color, bg) if code == 38 else (fg, color)
                    i += 4
                i += 1
    return image


def frame(content, title, subtitle, seed):
    title_scale, subtitle_scale, wordmark_scale = 3 * SCALE, 1 * SCALE, 2 * SCALE
    content_width = max(content.width, text_width(title, title_scale),
                        text_width(subtitle, subtitle_scale))
    width = content_width + 2 * (MARGIN + INSET)
    height = MARGIN + TITLE_H + content.height + FOOTER_H + MARGIN
    image = Image.new("RGBA", (width, height), CANVAS + (255,))
    draw = ImageDraw.Draw(image)

    left = MARGIN + INSET
    bitmap_text(draw, title, left, MARGIN + 18 * SCALE, title_scale, TITLE)
    bitmap_text(draw, subtitle, left, MARGIN + 55 * SCALE, subtitle_scale, SUBTITLE)
    image.paste(content, (left + (content_width - content.width) // 2, MARGIN + TITLE_H))

    # Wordmark bottom-right, led by a strip of the pin type colors
    bottom = height - MARGIN - 28 * SCALE
    right = width - MARGIN - INSET
    bitmap_text(draw, WORDMARK, right - text_width(WORDMARK, wordmark_scale), bottom,
                wordmark_scale, WORDMARK_COLOR)
    swatch = 8 * wordmark_scale
    x = right - text_width(WORDMARK, wordmark_scale) - 6 * SCALE - len(PIN_TYPES) * (swatch // 2 + 2 * SCALE)
    for color in PIN_TYPES:
        draw.rectangle([x, bottom, x + swatch // 2, bottom + swatch - 1], fill=color)
        x += swatch // 2 + 2 * SCALE

    rough_rectangle(image, (MARGIN, MARGIN, width - MARGIN, height - MARGIN),
                    BORDER, 2.5 * SCALE, seed)
    return image.convert("RGB")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    screens, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    fonts = load_fonts()
    for seed, (name, (title, subtitle)) in enumerate(SCREENS.items()):
        content = render_terminal(screens / f"{name}.ansi", fonts)
        image = frame(content, title, subtitle, seed)
        path = out / f"{name}.png"
        image.save(path, optimize=True)
        print(f"wrote {path} ({image.width}x{image.height})")


if __name__ == "__main__":
    main()
