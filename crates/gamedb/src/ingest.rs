//! One game from Steam's store API (`appdetails`) → a record for games.db.

use rigcheck_core::{parse, requirement_text};
use serde::Deserialize;
use serde_json::Value;

use crate::{Error, GameRecord};

/// What an `appdetails` response turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingested {
    /// Boxed: a record is far bigger than the other outcomes.
    Game(Box<GameRecord>),
    /// DLC, a soundtrack, a demo, a tool, hardware…: `kind` is Steam's `type`.
    NotAGame { kind: String },
    /// The store has no page for it (`"success": false`), e.g. delisted.
    Unavailable,
}

/// Reads the `appdetails` response for `app_id`.
pub fn ingest(app_id: u32, json: &str) -> Result<Ingested, Error> {
    #[derive(Deserialize)]
    struct Entry {
        success: bool,
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "type")]
        kind: String,
        name: String,
        #[serde(default)]
        platforms: Platforms,
        /// An object with `minimum`/`recommended` HTML, or `[]` when empty.
        #[serde(default)]
        pc_requirements: Value,
        release_date: Option<ReleaseDate>,
    }
    #[derive(Deserialize, Default)]
    struct Platforms {
        #[serde(default)]
        linux: bool,
    }
    #[derive(Deserialize)]
    struct ReleaseDate {
        date: String,
    }

    let mut response: std::collections::HashMap<String, Entry> = serde_json::from_str(json)?;
    let entry = response
        .remove(&app_id.to_string())
        .ok_or("response is for another app")?;
    let Some(data) = entry.data.filter(|_| entry.success) else {
        return Ok(Ingested::Unavailable);
    };
    if data.kind != "game" {
        return Ok(Ingested::NotAGame { kind: data.kind });
    }
    let html = |name: &str| data.pc_requirements.get(name)?.as_str();
    Ok(Ingested::Game(Box::new(GameRecord {
        app_id,
        minimum: html("minimum").map(parse),
        recommended: html("recommended").map(parse),
        minimum_text: html("minimum").map(requirement_text),
        recommended_text: html("recommended").map(requirement_text),
        name: data.name,
        release_date: data.release_date.map(|release| release.date),
        native: data.platforms.linux,
        ..GameRecord::default()
    })))
}
