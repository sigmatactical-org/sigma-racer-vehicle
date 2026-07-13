//! SocketCAN input (and optional demo frame injection on vcan).

use crate::can_log::CanLogger;
use crate::sim::Simulator;
use sigma_racer_telemetry::can::{decode_frame, encode_sim_frames};
use sigma_racer_telemetry::state::VehicleState;
use socketcan::frame::CanDataFrame;
use socketcan::{CanFrame, CanSocket, EmbeddedFrame, Frame, Socket};
use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub struct CanBus {
    socket: CanSocket,
    last_frame_at: Option<Instant>,
    _demo: Option<DemoInjector>,
}

struct DemoInjector {
    stop: Arc<AtomicBool>,
    _handle: thread::JoinHandle<()>,
}

impl CanBus {
    pub fn open(iface: &str, demo: bool) -> Result<Self, String> {
        let socket =
            CanSocket::open(iface).map_err(|err| format!("open CAN interface {iface}: {err}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|err| format!("CAN nonblocking {iface}: {err}"))?;

        let demo_injector = if demo {
            Some(DemoInjector::spawn(iface)?)
        } else {
            None
        };

        eprintln!(
            "sigma-racer-vehicle: SocketCAN on {iface}{}",
            if demo { " (demo injector)" } else { "" }
        );

        Ok(Self {
            socket,
            last_frame_at: None,
            _demo: demo_injector,
        })
    }

    pub fn poll(&mut self, state: &mut VehicleState, logger: &mut Option<CanLogger>) {
        loop {
            match self.socket.read_frame() {
                Ok(CanFrame::Data(frame)) => self.handle_data_frame(&frame, state, logger),
                Ok(_) => continue,
                Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                Err(err) => {
                    eprintln!("sigma-racer-vehicle: CAN read: {err}");
                    break;
                }
            }
        }
    }

    pub fn signals_live(&self) -> bool {
        self.last_frame_at
            .map(|t| t.elapsed() < Duration::from_millis(500))
            .unwrap_or(false)
    }

    fn handle_data_frame(
        &mut self,
        frame: &CanDataFrame,
        state: &mut VehicleState,
        logger: &mut Option<CanLogger>,
    ) {
        let id = frame.raw_id();
        let data = frame.data();
        let len = data.len().min(8);
        let mut payload = [0u8; 8];
        payload[..len].copy_from_slice(&data[..len]);
        if decode_frame(id, &payload[..len], state) {
            self.last_frame_at = Some(Instant::now());
            if let Some(log) = logger {
                log.log_frames(&[(id, payload)]);
            }
        } else {
            eprintln!("sigma-racer-vehicle: ignore undecodable CAN frame 0x{id:03X}");
        }
    }
}

impl DemoInjector {
    fn spawn(iface: &str) -> Result<Self, String> {
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

fn demo_loop(iface: &str, stop: Arc<AtomicBool>) {
    let socket = match CanSocket::open(iface) {
        Ok(socket) => socket,
        Err(err) => {
            eprintln!("sigma-racer-vehicle: CAN demo open {iface}: {err}");
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
                    eprintln!("sigma-racer-vehicle: CAN demo encode 0x{id:03X}");
                    continue;
                }
            };
            if let Err(err) = socket.write_frame(&frame) {
                eprintln!("sigma-racer-vehicle: CAN demo write: {err}");
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}
