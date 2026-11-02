//! Getting a new games.db: `manifest.json` says what's published, and the
//! downloaded file is checked before it replaces the old one (ROADMAP §7.3).

use serde::{Deserialize, Serialize};

use crate::Error;

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
