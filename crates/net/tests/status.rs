//! A failed request says which status the server answered with, so callers
//! can tell "the key was refused" from "offline" without reading the text.

use rigcheck_net::{Error, HttpStatus, status};

#[test]
fn an_error_status_can_be_read_back_from_the_error() {
    let refused: Error = HttpStatus(401).into();
    let offline: Error = "connection refused".into();

    assert_eq!(status(&refused), Some(401));
    assert_eq!(status(&offline), None);
}

#[test]
fn an_error_status_reads_like_http() {
    assert_eq!(HttpStatus(403).to_string(), "HTTP 403 Forbidden");
    assert_eq!(HttpStatus(599).to_string(), "HTTP 599");
}
