<img src="assets/brand/icon-128.png" alt="rigcheck icon" width="96" align="right">

# rigcheck

A Linux desktop app that tells you whether your PC can run a game before you buy it. It checks your installed Steam library and can search every game on Steam, covering both hardware requirements and Linux-specific factors (Proton compatibility, anti-cheat, drivers, Vulkan).

Local-first: no accounts, no telemetry, no servers. All data lives on your machine.

> Status: early development. See [ROADMAP.md](ROADMAP.md).

## Development

rigcheck is all Rust: a Cargo workspace with the desktop app (built with [iced](https://iced.rs)), the data pipeline and the shared crates.

```
app/        desktop app (binary: rigcheck)
pipeline/   data pipeline that builds games.db
crates/     core (parser + matcher), hardware, steam, gamedb, net
```

**Setup** (Arch shown; any distro with rustup works):

```sh
sudo pacman -S --needed rustup base-devel
rustup default stable            # includes clippy and rustfmt
cargo install cargo-deny --locked
```

No GUI system packages are needed to build: iced loads Wayland, X11 and Vulkan at runtime.

**Common commands:**

```sh
cargo run -p rigcheck            # open the app
cargo test --workspace           # all tests (UI tests run headless)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check bans            # only crates/net may use an HTTP client
```

CI runs fmt, clippy, tests and `cargo deny` on pushes to `master` and on pull requests.

## License

Code: [MIT](LICENSE).

