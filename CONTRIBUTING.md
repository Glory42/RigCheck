# Contributing to rigcheck

Thanks for helping! rigcheck is a small project, so the most useful contributions are often small too: a wrong verdict reported with details, a translation fix, or a capture of your hardware.

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through [SECURITY.md](SECURITY.md), not public issues.

## Quick ways to help

### Report a wrong verdict

Open a [Wrong verdict](https://github.com/Glory42/rigcheck/issues/new?template=wrong-verdict.yml) issue with the game, the stamp you got, what you expected, and the text from **My Rig → Copy as text**. If you know the game runs (or doesn't), say how well and with which Proton version.

### Fix hardware the app doesn't recognize

Verdicts need to know your CPU and GPU. Both live in [`data/hardware.toml`](data/hardware.toml):

- **A name isn't matched** (the game page shows Steam's text but no comparison): add an `aliases` entry to the right part.
- **A part is missing**: add it with a rough `score` (GPUs relative to a GTX 1060 6GB = 100, CPUs relative to an i5-4460 = 100) and, for GPUs, its PCI ID if you know it. The tests in `crates/core/tests/catalog.rs` show how names are matched.

### Share your machine as a test fixture

Hardware detection is tested against captured machines in [`data/fixtures/hardware/`](data/fixtures/hardware/). Five of the six are hand-written; real captures are better:

```sh
data/fixtures/hardware/capture.sh my-machine-name
```

It copies only the files detection reads (`/proc/cpuinfo`, `/proc/meminfo`, `/sys/class/drm/*`, `/etc/os-release`, and `vulkaninfo` / `nvidia-smi` output). Look through the folder before you commit it, then open a PR. The [fixtures README](data/fixtures/hardware/README.md) has the details.

### Translate

UI text lives in [`app/i18n/`](app/i18n/) as [Fluent](https://projectfluent.org) files: `en.ftl`, `tr.ftl`, `de.ftl`.

- To fix a translation, edit the line in that language's file.
- To add a language, copy `en.ftl`, translate every line, and add the language to `Language` in [`app/src/i18n.rs`](app/src/i18n.rs).
- Write uppercase labels in uppercase yourself (`AYARLAR`, `EINSTELLUNGEN`). The code never uppercases text, because that gets letters like Turkish İ wrong.

A test fails if any language is missing a line, so `cargo test` tells you when you're done.

## Development

Setup and everyday commands are in the [README](README.md#development). Before opening a PR, run what CI runs:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check bans
```

The UI tests render offscreen. On a machine without a GPU, set `ICED_TEST_BACKEND=tiny-skia`, as CI does.

### How code gets written here

- **Test first.** Write a failing test, then just enough code to pass it, one behavior at a time. A test that passes on its first run should be shown to fail when the code is broken.
- **Test through public interfaces.** Tests call a crate's public functions (or click through the UI with `iced_test`), not private helpers, so code can be refactored without rewriting tests.
- **Expected values come from outside the code**: Steam's text, a worked example, the roadmap. Not from running the code and copying its output.
- **Real data over invented data.** Fixtures are captured from real machines and real store pages where possible.
- **Keep `crates/core` pure** (no files, no network). Only `crates/net` may use an HTTP client; `cargo deny check bans` enforces it.

The [roadmap](ROADMAP.md) says what's planned and why. For anything bigger than a fix, open an issue first so we can agree on the approach.

### Pull requests

- Branch from `master` and keep each PR to one topic.
- Write commit messages that say what changed and why.
- The [PR template](.github/pull_request_template.md) asks for a summary (a diagram, call tree or file tree beats prose), evidence (a test that failed before and passes now, or a screenshot), and how risky the merge is.

## License

Code contributions are MIT licensed, like the rest of the code ([LICENSE](LICENSE)). Data contributions to `data/` are included in the published game database, which will be licensed under the ODbL (see [ROADMAP §1.2](ROADMAP.md#12-licenses)).
