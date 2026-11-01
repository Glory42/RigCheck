//! Steam Deck compatibility from the store's (unofficial) Deck report.

use serde::Deserialize;

use crate::Error;

/// verified | playable | unsupported | unknown, from a response of
/// `saleaction/ajaxgetdeckappcompatibilityreport`.
pub fn status(json: &str) -> Result<&'static str, Error> {
    #[derive(Deserialize)]
    struct Response {
        results: Option<Results>,
    }
    #[derive(Deserialize)]
    struct Results {
        resolved_category: u8,
    }

    let response: Response = serde_json::from_str(json)?;
    Ok(match response.results.map(|r| r.resolved_category) {
        Some(3) => "verified",
        Some(2) => "playable",
        Some(1) => "unsupported",
        _ => "unknown",
    })
}
