//! Sigma Racer vehicle daemon — CAN → VSS → Unix socket telemetry.

#![forbid(unsafe_code)]

mod broadcast;
#[cfg(feature = "can-socket")]
mod can_bus;
mod can_log;
mod env;
mod log;
#[cfg(feature = "rpmsg")]
mod rpmsg_bus;
mod sim;
mod source;

use broadcast::Broadcaster;
use env::{flag, var_or};
use log::log;
use sigma_racer_telemetry::anomaly::AnomalyEngine;
use sigma_racer_telemetry::availability::AvailabilityTracker;
use sigma_racer_telemetry::protocol::{Message, SNAPSHOT_INTERVAL_MS, SOCKET_PATH, diff_vss};
use sigma_racer_telemetry::socket::bind_listener;
use sigma_racer_telemetry::state::VehicleState;
use source::SignalSource;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Entry point: run the daemon and exit non-zero on a startup error.
fn main() {
    if let Err(err) = run() {
        log!("{err}");
        std::process::exit(1);
    }
}

/// Main loop: sample the signal source at 50 ms, diff against the previous
/// state, and broadcast updates/snapshots/heartbeats to connected clients.
fn run() -> Result<(), String> {
    let socket_path = var_or("TELEMETRY_SOCKET", SOCKET_PATH);
    let demo = flag("VEHICLE_DEMO");
    let (mut source, mut can_logger) = SignalSource::open(demo)?;
    let mut state = VehicleState::idle();
    let mut tracker = AvailabilityTracker::sigma_default();
    source.apply_to(
        &mut state,
        &mut can_logger,
        &mut tracker,
        chrono::Utc::now().timestamp_millis(),
    );

    let listener = bind_listener(Path::new(&socket_path))
        .map_err(|err| format!("bind {socket_path}: {err}"))?;

    let mut broadcaster = Broadcaster::new();
    let started = Instant::now();
    let mut seq: u64 = 0;
    let mut prev = state.clone();
    let mut prev_avail = tracker.stale_paths(chrono::Utc::now().timestamp_millis());
    let mut sample_at = Instant::now();
    let mut snapshot_at = Instant::now();
    let mut heartbeat_at = Instant::now();
    // Observe-only anomaly detection: raises/clears travel as Event messages.
    // The daemon never actuates anything; protective action stays on the M7.
    let mut anomalies = AnomalyEngine::sigma_defaults();

    log!("listening on {socket_path} (source={})", source.name());

    loop {
        accept_clients(&listener, &mut broadcaster, &mut seq, &state);

        if sample_at.elapsed() >= Duration::from_millis(50) {
            source.step(Duration::from_millis(50));
            // Clock captured once at the loop boundary; detectors are pure.
            let ts_ms = chrono::Utc::now().timestamp_millis();
            source.apply_to(&mut state, &mut can_logger, &mut tracker, ts_ms);

            let patch = diff_vss(&prev, &state);
            let avail = tracker.stale_paths(ts_ms);
            // A signal can go stale without its value changing (so the diff is
            // empty), so emit on an availability change too — otherwise the
            // cluster only learns the bus died at the next periodic snapshot.
            let avail_changed = avail != prev_avail;
            if !patch.is_empty() || avail_changed {
                seq += 1;
                broadcaster.send(
                    Message::signal_update(seq, patch)
                        .with_avail(avail.clone())
                        .to_line(),
                );
                prev = state.clone();
                prev_avail = avail;
                snapshot_at = Instant::now();
            } else if snapshot_at.elapsed() >= Duration::from_millis(SNAPSHOT_INTERVAL_MS) {
                seq += 1;
                broadcaster.send(Message::snapshot(seq, &state).with_avail(avail.clone()).to_line());
                prev_avail = avail;
                snapshot_at = Instant::now();
            }

            for ev in anomalies.observe(ts_ms, &state) {
                seq += 1;
                broadcaster.send(ev.to_message(seq).to_line());
                log!("anomaly {} {:?}: {}", ev.id, ev.edge, ev.message);
            }
            sample_at = Instant::now();
        }

        if heartbeat_at.elapsed() >= Duration::from_secs(1) {
            seq += 1;
            broadcaster
                .send(Message::heartbeat(seq, started.elapsed().as_millis() as u64).to_line());
            // Silence watchdog: catches a wedged source, not just stale flags.
            let ts_ms = chrono::Utc::now().timestamp_millis();
            for ev in anomalies.tick(ts_ms) {
                seq += 1;
                broadcaster.send(ev.to_message(seq).to_line());
                log!("anomaly {} {:?}: {}", ev.id, ev.edge, ev.message);
            }
            heartbeat_at = Instant::now();
        }

        thread::sleep(Duration::from_millis(5));
    }
}

/// Accept every pending client connection and greet each with a snapshot.
fn accept_clients(
    listener: &std::os::unix::net::UnixListener,
    broadcaster: &mut Broadcaster,
    seq: &mut u64,
    state: &VehicleState,
) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                *seq += 1;
                let snap = Message::snapshot(*seq, state);
                broadcaster.add(stream, snap.to_line());
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => {
                log!("accept: {err}");
                break;
            }
        }
    }
}
