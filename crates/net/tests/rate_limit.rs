//! Tests for staying under Steam's limits: a sliding-window rate limiter and
//! retry with backoff. Time is passed in, so tests control the clock.

use std::time::Duration;

use rigcheck_net::{RateLimiter, Retry};

const fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

#[test]
fn requests_wait_once_the_window_is_full() {
    let mut limiter = RateLimiter::new(3, secs(10));

    assert_eq!(limiter.wait(secs(0)), secs(0));
    assert_eq!(limiter.wait(secs(1)), secs(0));
    assert_eq!(limiter.wait(secs(2)), secs(0));
    // The first request leaves the window at 10 s.
    assert_eq!(limiter.wait(secs(4)), secs(6));
    // That waited request was sent at 10 s; the next slot opens at 11 s.
    assert_eq!(limiter.wait(secs(10)), secs(1));
}

#[test]
fn rate_limits_and_server_errors_are_retried_with_growing_waits() {
    let retry = Retry::new(3, secs(2));

    // `after(status, attempt)`: attempt is how many tries have failed so far.
    assert_eq!(retry.after(429, 1), Some(secs(2)));
    assert_eq!(retry.after(503, 2), Some(secs(4)));
    assert_eq!(retry.after(429, 3), Some(secs(8)));
    assert_eq!(retry.after(429, 4), None, "gives up after 3 retries");
}

#[test]
fn other_errors_are_not_retried() {
    let retry = Retry::new(3, secs(2));

    assert_eq!(retry.after(404, 1), None);
    assert_eq!(retry.after(403, 1), None);
}
