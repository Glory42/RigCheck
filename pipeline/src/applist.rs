//! Steam's list of every game, from `IStoreService/GetAppList`.

use serde::Deserialize;

use crate::Error;

/// A game on Steam, as the app list names it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct App {
    #[serde(rename = "appid")]
    pub app_id: u32,
    pub name: String,
    /// When the store page last changed, as a Unix timestamp.
    #[serde(default)]
    pub last_modified: i64,
}

/// Reads every page. `fetch_page` gets the last app ID seen (0 for the first
/// page) and returns that page's JSON.
pub fn collect(
    mut fetch_page: impl FnMut(u32) -> Result<String, Error>,
) -> Result<Vec<App>, Error> {
    #[derive(Deserialize)]
    struct Page {
        response: Response,
    }
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        apps: Vec<App>,
        #[serde(default)]
        have_more_results: bool,
        #[serde(default)]
        last_appid: u32,
    }

    let mut apps = Vec::new();
    let mut last_appid = 0;
    loop {
        let page: Page = serde_json::from_str(&fetch_page(last_appid)?)?;
        apps.extend(page.response.apps);
        if !page.response.have_more_results {
            return Ok(apps);
        }
        last_appid = page.response.last_appid;
    }
}
