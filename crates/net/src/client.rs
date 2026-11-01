//! The HTTP client: every request and redirect checked against an
//! [`Allowlist`], paced by a [`RateLimiter`], retried per [`Retry`].

use std::time::Instant;

use crate::{Allowlist, RateLimiter, Retry};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

const USER_AGENT: &str = concat!(
    "rigcheck/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Glory42/rigcheck)"
);

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
        if !self.allowlist.allows(url) {
            return Err("host isn't on the allowlist".into());
        }
        for attempt in 1.. {
            std::thread::sleep(self.limiter.wait(self.started.elapsed()));
            let response = self.http.get(url).send().map_err(|e| e.without_url())?;
            let status = response.status();
            if status.is_success() {
                return Ok(response.text().map_err(|e| e.without_url())?);
            }
            match self.retry.after(status.as_u16(), attempt) {
                Some(wait) => std::thread::sleep(wait),
                None => return Err(format!("HTTP {status}").into()),
            }
        }
        unreachable!("the loop only ends by returning")
    }
}
