"""Draw the rigcheck icon: a pixel-art oil derrick with a check badge.

Pure stdlib. Usage: python3 draw_icon_32.py OUT.png
"""

import struct
import sys
import zlib

N = 32

T = (0, 0, 0, 0)
STEEL = (0x5B, 0x6B, 0x7A, 255)
STEEL_LIGHT = (0x8A, 0x9B, 0xAB, 255)
STEEL_DARK = (0x33, 0x3D, 0x47, 255)
ORANGE = (0xF5, 0x8A, 0x1F, 255)
ORANGE_DARK = (0xC2, 0x5E, 0x0A, 255)
GREEN = (0x2E, 0xA0, 0x43, 255)
GREEN_DARK = (0x1A, 0x6B, 0x2A, 255)
WHITE = (0xFF, 0xFF, 0xFF, 255)

img = [[T] * N for _ in range(N)]


def px(x, y, c):
    if 0 <= x < N and 0 <= y < N:
        img[y][x] = c


def line(x0, y0, x1, y1, c):
    dx, dy = abs(x1 - x0), -abs(y1 - y0)
    sx = 1 if x0 < x1 else -1
    sy = 1 if y0 < y1 else -1
    err = dx + dy
    while True:
        px(x0, y0, c)
        if x0 == x1 and y0 == y1:
            return
        e2 = 2 * err
        if e2 >= dy:
            err += dy
            x0 += sx
        if e2 <= dx:
            err += dx
            y0 += sy


def rect(x0, y0, x1, y1, c):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            px(x, y, c)


def leg_x(y, left):
    """x of the left/right tower leg at row y (top y=5, base y=27)."""
    t = (y - 5) / (27 - 5)
    return round(13 - t * 9) if left else round(18 + t * 9)


# Pennant on top
line(15, 0, 15, 4, STEEL_DARK)
rect(16, 0, 19, 1, ORANGE)
px(20, 0, ORANGE)
px(16, 2, ORANGE_DARK)
px(17, 2, ORANGE_DARK)

# Crown block
rect(12, 4, 19, 5, STEEL_DARK)

# Girders and alternating V / inverted-V (Warren truss) bracing
girders = [11, 19, 27]
prev = 5
for i, g in enumerate(girders):
    l0, r0 = leg_x(prev, True), leg_x(prev, False)
    l1, r1 = leg_x(g, True), leg_x(g, False)
    if i % 2 == 0:  # V: top corners down to the middle
        line(l0 + 1, prev + 1, 15, g - 1, STEEL)
        line(r0 - 1, prev + 1, 16, g - 1, STEEL)
    else:  # inverted V: middle top down to the bottom corners
        line(15, prev + 1, l1 + 1, g - 1, STEEL)
        line(16, prev + 1, r1 - 1, g - 1, STEEL)
    line(l1, g, r1, g, STEEL_DARK)
    prev = g

# Legs: two pixels thick, dark outer edge + lighter inner edge
for y in range(5, 28):
    lx, rx = leg_x(y, True), leg_x(y, False)
    px(lx, y, STEEL_DARK)
    px(lx + 1, y, STEEL_LIGHT)
    px(rx, y, STEEL_DARK)
    px(rx - 1, y, STEEL_LIGHT)

# Ground platform
rect(1, 28, 30, 29, STEEL_DARK)
rect(2, 30, 29, 30, STEEL)

# Check badge, bottom right, with a white ring to separate it from the tower
cx, cy, r = 25.0, 24.5, 6.5
for y in range(N):
    for x in range(N):
        d = ((x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2) ** 0.5
        if d <= r - 1.2:
            px(x, y, GREEN_DARK if (r - 2.2 < d and y >= cy + 2) else GREEN)
        elif d <= r:
            px(x, y, WHITE)
for x, y in [(22, 24), (23, 25)]:
    px(x, y, WHITE)
    px(x, y + 1, WHITE)
for x, y in [(24, 25), (25, 24), (26, 23), (27, 22)]:
    px(x, y, WHITE)
    px(x, y + 1, WHITE)


def write_png(path, rows):
    raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in rows)

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(raw, 9)))
        f.write(chunk(b"IEND", b""))


write_png(sys.argv[1], img)
