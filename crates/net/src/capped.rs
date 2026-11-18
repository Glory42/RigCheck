//! A size limit on a response body.

use std::io::{self, Write};

/// Writes through to `inner` until `limit` bytes, then fails, so a server
/// (or a tampered release) can't fill the disk or the memory.
pub struct Capped<W> {
    inner: W,
    limit: u64,
    written: u64,
}

impl<W: Write> Capped<W> {
    pub fn new(inner: W, limit: u64) -> Capped<W> {
        Capped {
            inner,
            limit,
            written: 0,
        }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for Capped<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written + buf.len() as u64 > self.limit {
            return Err(io::Error::other(format!(
                "the response is bigger than {} bytes",
                self.limit
            )));
        }
        let written = self.inner.write(buf)?;
        self.written += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
