//! `validate`: checks a built games.db before it's published.

use std::path::Path;

use rigcheck_gamedb::GameDb;

use crate::Error;

/// What a publishable games.db must meet.
#[derive(Debug, Clone, Copy)]
pub struct Rules {
    /// How many of the most popular games to check.
    pub top: usize,
    /// The share of those that must have CPU, GPU and RAM parsed. Below 1.0
    /// because some popular games leave a field out on Steam (TF2 lists no GPU).
    pub min_parsed_share: f64,
    /// Size limit for the file (ROADMAP §7.4).
    pub max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    NoGames,
    TooFewParsed { parsed: usize, top: usize },
    TooBig { bytes: u64, max: u64 },
}

/// Everything wrong with the games.db at `path`; empty if it can be published.
pub fn validate(db: &GameDb, path: &Path, rules: &Rules) -> Result<Vec<Problem>, Error> {
    let mut problems = Vec::new();
    let top = db.most_popular(rules.top)?;
    if top.is_empty() {
        return Ok(vec![Problem::NoGames]);
    }
    let parsed = top.iter().filter(|(_, status)| status == "parsed").count();
    if (parsed as f64) < rules.min_parsed_share * top.len() as f64 {
        problems.push(Problem::TooFewParsed {
            parsed,
            top: top.len(),
        });
    }
    let bytes = std::fs::metadata(path)?.len();
    if bytes > rules.max_bytes {
        problems.push(Problem::TooBig {
            bytes,
            max: rules.max_bytes,
        });
    }
    Ok(problems)
}
