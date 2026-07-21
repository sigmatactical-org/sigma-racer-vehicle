//! SocketCAN input (and optional demo frame injection on vcan).

mod demo_injector;

use crate::can_log::CanLogger;
use crate::log::log;
use demo_injector::DemoInjector;
use sigma_racer_telemetry::availability::AvailabilityTracker;
use sigma_racer_telemetry::can::decode_frame;
use sigma_racer_telemetry::state::VehicleState;
use socketcan::frame::CanDataFrame;
use socketcan::{CanFrame, CanSocket, EmbeddedFrame, Frame, Socket};
use std::io::ErrorKind;
use std::time::{Duration, Instant};

/// Non-blocking SocketCAN reader that decodes M7 safety-bus frames into the
/// shared [`VehicleState`].
pub struct CanBus {
    socket: CanSocket,
    /// When the last decodable frame arrived; drives [`CanBus::signals_live`].
    last_frame_at: Option<Instant>,
    _demo: Option<DemoInjector>,
}

impl CanBus {
    /// Open `iface` in non-blocking mode, optionally spawning the demo
    /// injector that feeds the bus with simulated frames.
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

        log!(
            "SocketCAN on {iface}{}",
            if demo { " (demo injector)" } else { "" }
        );

        Ok(Self {
            socket,
            last_frame_at: None,
            _demo: demo_injector,
        })
    }

    /// Drain every frame currently queued on the socket into `state`,
    /// marking each decoded frame id on `tracker` at `now_ms`, and mirroring
    /// decoded frames to the optional MDF4 `logger`.
    pub fn poll(
        &mut self,
        state: &mut VehicleState,
        logger: &mut Option<CanLogger>,
        tracker: &mut AvailabilityTracker,
        now_ms: i64,
    ) {
        loop {
            match self.socket.read_frame() {
                Ok(CanFrame::Data(frame)) => {
                    self.handle_data_frame(&frame, state, logger, tracker, now_ms)
                }
                Ok(_) => continue,
                Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                Err(err) => {
                    log!("CAN read: {err}");
                    break;
                }
            }
        }
    }

    /// Whether a decodable frame arrived recently enough (500 ms) to consider
    /// the M7 signal source alive.
    pub fn signals_live(&self) -> bool {
        self.last_frame_at
            .map(|t| t.elapsed() < Duration::from_millis(500))
            .unwrap_or(false)
    }

    /// Decode one data frame into `state` and log it; unknown IDs are noted
    /// but otherwise ignored.
    fn handle_data_frame(
        &mut self,
        frame: &CanDataFrame,
        state: &mut VehicleState,
        logger: &mut Option<CanLogger>,
        tracker: &mut AvailabilityTracker,
        now_ms: i64,
    ) {
        let id = frame.raw_id();
        let data = frame.data();
        let len = data.len().min(8);
        let mut payload = [0u8; 8];
        payload[..len].copy_from_slice(&data[..len]);
        if decode_frame(id, &payload[..len], state) {
            self.last_frame_at = Some(Instant::now());
            tracker.mark(id, now_ms);
            if let Some(log) = logger {
                log.log_frames(&[(id, payload)]);
            }
        } else {
            log!("ignore undecodable CAN frame 0x{id:03X}");
        }
    }
}
