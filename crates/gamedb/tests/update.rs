//! Downloading a new games.db: the manifest check and the safe swap
//! (ROADMAP §7.3).

use rigcheck_gamedb::Manifest;

/// The manifest the first scheduled build published to `db-latest`.
const PUBLISHED: &str = r#"{
  "schema_version": 1,
  "db_version": "2026-11-01.1",
  "built_at": "2026-11-01T18:26:21Z",
  "url": "https://github.com/Glory42/RigCheck/releases/download/db-latest/games.db.zst",
  "sha256": "4aae6cfa92b25ade6478620c60823abd1309051d53905b3fadd426f50f02146b",
  "size_bytes": 6357008,
  "min_app_version": "0.4.0"
}
"#;

#[test]
fn reads_the_published_manifest() {
    let manifest = Manifest::from_json(PUBLISHED).unwrap();

    assert_eq!(manifest.db_version, "2026-11-01.1");
    assert_eq!(manifest.size_bytes, 6_357_008);
    assert!(!manifest.needs_newer_app());
}

#[test]
fn a_newer_schema_needs_a_newer_app() {
    let json = PUBLISHED.replace(r#""schema_version": 1"#, r#""schema_version": 2"#);

    let manifest = Manifest::from_json(&json).unwrap();

    assert!(manifest.needs_newer_app());
}
