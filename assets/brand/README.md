# Brand assets

The rigcheck icon: a pixel-art oil derrick (the "rig") with a check badge. Original artwork made for this project and covered by the repository's MIT license.

| File | Source |
|---|---|
| `src/draw_icon_32.py` | Draws the 32×32 master icon |
| `src/draw_icon_16.py` | Hand-drawn 16×16 version for favicons, stored as a text pixel map: edit the characters to change pixels |
| `icon-16.png`, `icon-32.png` | Output of the scripts above |
| `icon-64.png` … `icon-1024.png` | `icon-32.png` scaled up with nearest-neighbor, so the pixels stay sharp |

Regenerate everything after editing a script:

```sh
./build.sh
```

Requires `python3` (standard library only) and ImageMagick 7.

## Palette

| Name | Hex | Use |
|---|---|---|
| Steel dark | `#333D47` | Outlines, crown block, platform |
| Steel | `#5B6B7A` | Bracing, platform edge |
| Steel light | `#8A9BAB` | Leg highlights |
| Orange | `#F58A1F` / `#C25E0A` | Pennant |
| Green | `#2EA043` / `#1A6B2A` | Check badge |
| White | `#FFFFFF` | Badge ring and check |
