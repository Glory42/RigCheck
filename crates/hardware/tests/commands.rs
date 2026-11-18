//! Running the programs detection reads (`vulkaninfo`, `nvidia-smi`): a
//! hung driver mustn't hang detection with it.

use std::time::{Duration, Instant};

use rigcheck_hardware::run_with_timeout;

#[test]
fn returns_what_the_program_prints() {
    let out = run_with_timeout("echo", &["Mesa"], Duration::from_secs(10));

    assert_eq!(out.as_deref(), Some("Mesa\n"));
}

#[test]
fn a_program_that_fails_returns_nothing() {
    assert_eq!(
        run_with_timeout("false", &[], Duration::from_secs(10)),
        None
    );
    assert_eq!(
        run_with_timeout("no-such-program-rigcheck", &[], Duration::from_secs(10)),
        None
    );
}

#[test]
fn a_program_that_hangs_is_given_up_on() {
    let started = Instant::now();

    let out = run_with_timeout("sleep", &["30"], Duration::from_millis(200));

    assert_eq!(out, None);
    assert!(started.elapsed() < Duration::from_secs(5));
}
