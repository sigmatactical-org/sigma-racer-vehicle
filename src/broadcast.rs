//! Fan-out of telemetry lines to connected clients.
//!
//! Each client is written to in non-blocking mode with a bounded outbound
//! buffer. A slow or stalled consumer only grows its own buffer up to a cap;
//! once the cap is exceeded that client is dropped rather than blocking the
//! whole telemetry loop (which would stall every other consumer).

mod client;

use client::Client;
use std::os::unix::net::UnixStream;

/// Broadcasts newline-delimited telemetry messages to every connected client.
pub struct Broadcaster {
    clients: Vec<Client>,
}

impl Broadcaster {
    /// Create a broadcaster with no clients.
    pub fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }

    /// Adopt a freshly accepted stream and send it `initial` (a snapshot).
    ///
    /// The client is dropped on the spot if the stream cannot be switched to
    /// non-blocking mode or the initial write fails fatally.
    pub fn add(&mut self, stream: UnixStream, initial: String) {
        if stream.set_nonblocking(true).is_err() {
            return;
        }
        let mut client = Client::new(stream);
        if client.enqueue_and_flush(with_newline(initial).as_bytes()) {
            self.clients.push(client);
        }
    }

    /// Send one message line to every client, dropping any that fall behind.
    pub fn send(&mut self, line: String) {
        let bytes = with_newline(line);
        self.clients
            .retain_mut(|client| client.enqueue_and_flush(bytes.as_bytes()));
    }
}

/// Terminate a message with the protocol's line delimiter.
fn with_newline(mut line: String) -> String {
    line.push('\n');
    line
}
