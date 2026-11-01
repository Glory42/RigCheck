# Brand assets

| File | Source |
|---|---|
| `icon-16.png`, `icon-32.png`, `icon-96.png` | Original files from Icons8 |
| `icon-128.png` … `icon-1024.png` | Generated from `icon-32.png` with nearest-neighbor scaling (the art sits on a 32×32 pixel grid, so it stays sharp) |

Regenerate the large sizes:

```sh
for s in 128 256 512 1024; do magick icon-32.png -filter point -resize ${s}x${s} PNG32:icon-$s.png; done
```

## Attribution

"Oil Pump" (Color Pixels) icon by [Icons8](https://icons8.com), used under the Icons8 free license. The license requires a link to icons8.com wherever the icon appears publicly: the app's About page, the README, store listings and the website.

These icons are **not** covered by this repository's MIT license.
