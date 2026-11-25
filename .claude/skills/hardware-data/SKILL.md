---
name: hardware-data
description: Add or fix CPU/GPU data in rigcheck: hardware-table entries, aliases, PCI IDs, scores, or captured hardware-detection fixtures. Use when a game shows an unknown part, a name doesn't match, a part is missing, or detection needs a real machine.
---

# Hardware data

Two separate jobs. Decide which one the task is before changing anything.

## A. A part is missing or a name doesn't match (hardware table)

The table is `data/hardware.toml`, hand-maintained and shipped inside
`games.db` (`rigcheck_core::Catalog`). The pipeline fills scores the table
lacks from Blender Open Data and adds GPU PCI IDs, so the table is the seed,
not the whole published database.

Typical fixes:

1. **A name isn't matched** (the game page shows Steam's text but no
   comparison): add the spelling to `aliases` on the existing entry. Matching
   normalizes case, drops brand words like NVIDIA/GeForce/Intel/Core and
   punctuation, so only add genuinely different spellings.
2. **A part is missing**: add a `[[cpu]]` or `[[gpu]]` entry with:
   - `name`: canonical display name (`GeForce GTX 1060 6GB`, `Core i5-4690`).
   - `vendor`: `nvidia`, `amd` or `intel`.
   - `score`: rough relative performance. GPUs relative to a GTX 1060 6GB =
     100; CPUs relative to an i5-4460 = 100. Compare only within a kind.
   - GPUs: `vram_mb`, and `pci = ["vendor:device"]` only when certain.
   - `aliases` for other spellings Steam or `/proc` use.
3. **A wrong verdict points at hardware**: check the matched entry's `score`,
   `vram_mb` and `pci` before changing matcher logic.

Find unknowns from a built database:

```sh
cargo run -p rigcheck-pipeline -- report path/to/games.db
```

### Test first

Add the matching case to `crates/core/tests/catalog.rs` before editing
`hardware.toml`, using the real requirement string:

```sh
cargo test -p rigcheck-core --test catalog
```

Keep aliases narrow enough not to swallow a different model. Don't invent PCI
IDs; copy them from `pci.ids` or the machine's `/sys`.

## B. Detection needs a machine (fixtures)

Detection reads `/proc`, `/sys`, `/etc` and `vulkaninfo` / `nvidia-smi` output.
Tests replay fixtures from `data/fixtures/hardware/`.

Capture a real machine:

```sh
data/fixtures/hardware/capture.sh my-machine-name
```

The script copies only the files detection reads and runs `vulkaninfo --summary`
and `nvidia-smi` if installed. **Look through the output before committing it.**
Nothing is uploaded.

Then:

1. Add tests in `crates/hardware/tests/` that point the detector at the new
   fixture directory (the crate takes a root path; never read the real `/`).
2. Add the fixture to the table in `data/fixtures/hardware/README.md` with its
   machine and whether it is real or hand-written.
3. Run `cargo test -p rigcheck-hardware`.

Hand-written fixtures are acceptable when a capture isn't possible, but they
must use real PCI IDs and real file formats, and only the lines detection reads.

## Finish

```sh
cargo test -p rigcheck-core
cargo test -p rigcheck-hardware
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

In the PR, say where the data came from (issue, store page, `pci.ids`, machine
capture) and include the test that failed before.
