#!/usr/bin/env python3
"""Generate app.ico for WSL Traffic Monitor.

The icon is checked in, but this script is the source of truth for it: run
`python3 generate_icon.py` from this directory to regenerate. Keeping the
generator alongside the binary means the design can be adjusted without
reverse-engineering a blob.

Design: a dark rounded tile carrying a cyan down-arrow and an amber up-arrow,
matching the colours the tray icon and overlay already use for download and
upload. Small sizes are emitted as BMP (the classic ICO payload), 128px and
above as PNG, which is what conventional icon tooling does.

No third-party dependencies: PNG is written directly via zlib.
"""

import struct
import zlib

# Palette shared with the overlay and tray renderer.
BACKGROUND = (20, 24, 32)
DOWNLOAD = (0, 220, 255)  # cyan
UPLOAD = (255, 208, 0)  # amber

SIZES = [16, 20, 24, 32, 48, 64, 128, 256]
PNG_FROM = 128  # sizes >= this are stored as PNG
SUPERSAMPLE = 4  # rendered at NxN then box-filtered for antialiasing


def rounded_rect_alpha(x, y, size, radius):
    """1.0 inside a rounded square covering the canvas, 0.0 outside."""
    r = radius
    cx = min(max(x, r), size - r)
    cy = min(max(y, r), size - r)
    dx, dy = x - cx, y - cy
    return 1.0 if (dx * dx + dy * dy) <= r * r else 0.0


def point_in_triangle(px, py, a, b, c):
    def sign(p1, p2, p3):
        return (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1])

    d1 = sign((px, py), a, b)
    d2 = sign((px, py), b, c)
    d3 = sign((px, py), c, a)
    has_neg = (d1 < 0) or (d2 < 0) or (d3 < 0)
    has_pos = (d1 > 0) or (d2 > 0) or (d3 > 0)
    return not (has_neg and has_pos)


def render(size):
    """Return RGBA bytes, top-down, for one icon size."""
    ss = SUPERSAMPLE
    hi = size * ss
    radius = hi * 0.20

    # Arrow geometry in normalised coordinates.
    down = [(0.10, 0.26), (0.44, 0.26), (0.27, 0.74)]
    up = [(0.56, 0.74), (0.90, 0.74), (0.73, 0.26)]
    down = [(x * hi, y * hi) for x, y in down]
    up = [(x * hi, y * hi) for x, y in up]

    # Accumulate coverage at supersampled resolution.
    acc = [[(0, 0, 0, 0.0) for _ in range(size)] for _ in range(size)]
    inv = 1.0 / (ss * ss)

    for gy in range(hi):
        for gx in range(hi):
            px, py = gx + 0.5, gy + 0.5
            if rounded_rect_alpha(px, py, hi, radius) == 0.0:
                continue
            if point_in_triangle(px, py, *down):
                colour = DOWNLOAD
            elif point_in_triangle(px, py, *up):
                colour = UPLOAD
            else:
                colour = BACKGROUND

            oy, ox = gy // ss, gx // ss
            r, g, b, a = acc[oy][ox]
            acc[oy][ox] = (r + colour[0], g + colour[1], b + colour[2], a + 1.0)

    out = bytearray()
    for row in acc:
        for r, g, b, count in row:
            if count == 0:
                out += bytes((0, 0, 0, 0))
            else:
                alpha = count * inv
                out += bytes(
                    (
                        int(r / count),
                        int(g / count),
                        int(b / count),
                        int(round(alpha * 255)),
                    )
                )
    return bytes(out)


def png_encode(rgba, size):
    def chunk(tag, data):
        payload = tag + data
        return struct.pack(">I", len(data)) + payload + struct.pack(
            ">I", zlib.crc32(payload) & 0xFFFFFFFF
        )

    raw = bytearray()
    for y in range(size):
        raw += b"\x00"  # filter type 0
        raw += rgba[y * size * 4 : (y + 1) * size * 4]

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def bmp_encode(rgba, size):
    """Classic ICO payload: BITMAPINFOHEADER, bottom-up BGRA, then an AND mask."""
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        size,
        size * 2,  # height covers XOR image plus AND mask
        1,
        32,
        0,
        0,
        0,
        0,
        0,
        0,
    )

    xor = bytearray()
    for y in range(size - 1, -1, -1):  # bottom-up
        for x in range(size):
            i = (y * size + x) * 4
            r, g, b, a = rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]
            xor += bytes((b, g, r, a))

    # AND mask is unused for 32bpp icons but must be present and row-padded to 4 bytes.
    mask_row = ((size + 31) // 32) * 4
    and_mask = bytes(mask_row * size)

    return header + bytes(xor) + and_mask


def main():
    images = []
    for size in SIZES:
        rgba = render(size)
        if size >= PNG_FROM:
            images.append((size, png_encode(rgba, size)))
        else:
            images.append((size, bmp_encode(rgba, size)))
        print(f"  rendered {size}x{size}")

    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)

    entries = bytearray()
    for size, data in images:
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,  # 0 means 256
            0 if size >= 256 else size,
            0,
            0,
            1,
            32,
            len(data),
            offset,
        )
        offset += len(data)

    with open("app.ico", "wb") as fh:
        fh.write(header + bytes(entries) + b"".join(d for _, d in images))
    print(f"wrote app.ico ({offset} bytes, {len(images)} sizes)")


if __name__ == "__main__":
    main()
