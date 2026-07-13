//! Sigma Racer vehicle daemon — CAN → VSS → Unix socket telemetry.

mod broadcast;
#[cfg(feature = "can-socket")]
mod can_bus;
mod can_log;
mod env;
#[cfg(feature = "rpmsg")]
mod rpmsg_bus;
mod sim;
mod source;

use broadcast::Broadcaster;
use env::{flag, var_or};
use sigma_racer_telemetry::protocol::{Message, SNAPSHOT_INTERVAL_MS, SOCKET_PATH, diff_vss};
use sigma_racer_telemetry::socket::bind_listener;
use sigma_racer_telemetry::state::VehicleState;
use source::SignalSource;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    if let Err(err) = run() {
        eprintln!("sigma-racer-vehicle: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let socket_path = var_or("TELEMETRY_SOCKET", SOCKET_PATH);
    let demo = flag("VEHICLE_DEMO");
    let (mut source, mut can_logger) = SignalSource::open(demo)?;
    let mut state = VehicleState::idle();
    source.apply_to(&mut state, &mut can_logger);

    let listener = bind_listener(Path::new(&socket_path))
        .map_err(|err| format!("bind {socket_path}: {err}"))?;

    let mut broadcaster = Broadcaster::new();
    let started = Instant::now();
    let mut seq: u64 = 0;
    let mut prev = state.clone();
    let mut sample_at = Instant::now();
    let mut snapshot_at = Instant::now();
    let mut heartbeat_at = Instant::now();

    eprintln!(
        "sigma-racer-vehicle: listening on {socket_path} (source={})",
        source.name()
    );

    loop {
        accept_clients(&listener, &mut broadcaster, &mut seq, &state);

        if sample_at.elapsed() >= Duration::from_millis(50) {
            source.step(Duration::from_millis(50));
            source.apply_to(&mut state, &mut can_logger);

            let patch = diff_vss(&prev, &state);
            if !patch.is_empty() {
                seq += 1;
                broadcaster.send(Message::signal_update(seq, patch).to_line());
                prev = state.clone();
                snapshot_at = Instant::now();
            } else if snapshot_at.elapsed() >= Duration::from_millis(SNAPSHOT_INTERVAL_MS) {
                seq += 1;
                broadcaster.send(Message::snapshot(seq, &state).to_line());
                snapshot_at = Instant::now();
            }
            sample_at = Instant::now();
        }

        if heartbeat_at.elapsed() >= Duration::from_secs(1) {
            seq += 1;
            broadcaster
                .send(Message::heartbeat(seq, started.elapsed().as_millis() as u64).to_line());
            heartbeat_at = Instant::now();
        }

        thread::sleep(Duration::from_millis(5));
    }
}

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
                eprintln!("sigma-racer-vehicle: accept: {err}");
                break;
            }
        }
    }
}
