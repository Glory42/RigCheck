---
name: data-pipeline
description: Work on rigcheck's data pipeline and games.db: crawling Steam, building/validating/publishing the database, schema or query changes in crates/gamedb, and the Build games.db workflow. Use when a task touches pipeline/, crates/gamedb, database contents, or data freshness.
---

# Data pipeline and games.db

The pipeline builds `games.db` daily in GitHub Actions. The logic is in
`pipeline/src/`; `pipeline/src/main.rs` is only the command line. The app reads
the same schema through `crates/gamedb`.

## Commands

```sh
# Fetch Steam's app list (needs a key; network)
cargo run -p rigcheck-pipeline -- applist apps.json

# Crawl store pages into games.db; resumable, rate-limited
cargo run -p rigcheck-pipeline -- crawl apps.json games.db 3000 reports.json

# Merge data sources, fill gaps, re-match hardware, rebuild search
cargo run -p rigcheck-pipeline -- build games.db apps.json reports.json \
  awacy.json pci.ids blender.jsonl

# Refuse to publish a bad database
cargo run -p rigcheck-pipeline -- validate games.db

# Unknown hardware, to feed data/hardware.toml
cargo run -p rigcheck-pipeline -- report games.db

# Write games.db.zst + manifest.json
cargo run -p rigcheck-pipeline -- publish games.db out <version> <built_at> <url> <min_app_version>
```

CI uses `cargo build --profile pipeline --locked -p rigcheck-pipeline`; the
workflow is `.github/workflows/build-db.yml`.

## Rules

- **Never hit Steam or any host from tests.** Pipeline tests use fixtures from
  `data/fixtures/` and temp databases.
- **Only `crates/net` may make requests**, and only to hosts in ROADMAP §6.
  `deny.toml` fails CI otherwise. Don't add a host without changing the
  roadmap, README and privacy text together.
- **Don't bypass the rate limiter or parallelism limits.** Steam's limits are
  per 5 minutes; the crawl is designed to be run repeatedly.
- **Keep `crates/core` the single parser/matcher.** The pipeline and app must
  share it; don't fork requirement-parsing logic into `pipeline/`.
- **Schema changes are cross-cutting.** Update `crates/gamedb` (schema,
  migrations, queries, tests), the app that reads it, and `manifest.json`'s
  `schema_version` / `min_app_version` when compatibility changes. The app
  must keep its current database instead of opening an incompatible one.
- **Respect the size budget** (ROADMAP §7.4). Don't ship raw requirement HTML;
  store cleaned text only. `validate` enforces the cap.
- **The build is a pure function of its inputs** where practical: `build` should
  be deterministic given the same sources, so failures are reproducible.

## Where to test

- `pipeline/tests/` for each stage (`applist`, `crawl`, `build`, `validate`,
  `publish`, `report`, source joiners).
- `crates/gamedb/tests/` for schema, queries, install/update safety.
- `crates/core/tests/` for parsing and matching, with real corpus fixtures.
- Download/update safety tests should use temp dirs and verify a failed update
  never leaves a broken database.

```sh
cargo test -p rigcheck-pipeline
cargo test -p rigcheck-gamedb
cargo test -p rigcheck-core
```

## Workflow and releases

The database is published to the `db-latest` GitHub release and assembled in
`db-wip` between publishes. Signing happens in the `publish` job behind the
`db-signing` environment; never move data-fetching steps into it.

When changing `build-db.yml`, keep the budget/`continue-on-error` behavior so a
crawl timeout still publishes the progress. Validate a real build locally with
`validate` before relying on CI.
