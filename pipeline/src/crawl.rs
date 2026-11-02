//! Deciding which apps to fetch next (ROADMAP Phase 4, §9).

use std::cmp::Reverse;
use std::collections::HashMap;

use rigcheck_gamedb::GameDb;

use crate::Error;
use crate::applist::App;

/// The next apps to fetch, at most `budget`: those never fetched, whose
/// store page changed since, or stored without their requirement text,
/// popular ones first (by ProtonDB report count),
/// then the most recently changed.
pub fn plan(
    apps: &[App],
    db: &GameDb,
    popularity: &HashMap<u32, u32>,
    budget: usize,
) -> Result<Vec<u32>, Error> {
    let fetched = db.fetched()?;
    let without_text = db.without_requirement_text()?;
    let mut candidates: Vec<&App> = apps
        .iter()
        .filter(|app| {
            without_text.contains(&app.app_id)
                || fetched
                    .get(&app.app_id)
                    .is_none_or(|&at| app.last_modified > at)
        })
        .collect();
    candidates.sort_by_key(|app| {
        let reports = popularity.get(&app.app_id).copied().unwrap_or(0);
        (Reverse(reports), Reverse(app.last_modified))
    });
    Ok(candidates
        .into_iter()
        .take(budget)
        .map(|app| app.app_id)
        .collect())
}
