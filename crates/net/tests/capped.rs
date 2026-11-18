//! A response body is written through [`Capped`], so a server (or a
//! tampered release) can't fill the disk or the memory.

use std::io::Write;

use rigcheck_net::Capped;

#[test]
fn writes_up_to_the_limit() {
    let mut body = Capped::new(Vec::new(), 8);

    body.write_all(b"12345678").unwrap();

    assert_eq!(body.into_inner(), b"12345678");
}

#[test]
fn refuses_a_byte_past_the_limit() {
    let mut body = Capped::new(Vec::new(), 8);
    body.write_all(b"1234").unwrap();

    let error = body.write_all(b"56789").unwrap_err();

    assert_eq!(error.to_string(), "the response is bigger than 8 bytes");
}
