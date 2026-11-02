//! The HTTP client: every request and redirect checked against an
//! [`Allowlist`], paced by a [`RateLimiter`], retried per [`Retry`].

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use crate::{Allowlist, RateLimiter, Retry};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

const USER_AGENT: &str = concat!(
    "rigcheck/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Glory42/rigcheck)"
);

/// Long enough for the 50 MB games.db budget on a slow (~0.5 Mbit/s) line.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct Client {
    http: reqwest::blocking::Client,
    allowlist: Allowlist,
    limiter: RateLimiter,
    retry: Retry,
    started: Instant,
}

impl Client {
    pub fn new(allowlist: Allowlist, limiter: RateLimiter, retry: Retry) -> Result<Client, Error> {
        let redirects = allowlist.clone();
        let http = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            // A redirect to a host off the list is refused like any request.
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() > 5 {
                    attempt.error("too many redirects")
                } else if redirects.allows(attempt.url().as_str()) {
                    attempt.follow()
                } else {
                    attempt.error("redirect to a host that isn't allowed")
                }
            }))
            .build()?;
        Ok(Client {
            http,
            allowlist,
            limiter,
            retry,
            started: Instant::now(),
        })
    }

    /// The body of `url`. Errors never include the URL, which can carry an
    /// API key and end up in CI logs.
    pub fn get_text(&mut self, url: &str) -> Result<String, Error> {
        Ok(self.get(url, None)?.text().map_err(|e| e.without_url())?)
    }

    /// Streams the body of `url` into `to`, calling `progress` with the bytes
    /// written so far after each chunk. Returns the total.
    pub fn download(
        &mut self,
        url: &str,
        to: &mut impl Write,
        mut progress: impl FnMut(u64),
    ) -> Result<u64, Error> {
        let mut response = self.get(url, Some(DOWNLOAD_TIMEOUT))?;
        let mut buffer = vec![0; 64 * 1024];
        let mut done = 0;
        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 {
                return Ok(done);
            }
            to.write_all(&buffer[..read])?;
            done += read as u64;
            progress(done);
        }
    }

    /// A successful response for `url`, after any retries. `timeout` covers
    /// the whole request including reading the body (reqwest's blocking
    /// client defaults to 30 seconds).
    fn get(
        &mut self,
        url: &str,
        timeout: Option<Duration>,
    ) -> Result<reqwest::blocking::Response, Error> {
        if !self.allowlist.allows(url) {
            return Err("host isn't on the allowlist".into());
        }
        for attempt in 1.. {
            std::thread::sleep(self.limiter.wait(self.started.elapsed()));
            let mut request = self.http.get(url);
            if let Some(timeout) = timeout {
                request = request.timeout(timeout);
            }
            let response = request.send().map_err(|e| e.without_url())?;
            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }
            match self.retry.after(status.as_u16(), attempt) {
                Some(wait) => std::thread::sleep(wait),
                None => return Err(format!("HTTP {status}").into()),
            }
        }
        unreachable!("the loop only ends by returning")
    }
}
