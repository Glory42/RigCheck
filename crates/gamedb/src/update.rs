//! Getting a new games.db: `manifest.json` says what's published, and the
//! downloaded file is checked before it replaces the old one (ROADMAP §7.3).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, GameDb, guarded};

/// The games.db schema this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The `meta` key an installed file keeps its manifest's `db_version` under.
pub(crate) const DB_VERSION: &str = "db_version";

/// `manifest.json`, published next to `games.db.zst`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    /// e.g. "2026-11-01.1": build date and run number.
    pub db_version: String,
    /// RFC 3339, e.g. "2026-11-01T04:00:00Z".
    pub built_at: String,
    /// Where `games.db.zst` is downloaded from.
    pub url: String,
    /// Of `games.db.zst`, as lowercase hex.
    pub sha256: String,
    pub size_bytes: u64,
    /// The oldest app version that can read this schema.
    pub min_app_version: String,
}

impl Manifest {
    pub fn from_json(json: &str) -> Result<Manifest, Error> {
        Ok(serde_json::from_str(json)?)
    }

    /// Whether the published database uses a schema this build can't read,
    /// so the app keeps its current one and asks to be updated.
    pub fn needs_newer_app(&self) -> bool {
        self.schema_version > SCHEMA_VERSION
    }

    /// Whether this build replaces the `installed` one (its `db_version`
    /// meta row; `None` if there's none): only a newer build does, so an
    /// older one served again can't roll the data back. Versions are
    /// "date.run", and runs compare as numbers ("2026-11-01.10" is after
    /// "2026-11-01.9"). A version named some other way (a seed, a hand
    /// build) is replaced by any published one.
    pub fn is_update_for(&self, installed: Option<&str>) -> bool {
        let Some(installed) = installed else {
            return true;
        };
        match (build(&self.db_version), build(installed)) {
            (Some(published), Some(installed)) => published > installed,
            _ => installed != self.db_version,
        }
    }
}

/// "2026-11-01.10" as ("2026-11-01", 10): ISO dates sort as text.
fn build(db_version: &str) -> Option<(&str, u64)> {
    let (date, run) = db_version.split_once('.')?;
    let is_date = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    is_date.then_some(())?;
    Some((date, run.parse().ok()?))
}

/// How big a download and the database it unpacks to may be. A tampered
/// release could otherwise fill the disk or the memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub download_bytes: u64,
    pub unpacked_bytes: u64,
}

impl Default for Limits {
    /// Twice the 50 MB download budget (ROADMAP §7.4); the unpacked file
    /// is ~5× the download.
    fn default() -> Limits {
        Limits {
            download_bytes: 100_000_000,
            unpacked_bytes: 1_000_000_000,
        }
    }
}

/// Replaces the games.db at `dest` with the downloaded `zst`, but only once
/// it matches the manifest's size and SHA-256, unpacks within the default
/// [`Limits`], and holds only the schema the pipeline writes. Any failure
/// leaves the old file as it was.
pub fn install(zst: &Path, manifest: &Manifest, dest: &Path) -> Result<GameDb, Error> {
    install_within(zst, manifest, dest, &Limits::default())
}

/// [`install`], within `limits`.
pub fn install_within(
    zst: &Path,
    manifest: &Manifest,
    dest: &Path,
    limits: &Limits,
) -> Result<GameDb, Error> {
    let size = fs::metadata(zst)?.len();
    if size > limits.download_bytes {
        return Err("the download is bigger than a games.db can be".into());
    }
    if size != manifest.size_bytes {
        return Err("the download isn't the size the manifest says".into());
    }
    // Within the limit above, so reading it whole is fine.
    let bytes = fs::read(zst)?;
    if sha256_hex(&bytes) != manifest.sha256 {
        return Err("the download doesn't match the manifest's SHA-256".into());
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir)?;
    }
    // Unpacked next to `dest`, so the rename below stays on one filesystem
    // and replaces the old file in one step.
    let new = dest.with_extension("db.new");
    let unpacked = unpack(&bytes, &new, limits.unpacked_bytes)
        .and_then(|()| check(&new, manifest))
        .and_then(|()| add_indexes(&new))
        .and_then(|()| GameDb::open(&new)?.set_meta(DB_VERSION, &manifest.db_version));
    if let Err(error) = unpacked {
        let _ = fs::remove_file(&new);
        return Err(error);
    }
    fs::rename(&new, dest)?;
    GameDb::open(dest)
}

/// Decompresses `zst` into `to`, stopping past `limit` bytes.
fn unpack(zst: &[u8], to: &Path, limit: u64) -> Result<(), Error> {
    let mut decoder = zstd::stream::read::Decoder::new(zst)?.take(limit + 1);
    let written = io::copy(&mut decoder, &mut File::create(to)?)?;
    if written > limit {
        return Err("the download unpacks to more than a games.db can be".into());
    }
    Ok(())
}

/// Checks the unpacked file before anything writes to it, read-only: a
/// sound SQLite file, the schema version the manifest announced, and no
/// SQL that would run on a write (triggers, views, virtual tables) other
/// than the pipeline's own, word for word. Tables and indexes may differ:
/// the pipeline adds those over time, and they run nothing.
fn check(path: &Path, manifest: &Manifest) -> Result<(), Error> {
    let conn = guarded(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)?;
    let health: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if health != "ok" {
        return Err("the download is a damaged database".into());
    }
    let ours = runnable_sql(&GameDb::create_in_memory()?.conn)?;
    for (name, sql) in runnable_sql(&conn)? {
        if ours.get(&name) != Some(&sql) {
            return Err(format!("the download has SQL of its own ({name})").into());
        }
    }
    let schema: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .ok();
    if schema != Some(manifest.schema_version.to_string()) {
        return Err("the download isn't the games.db the manifest announced".into());
    }
    Ok(())
}

/// Every trigger, view and virtual table in `conn`, by name, with its SQL
/// in single spaces.
fn runnable_sql(conn: &Connection) -> Result<HashMap<String, String>, Error> {
    let mut statement = conn.prepare(
        "SELECT name, sql FROM sqlite_master
         WHERE type IN ('trigger', 'view')
            OR (type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%')",
    )?;
    let rows = statement.query_map([], |row| {
        let sql: Option<String> = row.get(1)?;
        let sql = sql.unwrap_or_default();
        Ok((
            row.get(0)?,
            sql.split_whitespace().collect::<Vec<_>>().join(" "),
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Indexes the app needs but the pipeline doesn't. Built here rather than
/// shipped, since they'd add ~5 MB to every download.
/// - Paging through all games by popularity: ~5 ms a page instead of up to
///   a second on 200k games.
/// - A game's required CPUs and GPUs, read for every game page and Catalog
///   row: without it each lookup scans the whole table (~200k rows after a
///   month of crawling).
/// - `games_search`: the name search again, with indexes for 1–3 letter
///   prefixes. Typing "s" matches ~50k names; this takes it from ~90 ms to
///   ~40 ms. About 6 s to build, on the download's thread.
fn add_indexes(path: &Path) -> Result<(), Error> {
    GameDb::open(path)?.conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS games_by_popularity ON games (popularity DESC, name);
         CREATE INDEX IF NOT EXISTS requirement_hardware_by_game
             ON requirement_hardware (app_id, tier, kind, position);
         CREATE VIRTUAL TABLE IF NOT EXISTS games_search USING fts5(
             name, content='games', content_rowid='app_id',
             tokenize='unicode61 remove_diacritics 2', prefix='1 2 3'
         );
         INSERT INTO games_search(games_search) VALUES ('rebuild');",
    )?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
