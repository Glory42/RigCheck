# rigcheck: guide for coding agents

rigcheck is a Linux desktop app that tells you whether your PC can run a Steam
game. It's a Rust workspace: an iced desktop app (`app/`), a data pipeline
(`pipeline/`), and shared crates (`crates/`).

Read these before changing code:

- [`CONTEXT.md`](CONTEXT.md): the domain words (rig, record, swap, verdict,
  stamp, lookup…). Use them in code, comments and commits.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): how code gets written here, including
  the test-first rule.
- [`ROADMAP.md`](ROADMAP.md): why the architecture is what it is. §6 is the
  network allowlist, §7 the data design, §8 the matching logic.

## Commands

```sh
cargo install cargo-deny --locked      # once
cargo run -p rigcheck --release        # the app
cargo test --workspace                 # all tests (UI tests render offscreen)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check advisories bans       # known vulns; only crates/net may use HTTP
```

On a machine without a GPU, set `ICED_TEST_BACKEND=tiny-skia`, as CI does.

Run one test file or test:

```sh
cargo test -p rigcheck-core --test parser reads_memory
```

Accept parser snapshots only after reviewing the diff:

```sh
INSTA_UPDATE=always cargo test -p rigcheck-core --test parser
git diff crates/core/tests/snapshots/
```

Regenerate screenshots (needs a real `games.db`):

```sh
RIGCHECK_SCREENSHOT_DB=~/.local/share/rigcheck/games.db \
  cargo test -p rigcheck screenshots -- --ignored
```

Website (separate npm project):

```sh
cd website && npm install && npm run build
```

## Rules that apply to every change

- **Test first.** Write a failing test, then just enough code to pass it, one
  behavior at a time. A test that passes on its first run should be shown to
  fail when the code is broken.
- **Test through public interfaces.** Tests call a crate's public functions
  (or click through the UI with `iced_test`), not private helpers, so code can
  be refactored without rewriting tests.
- **Expected values come from outside the code**: Steam's text, a captured
  machine, a worked example, the roadmap. Never copy what the code currently
  prints.
- **Keep `crates/core` pure**: no files, no network. Only `crates/net` may use
  an HTTP client, and only for the hosts in ROADMAP §6. `deny.toml` enforces
  both.
- **`app/` stays thin**: screens, state and wiring. Anything worth unit-testing
  belongs in a crate.
- **Root paths over real hardware.** `hardware` and `steam` take a root path
  (default `/` or `$HOME`), so tests point them at fixtures in `data/fixtures/`.
- **All user-facing text goes through Fluent** (`app/i18n/*.ftl`). Add an ID to
  `en.ftl` first, then `tr.ftl` and `de.ftl`. Write uppercase labels uppercase
  in the files; never uppercase in code, which gets Turkish İ/ı wrong. Game
  names and Steam's requirement text stay in English.
- **Real data over invented data.** Prefer a real store page or machine
  capture; don't hit Steam or any other host from tests.
- **Don't add a dependency without discussing it.** If a dependency changes,
  commit the updated `Cargo.lock`.
- **Don't bump the version or touch release recipes** in a normal PR. See the
  `release` skill.
- **Don't edit generated or ignored paths**: `target/`,
  `website/node_modules/`, `packaging/aur/src/`, `packaging/aur/pkg/`.
- **Commit messages say what changed and why.** No `Co-Authored-By` or other
  agent-attribution trailers.
- **One topic per PR**, branch from `master`. For anything bigger than a fix,
  open an issue first so the approach can be agreed.

## Definition of done

Run what CI runs before calling a change done:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check advisories bans
```

For packaging-only changes, also run:

```sh
desktop-file-validate packaging/dev.gorkemkaryol.rigcheck.desktop
appstreamcli validate --no-net --explain packaging/dev.gorkemkaryol.rigcheck.metainfo.xml
```

Update docs when behavior, data shapes or user-visible text changes. A PR needs
a summary, evidence (a test that failed before and passes now, or a screenshot)
and a risk note; the PR template asks for those.

## Skills

Repo skills live in `.agents/skills/`. `.claude/skills/` is a byte-identical
copy for Claude Code and OpenCode, not a symlink, so checkouts work on Windows
too. Edit the `.agents/skills/` copy, then mirror it with
`cp -a .agents/skills/. .claude/skills/`; CI runs
`diff -r .claude/skills .agents/skills` and fails if they drift.

Load the skill that matches the task:

| Skill | Use when |
|---|---|
| `test-first` | Implementing a feature or fixing a bug anywhere in the workspace |
| `hardware-data` | A CPU/GPU is missing or unmatched, scores/aliases/PCI IDs need changing, or detection needs a captured machine |
| `translations` | Changing UI or website text, or adding a language |
| `data-pipeline` | Working on `pipeline/`, `crates/gamedb`, crawling, validation or `games.db` |
| `release` | Bumping the version, tagging a release, or updating AUR/Flatpak/AppStream |
| `screenshots` | Regenerating the README/website screenshots |
| `website` | Changing the Astro showcase site in `website/` |
