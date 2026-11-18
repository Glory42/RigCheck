//! Deciding which apps to fetch next (ROADMAP Phase 4, §9).

use std::cmp::Reverse;
use std::collections::HashMap;

use rigcheck_gamedb::GameDb;

use crate::Error;
use crate::applist::App;

const DAY: i64 = 24 * 60 * 60;
/// How long a changed store page waits before it's fetched again, for a game
/// with ProtonDB reports (people play it on Linux) and for the rest. Some
/// pages change daily; without a wait they'd take the budget that games
/// never fetched need.
const COOLDOWN_PLAYED: i64 = 7 * DAY;
const COOLDOWN_OTHER: i64 = 30 * DAY;

/// The next apps to fetch at `now`, at most `budget`: those never fetched,
/// stored without their requirement text, or whose store page changed since
/// and has waited out its cooldown. Popular ones first (by ProtonDB report
/// count), then those never fetched, then the most recently changed.
pub fn plan(
    apps: &[App],
    db: &GameDb,
    popularity: &HashMap<u32, u32>,
    budget: usize,
    now: i64,
) -> Result<Vec<u32>, Error> {
    let fetched = db.fetched()?;
    let without_text = db.without_requirement_text()?;
    let reports = |app: &App| popularity.get(&app.app_id).copied().unwrap_or(0);
    let mut candidates: Vec<&App> = apps
        .iter()
        .filter(|app| {
            if without_text.contains(&app.app_id) {
                return true;
            }
            match fetched.get(&app.app_id) {
                None => true,
                Some(&at) => {
                    let cooldown = if reports(app) > 0 {
                        COOLDOWN_PLAYED
                    } else {
                        COOLDOWN_OTHER
                    };
                    app.last_modified > at && now - at >= cooldown
                }
            }
        })
        .collect();
    candidates.sort_by_key(|app| {
        let again = fetched.contains_key(&app.app_id);
        (Reverse(reports(app)), again, Reverse(app.last_modified))
    });
    Ok(candidates
        .into_iter()
        .take(budget)
        .map(|app| app.app_id)
        .collect())
}
