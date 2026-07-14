//! Background thread that injects simulated CAN frames onto a (v)can bus.

use crate::log::log;
use crate::sim::Simulator;
use sigma_racer_telemetry::can::encode_sim_frames;
use sigma_racer_telemetry::state::VehicleState;
use socketcan::{CanFrame, CanSocket, Frame, Socket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

/// Handle to the demo injection thread; the thread is asked to stop when the
/// handle is dropped (it exits at its next 50 ms tick).
pub(super) struct DemoInjector {
    stop: Arc<AtomicBool>,
    _handle: thread::JoinHandle<()>,
}

impl DemoInjector {
    /// Spawn the injector thread writing simulated frames to `iface`.
    pub(super) fn spawn(iface: &str) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let iface = iface.to_owned();
        let handle = thread::Builder::new()
            .name("can-demo".into())
            .spawn(move || demo_loop(&iface, stop_flag))
            .map_err(|err| format!("spawn CAN demo injector: {err}"))?;
        Ok(Self {
            stop,
            _handle: handle,
        })
    }
}

impl Drop for DemoInjector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Encode the demo ride through the M7 codec and write it to the bus at the
/// same 50 ms cadence the real firmware uses.
fn demo_loop(iface: &str, stop: Arc<AtomicBool>) {
    let socket = match CanSocket::open(iface) {
        Ok(socket) => socket,
        Err(err) => {
            log!("CAN demo open {iface}: {err}");
            return;
        }
    };

    let mut sim = Simulator::new(true);
    while !stop.load(Ordering::Relaxed) {
        sim.step(Duration::from_millis(50));
        let mut state = VehicleState::idle();
        sim.apply_to(&mut state);
        for (id, payload) in encode_sim_frames(&state) {
            let frame = match CanFrame::from_raw_id(id, &payload) {
                Some(frame) => frame,
                None => {
                    log!("CAN demo encode 0x{id:03X}");
                    continue;
                }
            };
            if let Err(err) = socket.write_frame(&frame) {
                log!("CAN demo write: {err}");
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}
