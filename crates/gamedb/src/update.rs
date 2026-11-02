//! Getting a new games.db: `manifest.json` says what's published, and the
//! downloaded file is checked before it replaces the old one (ROADMAP §7.3).

use std::fs::{self, File};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, GameDb};

/// The games.db schema this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

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
}

/// Replaces the games.db at `dest` with the downloaded `zst`, but only once
/// it matches the manifest's SHA-256 and opens as a games.db. Any failure
/// leaves the old file as it was.
pub fn install(zst: &Path, manifest: &Manifest, dest: &Path) -> Result<GameDb, Error> {
    // At most the 50 MB size budget (ROADMAP §7.4), so reading it whole is fine.
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
    let unpacked = zstd::stream::copy_decode(bytes.as_slice(), File::create(&new)?)
        .map_err(Error::from)
        .and_then(|()| check(&new, manifest));
    if let Err(error) = unpacked {
        let _ = fs::remove_file(&new);
        return Err(error);
    }
    fs::rename(&new, dest)?;
    GameDb::open(dest)
}

/// The sanity query: a SQLite file whose schema is the one announced.
fn check(path: &Path, manifest: &Manifest) -> Result<(), Error> {
    let schema = GameDb::open(path)?.meta("schema_version")?;
    if schema != Some(manifest.schema_version.to_string()) {
        return Err("the download isn't the games.db the manifest announced".into());
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
