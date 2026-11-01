//! Staying under a server's request limit.

use std::collections::VecDeque;
use std::time::Duration;

/// At most `max` requests in any `window`. Time is passed in (time since
/// start), so callers own the clock and tests can fake it.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    max: usize,
    window: Duration,
    /// When the requests still inside the window were sent.
    sent: VecDeque<Duration>,
}

impl RateLimiter {
    pub fn new(max: usize, window: Duration) -> RateLimiter {
        RateLimiter {
            max,
            window,
            sent: VecDeque::with_capacity(max),
        }
    }

    /// How long to wait before sending a request at `now`. The request is
    /// counted as sent once that wait is over.
    pub fn wait(&mut self, now: Duration) -> Duration {
        let wait = if self.sent.len() < self.max {
            Duration::ZERO
        } else {
            let oldest = self.sent.pop_front().expect("the window is full");
            (oldest + self.window).saturating_sub(now)
        };
        self.sent.push_back(now + wait);
        wait
    }
}

/// When to try a failed request again: rate limits (429) and server errors
/// (5xx) are retried with doubling waits; other failures are final.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    max_retries: u32,
    first_wait: Duration,
}

impl Retry {
    pub fn new(max_retries: u32, first_wait: Duration) -> Retry {
        Retry {
            max_retries,
            first_wait,
        }
    }

    /// How long to wait after the `attempt`-th failure with `status`, or
    /// `None` to give up.
    pub fn after(&self, status: u16, attempt: u32) -> Option<Duration> {
        let retryable = status == 429 || (500..600).contains(&status);
        (retryable && attempt <= self.max_retries).then(|| self.first_wait * 2u32.pow(attempt - 1))
    }
}
