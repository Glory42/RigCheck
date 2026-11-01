"""Hand-drawn 16x16 rigcheck icon (simplified for favicons). Usage: python3 draw_icon_16.py OUT.png"""

import struct
import sys
import zlib

PALETTE = {
    ".": (0, 0, 0, 0),
    "D": (0x33, 0x3D, 0x47, 255),
    "S": (0x5B, 0x6B, 0x7A, 255),
    "L": (0x8A, 0x9B, 0xAB, 255),
    "O": (0xF5, 0x8A, 0x1F, 255),
    "o": (0xC2, 0x5E, 0x0A, 255),
    "G": (0x2E, 0xA0, 0x43, 255),
    "g": (0x1A, 0x6B, 0x2A, 255),
    "W": (0xFF, 0xFF, 0xFF, 255),
}

ART = """
.......DOOO.....
.......Doo......
......DDDD......
......DLLD......
.....DLSSLD.....
.....DS..SD.....
....DLDDDDLD....
....DS.S..WWWW..
...DLS...WGGGGW.
...DS...WGGGGGGW
..DLDDDDWGGGGWGW
..DS....WGWGWGGW
.DLS....WGGWGGGW
DDDDDDDD.WggggW.
.SSSSSSS..WWWW..
................
"""

rows = [line for line in ART.strip("\n").split("\n")]
assert len(rows) == 16 and all(len(r) == 16 for r in rows), [len(r) for r in rows]
raw = b"".join(b"\x00" + b"".join(bytes(PALETTE[c]) for c in row) for row in rows)


def chunk(tag, data):
    c = struct.pack(">I", len(data)) + tag + data
    return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


with open(sys.argv[1], "wb") as f:
    f.write(b"\x89PNG\r\n\x1a\n")
    f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0)))
    f.write(chunk(b"IDAT", zlib.compress(raw, 9)))
    f.write(chunk(b"IEND", b""))
