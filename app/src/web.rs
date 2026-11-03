//! Every request the app makes (ROADMAP §6), in one place. [`Web`] writes
//! the URLs and says how a request failed; a [`Transport`] sends them:
//! [`Net`] in the app, a fake in tests. Requests block, so pages run them
//! through [`background`].

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::Task;
use iced::futures::channel::oneshot;
use rigcheck_net::{Allowlist, Client, RateLimiter, Retry};

/// Published with every games.db build (ROADMAP §7.3).
const MANIFEST_URL: &str =
    "https://github.com/Glory42/RigCheck/releases/download/db-latest/manifest.json";
/// Uploaded with every release (packaging/package.sh). `latest` skips
/// prereleases, so the daily `db-latest` games.db release never counts.
const VERSION_URL: &str =
    "https://github.com/Glory42/rigcheck/releases/latest/download/version.txt";

/// Why a request got no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The server answered 401 or 403: it doesn't accept the key.
    Rejected,
    /// Offline, the server failed, or anything else. The text is for the
    /// log; it never holds the URL, which can carry a key.
    Failed(String),
}

impl From<rigcheck_net::Error> for Failure {
    fn from(error: rigcheck_net::Error) -> Failure {
        match rigcheck_net::status(&error) {
            Some(401 | 403) => Failure::Rejected,
            _ => Failure::Failed(error.to_string()),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Rejected => f.write_str("the key was refused"),
            Failure::Failed(why) => f.write_str(why),
        }
    }
}

/// Sends requests. Every method blocks until the answer is in.
pub trait Transport: Send + Sync {
    /// The body of `url`.
    fn text(&self, url: &str) -> Result<String, Failure>;
    /// The body of `url`, as bytes.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Failure>;
    /// Streams a large body into `to`, calling `progress` with the bytes
    /// written so far. Returns the total.
    fn download(
        &self,
        url: &str,
        to: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<u64, Failure>;
}

/// What the app asks for, and where. Cheap to clone: every clone shares one
/// transport.
#[derive(Clone)]
pub struct Web(Arc<dyn Transport>);

impl Web {
    /// The real network, through `rigcheck_net`.
    pub fn net() -> Web {
        Web(Arc::new(Net::default()))
    }

    #[cfg(test)]
    pub fn with(transport: impl Transport + 'static) -> Web {
        Web(Arc::new(transport))
    }

    /// Steam's store page data for `app_id` (`appdetails` JSON).
    pub fn appdetails(&self, app_id: u32) -> Result<String, Failure> {
        self.0.text(&format!(
            "https://store.steampowered.com/api/appdetails?appids={app_id}&l=english&cc=us"
        ))
    }

    /// Every game `steam_id` owns (`GetOwnedGames` JSON), asked with the
    /// user's own Steam Web API `key`.
    pub fn owned_games(&self, key: &str, steam_id: u64) -> Result<String, Failure> {
        self.0.text(&format!(
            "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?key={key}&steamid={steam_id}&include_appinfo=1&include_played_free_games=1&format=json"
        ))
    }

    /// Steam's 600×900 library art for `app_id`, the same image Steam caches.
    pub fn cover(&self, app_id: u32) -> Result<Vec<u8>, Failure> {
        self.0.bytes(&format!(
            "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{app_id}/library_600x900.jpg"
        ))
    }

    /// The latest rigcheck release's version (`version.txt`).
    pub fn latest_version(&self) -> Result<String, Failure> {
        self.0.text(VERSION_URL)
    }

    /// The published games.db's `manifest.json`.
    pub fn manifest(&self) -> Result<String, Failure> {
        self.0.text(MANIFEST_URL)
    }

    /// Streams `url` (a manifest's games.db) into `to`.
    pub fn download(
        &self,
        url: &str,
        to: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<u64, Failure> {
        self.0.download(url, to, progress)
    }
}

/// Runs blocking `work` on its own thread; the task ends with its result.
/// Nothing runs until iced runs the task (tests drop it unrun, so they
/// never reach the network).
pub fn background<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Task<Result<T, Failure>> {
    Task::perform(
        async move {
            let (sender, receiver) = oneshot::channel();
            std::thread::spawn(move || {
                let _ = sender.send(work());
            });
            receiver
                .await
                .unwrap_or_else(|_| Err(Failure::Failed("the request stopped".to_owned())))
        },
        |result| result,
    )
}

/// The network, through `rigcheck_net`: one client per host, built on first
/// use, so each host's rate limit sees every request to it.
#[derive(Default)]
pub struct Net {
    clients: Mutex<HashMap<String, Arc<Mutex<Client>>>>,
}

impl Net {
    /// The client for `url`'s host.
    fn client(&self, url: &str) -> Result<Arc<Mutex<Client>>, Failure> {
        let host = host(url);
        let mut clients = self.clients.lock().map_err(|_| broke())?;
        if let Some(client) = clients.get(host) {
            return Ok(client.clone());
        }
        let client = Arc::new(Mutex::new(new_client(host)?));
        clients.insert(host.to_owned(), client.clone());
        Ok(client)
    }
}

impl Transport for Net {
    fn text(&self, url: &str) -> Result<String, Failure> {
        let client = self.client(url)?;
        let mut client = client.lock().map_err(|_| broke())?;
        Ok(client.get_text(url)?)
    }

    fn bytes(&self, url: &str) -> Result<Vec<u8>, Failure> {
        let client = self.client(url)?;
        let mut client = client.lock().map_err(|_| broke())?;
        let mut body = Vec::new();
        client.download(url, &mut body, |_| {})?;
        Ok(body)
    }

