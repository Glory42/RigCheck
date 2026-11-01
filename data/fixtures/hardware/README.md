# Hardware fixtures

Fake filesystem roots for `crates/hardware/tests/`. Each directory mirrors the files hardware detection reads (`proc/`, `sys/`, `etc/`), and `commands/<program>.txt` holds what `vulkaninfo --summary` and `nvidia-smi` printed. A missing `commands/` file means the program isn't installed.

| Fixture | Machine | Source |
|---|---|---|
| `amd-laptop` | Ryzen 5 7530U, Radeon iGPU | Real, from `capture.sh` (connector `card1-eDP-1` and render node `renderD128` added by hand from the same machine) |
| `amd-desktop` | Ryzen 5 5600X, RX 6600 | Hand-written |
| `nvidia-desktop` | i5-8400, GTX 1060 6GB | Hand-written |
| `intel-laptop` | i5-8250U, UHD 620 | Hand-written |
| `hybrid-laptop` | i7-9750H, UHD 630 + GTX 1650 Mobile | Hand-written |
| `steam-deck` | Steam Deck LCD | Hand-written |

Hand-written fixtures use real PCI IDs and the real file formats, but only the lines detection reads. Replace them with real captures when you can.

## Capturing a real machine

```sh
data/fixtures/hardware/capture.sh nvidia-desktop
```

It copies only the files listed in the script and runs `vulkaninfo --summary` and `nvidia-smi` if they're installed. Nothing is uploaded. Read through the output before committing it.
