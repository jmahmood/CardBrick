#!/usr/bin/env python3
"""Render CardBrick boot-splash images as raw framebuffer dumps.

Python + pygame take several seconds to come up on the handheld, and
until the app presents its first frame the screen is black. CardBrick.sh
covers that gap by zcat-ing one of these images straight into /dev/fb0
before it does anything else — no interpreter, no SDL, just a file copy.

Run at package build time (build_package.sh does this automatically):

    python3 make_splash.py --icon icon-large.png \
        --font PixelMplus10-Regular.ttf --out-dir dist/CardBrick/splash

For every target panel size this writes two gzipped raw images, named so
the launch script can pick one by the framebuffer geometry it finds in
/sys/class/graphics/fb0:

    splash-<W>x<H>-32.raw.gz   # XRGB8888 little-endian (B,G,R,X)
    splash-<W>x<H>-16.raw.gz   # RGB565 little-endian

The look matches the app's first frame (BG/DIM in cardbrick/app.py) so
the splash -> app handoff doesn't flash.
"""

import argparse
import gzip
import os
import struct
import sys

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("ERROR: Pillow is required (pip install pillow)", file=sys.stderr)
    sys.exit(1)

# Keep in sync with cardbrick/app.py.
BG = (242, 237, 227)   # warm paper
DIM = (138, 133, 123)  # pencil grey

# Panels of the devices this package targets (RG35XX family 640x480,
# RGB30 720x720, TrimUI Brick 1024x768) plus common nearby modes; an
# unlisted panel just means no splash, never a broken launch.
DEFAULT_SIZES = [
    (640, 480),
    (720, 480),
    (720, 576),
    (720, 720),
    (1024, 768),
    (1280, 720),
]


def compose(size, icon, font_path):
    w, h = size
    img = Image.new("RGB", size, BG)

    icon_side = int(min(w, h) * 0.45)
    scaled = icon.resize((icon_side, icon_side), Image.LANCZOS)
    icon_x = (w - icon_side) // 2
    icon_y = int(h * 0.42) - icon_side // 2
    img.paste(scaled, (icon_x, icon_y), scaled)

    text = "Loading..."
    # PixelMplus10 is a 10px bitmap-style face; multiples of 10 stay crisp.
    font_size = max(20, round(min(w, h) / 12 / 10) * 10)
    font = ImageFont.truetype(font_path, font_size)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), text, font=font)
    text_x = (w - (bbox[2] - bbox[0])) // 2 - bbox[0]
    text_y = icon_y + icon_side + int(h * 0.08)
    draw.text((text_x, text_y), text, font=font, fill=DIM)
    return img


def to_xrgb8888(img):
    rgb = img.tobytes()
    n = len(rgb) // 3
    buf = bytearray(n * 4)
    buf[0::4] = rgb[2::3]  # B
    buf[1::4] = rgb[1::3]  # G
    buf[2::4] = rgb[0::3]  # R
    buf[3::4] = b"\xff" * n
    return bytes(buf)


def to_rgb565(img):
    rgb = img.tobytes()
    n = len(rgb) // 3
    buf = bytearray(n * 2)
    for i in range(n):
        r, g, b = rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2]
        struct.pack_into(
            "<H", buf, 2 * i, ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
        )
    return bytes(buf)


def write_gz(path, data):
    # mtime=0 keeps the output byte-identical across builds, so the
    # package manifest checksums don't churn.
    with open(path, "wb") as f:
        with gzip.GzipFile(filename="", mode="wb", fileobj=f, mtime=0) as gz:
            gz.write(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--icon", required=True, help="app icon PNG (RGBA)")
    parser.add_argument("--font", required=True, help="TTF/OTF for the text")
    parser.add_argument("--out-dir", required=True)
    parser.add_argument(
        "--size", action="append", dest="sizes", metavar="WxH",
        help="panel size to render (repeatable; default: known devices)")
    args = parser.parse_args()

    if args.sizes:
        sizes = [tuple(int(v) for v in s.lower().split("x")) for s in args.sizes]
    else:
        sizes = DEFAULT_SIZES

    icon = Image.open(args.icon).convert("RGBA")
    os.makedirs(args.out_dir, exist_ok=True)
    for size in sizes:
        img = compose(size, icon, args.font)
        for bpp, encode in ((32, to_xrgb8888), (16, to_rgb565)):
            name = f"splash-{size[0]}x{size[1]}-{bpp}.raw.gz"
            write_gz(os.path.join(args.out_dir, name), encode(img))
            print(f"  {name}")


if __name__ == "__main__":
    main()
