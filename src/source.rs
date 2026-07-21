//! CAN, RPMsg, or simulated signal input.

use crate::can_log::CanLogger;
use crate::env;
use crate::sim::Simulator;
use sigma_racer_telemetry::availability::AvailabilityTracker;
use sigma_racer_telemetry::can::encode_sim_frames;
use sigma_racer_telemetry::state::VehicleState;
use std::time::Duration;

#[cfg(feature = "can-socket")]
use crate::can_bus::CanBus;
#[cfg(feature = "rpmsg")]
use crate::rpmsg_bus::RpmsgBus;

/// The daemon's active signal input, selected by `VEHICLE_SOURCE`.
pub enum SignalSource {
    Sim(Simulator),
    #[cfg(feature = "can-socket")]
    Can(CanBus),
    #[cfg(feature = "rpmsg")]
    Rpmsg(RpmsgBus),
}

impl SignalSource {
    /// Open the source named by `VEHICLE_SOURCE` (default `sim`) together
    /// with the optional CAN logger.
    pub fn open(demo: bool) -> Result<(Self, Option<CanLogger>), String> {
        let source = env::var_or("VEHICLE_SOURCE", "sim");
        let logger = CanLogger::open();
        match source.as_str() {
            "sim" => Ok((Self::Sim(Simulator::new(demo)), logger)),
            "can" | "socketcan" => {
                #[cfg(feature = "can-socket")]
                {
                    let iface = env::var_or("CAN_IFACE", "can0");
                    Ok((Self::Can(CanBus::open(&iface, demo)?), logger))
                }
                #[cfg(not(feature = "can-socket"))]
                {
                    let _ = demo;
                    Err(
                        "VEHICLE_SOURCE=can requires a build with can-socket feature enabled"
                            .into(),
                    )
                }
            }
            "rpmsg" | "m7" => {
                #[cfg(feature = "rpmsg")]
                {
                    let _ = demo;
                    Ok((Self::Rpmsg(RpmsgBus::open()?), logger))
                }
                #[cfg(not(feature = "rpmsg"))]
                {
                    let _ = demo;
                    Err("VEHICLE_SOURCE=rpmsg requires a build with rpmsg feature enabled".into())
                }
            }
            other => Err(format!("unknown VEHICLE_SOURCE: {other}")),
        }
    }

    /// Short source name for the start-up log line.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Sim(_) => "sim",
            #[cfg(feature = "can-socket")]
            Self::Can(_) => "can",
            #[cfg(feature = "rpmsg")]
            Self::Rpmsg(_) => "rpmsg",
        }
    }

    /// Advance time-driven sources (only the simulator needs stepping).
    pub fn step(&mut self, dt: Duration) {
        match self {
            Self::Sim(sim) => sim.step(dt),
            #[cfg(feature = "can-socket")]
            Self::Can(_) => {}
            #[cfg(feature = "rpmsg")]
            Self::Rpmsg(_) => {}
        }
    }

    /// Pull the newest signals into `state`, mark arrived frames on `tracker`
    /// (for per-signal availability), and refresh derived values. `now_ms` is
    /// the epoch-millisecond sample time.
    pub fn apply_to(
        &mut self,
        state: &mut VehicleState,
        logger: &mut Option<CanLogger>,
        tracker: &mut AvailabilityTracker,
        now_ms: i64,
    ) {
        match self {
            Self::Sim(sim) => {
                sim.apply_to(state);
                state.signals_live = true;
                // The simulator produces the whole signal set every step.
                tracker.mark_all(now_ms);
                if let Some(log) = logger {
                    log.log_frames(&encode_sim_frames(state));
                }
            }
            #[cfg(feature = "can-socket")]
            Self::Can(bus) => {
                bus.poll(state, logger, tracker, now_ms);
                state.signals_live = bus.signals_live();
            }
            #[cfg(feature = "rpmsg")]
            Self::Rpmsg(bus) => {
                bus.poll(state);
                state.signals_live = bus.signals_live();
                // The rpmsg bridge delivers the full signal set in one packet.
                if bus.signals_live() {
                    tracker.mark_all(now_ms);
                }
            }
        }
        state.refresh_derived();
    }
}
