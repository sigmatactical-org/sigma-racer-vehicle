//! Simulated ride for QEMU / bench (encodes → decodes through M7 draft codec).

use sigma_racer_telemetry::can::{decode_frame, encode_sim_frames};
use sigma_racer_telemetry::state::VehicleState;
use std::time::Duration;

/// Synthetic ride generator: demo mode sweeps rpm/speed/lean, otherwise it
/// produces an idle state. Output is round-tripped through the M7 CAN codec
/// so the daemon exercises the same decode path as real hardware.
pub struct Simulator {
    demo: bool,
    t: f32,
    phase: f32,
}

impl Simulator {
    /// Create a simulator; `demo` animates the ride instead of idling.
    pub fn new(demo: bool) -> Self {
        Self {
            demo,
            t: 0.0,
            phase: 0.0,
        }
    }

    /// Advance simulated time by `dt`.
    pub fn step(&mut self, dt: Duration) {
        self.t += dt.as_secs_f32();
        if self.demo {
            self.phase += dt.as_secs_f32() * 0.15;
        }
    }

    /// Encode the simulated ride to CAN frames and decode them into `state`.
    pub fn apply_to(&self, state: &mut VehicleState) {
        let mut sim = VehicleState::idle();
        if self.demo {
            let rpm = 1_200.0 + 4_500.0 * (self.phase.sin().max(0.0));
            let speed = (rpm / 120.0).min(160.0);
            sim.rpm = rpm;
            sim.speed = speed;
            sim.gear = if speed < 1.0 {
                0
            } else if speed < 40.0 {
                1
            } else if speed < 70.0 {
                2
            } else if speed < 100.0 {
                3
            } else if speed < 130.0 {
                4
            } else {
                5
            };
            sim.side_stand = speed < 1.0;
            sim.lean_angle = (self.phase * 0.7).sin() * 18.0;
            sim.gforce = (self.phase * 1.1).sin() * 0.35;
        }

        sim.refresh_derived();
        let mut decoded = VehicleState::idle();
        for (id, payload) in encode_sim_frames(&sim) {
            decode_frame(id, &payload, &mut decoded);
        }
        *state = decoded;
    }
}
