//! Optional MDF4 CAN logging via mdf4-rs CanDbcLogger.

use crate::env;
use mdf4_rs::can::CanDbcLogger;
use mdf4_rs::writer::VecWriter;
use sigma_racer_telemetry::m7_dbc::m7_dbc;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

pub struct CanLogger {
    path: PathBuf,
    logger: Option<CanDbcLogger<VecWriter>>,
    started: Instant,
}

impl CanLogger {
    pub fn open() -> Option<Self> {
        let path = env::var("CAN_LOG_PATH")?;
        if path.is_empty() {
            return None;
        }

        let dbc = m7_dbc().clone();
        let logger = match CanDbcLogger::builder(dbc).include_units(true).build() {
            Ok(logger) => logger,
            Err(err) => {
                // Logging is best-effort; never take down telemetry because the
                // optional MDF4 logger could not be constructed.
                eprintln!("sigma-racer-vehicle: CAN logging disabled: {err}");
                return None;
            }
        };

        eprintln!("sigma-racer-vehicle: CAN MDF4 logging to {path}");
        Some(Self {
            path: PathBuf::from(path),
            logger: Some(logger),
            started: Instant::now(),
        })
    }

    pub fn log_frames(&mut self, frames: &[(u32, [u8; 8])]) {
        let Some(logger) = self.logger.as_mut() else {
            return;
        };
        let timestamp_us = self.started.elapsed().as_micros() as u64;
        for (id, payload) in frames {
            logger.log(*id, timestamp_us, payload);
        }
    }
}

impl Drop for CanLogger {
    fn drop(&mut self) {
        let Some(logger) = self.logger.take() else {
            return;
        };
        match logger.finalize() {
            Ok(bytes) => {
                if let Some(parent) = self.path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                match fs::write(&self.path, bytes) {
                    Ok(()) => {
                        eprintln!("sigma-racer-vehicle: wrote CAN log {}", self.path.display())
                    }
                    Err(err) => {
                        eprintln!(
                            "sigma-racer-vehicle: failed to write {}: {err}",
                            self.path.display()
                        );
                    }
                }
            }
            Err(err) => eprintln!("sigma-racer-vehicle: CAN log finalize: {err}"),
        }
    }
}