    /// On a client of its own: a download takes minutes, and holding the
    /// host's shared client that long would stall its other requests. Two
    /// requests in all, so the separate rate limit doesn't matter.
    fn download(
        &self,
        url: &str,
        mut to: &mut dyn Write,
        progress: &mut dyn FnMut(u64),
    ) -> Result<u64, Failure> {
        Ok(new_client(host(url))?.download(url, &mut to, progress)?)
    }
}

/// A client paced for `host`.
fn new_client(host: &str) -> Result<Client, Failure> {
    let (limiter, retry) = match host {
        // Well under Steam's ~200 requests per 5 minutes, even for someone
        // opening game after game.
        "store.steampowered.com" => (
            RateLimiter::new(20, Duration::from_secs(60)),
            Retry::new(1, Duration::from_secs(5)),
        ),
        // A CDN, so faster than the store API; still paced, since a big
        // library asks for hundreds of covers at once.
        "shared.akamai.steamstatic.com" => (
            RateLimiter::new(10, Duration::from_secs(1)),
            Retry::new(1, Duration::from_secs(2)),
        ),
        // GitHub and the Steam Web API: a few requests a session.
        _ => (
            RateLimiter::new(5, Duration::from_secs(1)),
            Retry::new(2, Duration::from_secs(2)),
        ),
    };
    Ok(Client::new(Allowlist::app(), limiter, retry)?)
}

/// "store.steampowered.com" for "https://store.steampowered.com/api/…".
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?']).next().unwrap_or(rest)
}

fn broke() -> Failure {
    Failure::Failed("the client broke".to_owned())
}

#[cfg(test)]
pub mod fake {
    //! A [`Transport`] that answers every request with the same reply and
    //! remembers what was asked.

    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::{Failure, Transport};

    #[derive(Clone)]
    pub struct Fake {
        reply: Result<String, Failure>,
        pub asked: Arc<Mutex<Vec<String>>>,
    }

    impl Fake {
        pub fn replying(reply: Result<&str, Failure>) -> Fake {
            Fake {
                reply: reply.map(str::to_owned),
                asked: Arc::default(),
            }
        }

        /// The URLs asked for, in order.
        pub fn asked(&self) -> Vec<String> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl Transport for Fake {
        fn text(&self, url: &str) -> Result<String, Failure> {
            self.asked.lock().unwrap().push(url.to_owned());
            self.reply.clone()
        }

        fn bytes(&self, url: &str) -> Result<Vec<u8>, Failure> {
            self.text(url).map(String::into_bytes)
        }

        fn download(
            &self,
            url: &str,
            to: &mut dyn Write,
            progress: &mut dyn FnMut(u64),
        ) -> Result<u64, Failure> {
            let body = self.bytes(url)?;
            to.write_all(&body)
                .map_err(|error| Failure::Failed(error.to_string()))?;
            progress(body.len() as u64);
            Ok(body.len() as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::Fake;
    use super::*;

    #[test]
    fn every_request_goes_to_an_allowed_host() {
        let fake = Fake::replying(Ok("{}"));
        let web = Web::with(fake.clone());

        let _ = web.appdetails(1086940);
        let _ = web.owned_games("0123456789ABCDEF0123456789ABCDEF", 76561198388197563);
        let _ = web.cover(1086940);
        let _ = web.latest_version();
        let _ = web.manifest();

        let app = Allowlist::app();
        for url in fake.asked() {
            assert!(app.allows(&url), "{url}");
        }
    }

    #[test]
    fn steam_is_asked_about_a_game_in_english_with_us_prices() {
        let fake = Fake::replying(Ok("{}"));

        let _ = Web::with(fake.clone()).appdetails(1086940);

        assert_eq!(
            fake.asked(),
            ["https://store.steampowered.com/api/appdetails?appids=1086940&l=english&cc=us"]
        );
    }

    #[test]
    fn the_owned_games_are_asked_for_with_the_key_and_the_signed_in_account() {
        let fake = Fake::replying(Ok("{}"));

        let _ = Web::with(fake.clone()).owned_games("KEY", 76561198388197563);

        let [url] = fake.asked().try_into().unwrap();
        assert!(url.starts_with("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?"));
        assert!(url.contains("key=KEY&steamid=76561198388197563"));
        assert!(
            url.contains("include_appinfo=1"),
            "names come with the list"
        );
    }

    #[test]
    fn a_refused_key_is_told_apart_from_being_offline() {
        let refused: rigcheck_net::Error = rigcheck_net::HttpStatus(401).into();
        let forbidden: rigcheck_net::Error = rigcheck_net::HttpStatus(403).into();
        let down: rigcheck_net::Error = rigcheck_net::HttpStatus(503).into();
        let offline: rigcheck_net::Error = "connection refused".into();

        assert_eq!(Failure::from(refused), Failure::Rejected);
        assert_eq!(Failure::from(forbidden), Failure::Rejected);
        assert_eq!(
            Failure::from(down),
            Failure::Failed("HTTP 503 Service Unavailable".to_owned())
        );
        assert_eq!(
            Failure::from(offline),
            Failure::Failed("connection refused".to_owned())
        );
    }

    #[test]
    fn requests_are_paced_per_host() {
        assert_eq!(
            host("https://store.steampowered.com/api/appdetails?appids=1"),
            "store.steampowered.com"
        );
        assert_eq!(host("https://github.com"), "github.com");
        assert_eq!(
            host("https://api.steampowered.com?key=1"),
            "api.steampowered.com"
        );
    }
}
