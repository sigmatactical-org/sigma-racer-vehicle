//! Connected telemetry clients.
//!
//! Each client is written to in non-blocking mode with a bounded outbound
//! buffer. A slow or stalled consumer only grows its own buffer up to a cap;
//! once the cap is exceeded that client is dropped rather than blocking the
//! whole telemetry loop (which would stall every other consumer).

use std::collections::VecDeque;
use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixStream;

/// Maximum number of bytes we will buffer for a single slow client before
/// giving up on it. A backlog this large means the consumer is not keeping up.
const MAX_CLIENT_BACKLOG: usize = 256 * 1024;

struct Client {
    stream: UnixStream,
    /// Bytes not yet accepted by the kernel socket buffer.
    pending: VecDeque<u8>,
}

impl Client {
    /// Queue bytes and attempt to flush. Returns `false` if the client should
    /// be dropped (fatal error or backlog exceeded).
    fn enqueue_and_flush(&mut self, bytes: &[u8]) -> bool {
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

pub struct Broadcaster {
    clients: Vec<Client>,
}

impl Broadcaster {
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    pub fn add(&mut self, stream: UnixStream, initial: String) {
        if stream.set_nonblocking(true).is_err() {
            return;
        }
        let mut client = Client {
            stream,
            pending: VecDeque::new(),
        };
        if client.enqueue_and_flush(with_newline(initial).as_bytes()) {
            self.clients.push(client);
        }
    }

    pub fn send(&mut self, line: String) {
        let bytes = with_newline(line);
        self.clients
            .retain_mut(|client| client.enqueue_and_flush(bytes.as_bytes()));
    }
}

fn with_newline(mut line: String) -> String {
    line.push('\n');
    line
}
