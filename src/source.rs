//! CAN or simulated signal input.

use crate::can_log::CanLogger;
use crate::env;
use crate::sim::Simulator;
use sigma_racer_telemetry::can::encode_sim_frames;
use sigma_racer_telemetry::state::VehicleState;
use std::time::Duration;

#[cfg(feature = "can-socket")]
use crate::can_bus::CanBus;

pub enum SignalSource {
    Sim(Simulator),
    #[cfg(feature = "can-socket")]
    Can(CanBus),
}

impl SignalSource {
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
            other => Err(format!("unknown VEHICLE_SOURCE: {other}")),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Sim(_) => "sim",
            #[cfg(feature = "can-socket")]
            Self::Can(_) => "can",
        }
    }

    pub fn step(&mut self, dt: Duration) {
        match self {
            Self::Sim(sim) => sim.step(dt),
            #[cfg(feature = "can-socket")]
            Self::Can(_) => {}
        }
    }

    pub fn apply_to(&mut self, state: &mut VehicleState, logger: &mut Option<CanLogger>) {
        match self {
            Self::Sim(sim) => {
                sim.apply_to(state);
                state.signals_live = true;
                if let Some(log) = logger {
                    log.log_frames(&encode_sim_frames(state));
                }
            }
            #[cfg(feature = "can-socket")]
            Self::Can(bus) => {
                bus.poll(state, logger);
                state.signals_live = bus.signals_live();
            }
        }
        state.refresh_derived();
    }
}
