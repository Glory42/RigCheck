//! Tests for packaging games.db for download: zstd compression and
//! manifest.json (ROADMAP §7.3).

use std::process::Command;

use rigcheck_gamedb::GameDb;
use rigcheck_pipeline::publish::{Release, package};
use tempfile::TempDir;

fn release() -> Release {
    Release {
        db_version: "2026-11-01.1".to_owned(),
        built_at: "2026-11-01T04:00:00Z".to_owned(),
        url: "https://github.com/Glory42/rigcheck/releases/download/db-latest/games.db.zst"
            .to_owned(),
        min_app_version: "0.4.0".to_owned(),
    }
}

#[test]
fn the_download_decompresses_to_the_same_database() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("games.db");
    drop(GameDb::create(&db_path).unwrap());
    let out = dir.path().join("out");

    package(&db_path, &out, &release()).unwrap();

    let compressed = std::fs::read(out.join("games.db.zst")).unwrap();
    let restored = zstd::decode_all(compressed.as_slice()).unwrap();
    assert_eq!(restored, std::fs::read(&db_path).unwrap());
}

#[test]
fn the_manifest_describes_the_download_exactly() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("games.db");
    drop(GameDb::create(&db_path).unwrap());
    let out = dir.path().join("out");

    let manifest = package(&db_path, &out, &release()).unwrap();

    let zst = out.join("games.db.zst");
    let sha256sum = Command::new("sha256sum").arg(&zst).output().unwrap();
    let expected = String::from_utf8(sha256sum.stdout).unwrap();
    assert_eq!(manifest.sha256, expected.split_whitespace().next().unwrap());
    assert_eq!(manifest.size_bytes, std::fs::metadata(&zst).unwrap().len());
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.db_version, "2026-11-01.1");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(written["sha256"], manifest.sha256.as_str());
}
