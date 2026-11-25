---
name: test-first
description: Write or fix Rust code in rigcheck test-first. Use when implementing a feature, fixing a bug, changing behavior in app/, crates/ or pipeline/, or when asked to add tests.
---

# Test-first in rigcheck

This repo is built test-first (see `CONTRIBUTING.md` → "How code gets written
here"). A behavior change starts with a test that fails for the right reason.

## 1. Find the public seam

Put the test against the interface callers use, not a private helper:

| Area | Test location |
|---|---|
| `crates/core` | `crates/core/tests/*.rs`; parser cases read real HTML from `data/fixtures/requirements/corpus.json` |
| `crates/hardware`, `crates/steam` | `crates/*/tests/*.rs`, pointed at `data/fixtures/` via the crate's root-path API |
| `crates/gamedb` | `crates/gamedb/tests/*.rs` with `tempfile` directories |
| `crates/net` | `crates/net/tests/*.rs`; use local fixtures/fakes, never the real network |
| `app/` | `#[cfg(test)]` module in the screen's file, driving the app with `iced_test`; shared data is in `app/src/test_support.rs` |
| `pipeline/` | `pipeline/tests/*.rs`, using fixture inputs and a temp database |

If the only place to test something is a private function, consider whether the
code should be split so the behavior has a public seam.

## 2. Write the failing test

- One behavior per test; name it after the behavior (`matches_gpus_with_and_without_their_memory_size`).
- Take expected values from outside the code: Steam's text, a captured machine,
  a worked example, the roadmap. Don't run the code and copy its output.
- Prefer real fixtures. Add a fixture only from a real store page or real
  machine capture when possible.
- For UI, send `Message`s and assert on the rendered view or resulting state,
  the way the existing tests do.

Run it and confirm it fails for the expected reason (usually "not implemented"
or a wrong value), not because it doesn't compile:

```sh
cargo test -p <crate> --test <file> <test_name>
```

## 3. Implement the minimum

Change just enough to pass. Re-run the one test, then the crate:

```sh
cargo test -p rigcheck-core
```

## 4. Keep invariants cheap

- Add a second test only for a distinct behavior, not for the same one.
- If the behavior is easy to break silently, add a regression test that would
  fail on the old code.
- Snapshot tests: `crates/core/tests/snapshots/parser__corpus.snap` is accepted
  with `INSTA_UPDATE=always cargo test -p rigcheck-core --test parser`. Always
  review the snapshot diff; never blind-accept it.

## 5. Finish

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

On a machine without a GPU, prefix with `ICED_TEST_BACKEND=tiny-skia`.

For the PR, keep the evidence: say which test failed before the change and
passes after it. Don't weaken an assertion to make a change pass.
