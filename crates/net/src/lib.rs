//! The only crate allowed to make network requests. Every request goes to a
//! host on an [`Allowlist`] (ROADMAP §6); `deny.toml` keeps HTTP clients out
//! of other crates.

mod limit;

use url::Url;

pub use limit::{RateLimiter, Retry};

/// The hosts a program may send requests to, over HTTPS only.
#[derive(Debug, Clone)]
pub struct Allowlist {
    hosts: &'static [&'static str],
}

impl Allowlist {
    /// The desktop app: GitHub Releases (games.db), Steam's store API (live
    /// lookups) and Steam's image CDN. Nothing else, ever (ROADMAP §6).
    pub fn app() -> Allowlist {
        Allowlist {
            hosts: &[
                "github.com",
                "store.steampowered.com",
                "shared.akamai.steamstatic.com",
            ],
        }
    }

    /// The data pipeline: the app's hosts plus its data sources (Steam Web
    /// API for the app list, GitHub files for ProtonDB and AreWeAntiCheatYet).
    pub fn pipeline() -> Allowlist {
        Allowlist {
            hosts: &[
                "github.com",
                "store.steampowered.com",
                "shared.akamai.steamstatic.com",
                "api.steampowered.com",
                "raw.githubusercontent.com",
            ],
        }
    }

    /// Whether a request to `url` is allowed.
    pub fn allows(&self, url: &str) -> bool {
        let Ok(url) = Url::parse(url) else {
            return false;
        };
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| self.hosts.contains(&host))
    }
}
