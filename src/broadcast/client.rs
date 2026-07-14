//! A single connected telemetry consumer.

use std::collections::VecDeque;
use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixStream;

/// Maximum number of bytes buffered for a single slow client before giving up
/// on it. A backlog this large means the consumer is not keeping up.
const MAX_CLIENT_BACKLOG: usize = 256 * 1024;

/// One connected client with its own bounded outbound buffer.
///
/// The stream is written in non-blocking mode; bytes the kernel does not
/// accept immediately are kept in [`Client::pending`] until the next flush.
pub(super) struct Client {
    stream: UnixStream,
    /// Bytes not yet accepted by the kernel socket buffer.
    pending: VecDeque<u8>,
}

impl Client {
    /// Wrap an accepted (already non-blocking) stream with an empty backlog.
    pub(super) fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            pending: VecDeque::new(),
        }
    }

    /// Queue bytes and attempt to flush. Returns `false` if the client should
    /// be dropped (fatal error or backlog exceeded).
    pub(super) fn enqueue_and_flush(&mut self, bytes: &[u8]) -> bool {
        self.pending.extend(bytes.iter().copied());
        self.flush()
    }

    /// Try to drain the pending buffer without blocking.
    fn flush(&mut self) -> bool {
        while !self.pending.is_empty() {
            let (head, _) = self.pending.as_slices();
            match self.stream.write(head) {
                Ok(0) => return false,
                Ok(n) => {
                    self.pending.drain(..n);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // Kernel buffer full; keep the backlog for the next tick
                    // unless the consumer has fallen too far behind.
                    return self.pending.len() <= MAX_CLIENT_BACKLOG;
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return false,
            }
        }
        true
    }
}
