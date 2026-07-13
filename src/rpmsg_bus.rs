//! RPMsg character device input from the M7 `sigma-m7-signals` endpoint.

use sigma_racer_sidearm::{M7Signals, decode_wire};
use sigma_racer_telemetry::can::from_signals;
use sigma_racer_telemetry::state::VehicleState;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

const ENDPOINT: &str = "sigma-m7-signals";

pub struct RpmsgBus {
    dev: File,
    last_rx: Instant,
}

impl RpmsgBus {
    pub fn open() -> Result<Self, String> {
        let path = resolve_device()?;
        let mut opts = OpenOptions::new();
        opts.read(true).write(false);
        #[cfg(target_os = "linux")]
        opts.custom_flags(0x800); // O_NONBLOCK
        let dev = opts
            .open(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        Ok(Self {
            dev,
            last_rx: Instant::now() - Duration::from_secs(60),
        })
    }

    pub fn poll(&mut self, state: &mut VehicleState) {
        let mut buf = [0u8; 64];
        match self.dev.read(&mut buf) {
            Ok(n) if n >= 60 => {
                let mut sig = M7Signals::default();
                if decode_wire(&buf[..n], &mut sig).is_some() {
                    from_signals(&sig, state);
                    state.signals_live = true;
                    self.last_rx = Instant::now();
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => eprintln!("rpmsg read: {e}"),
        }
    }

    pub fn signals_live(&self) -> bool {
        self.last_rx.elapsed() < Duration::from_millis(500)
    }
}

fn resolve_device() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("RPMSG_DEVICE") {
        return Ok(PathBuf::from(path));
    }
    let sysfs = Path::new("/sys/bus/rpmsg/devices");
    if !sysfs.is_dir() {
        return Err("rpmsg sysfs missing — is imx remoteproc running?".into());
    }
    for entry in fs::read_dir(sysfs).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name_path = entry.path().join("name");
        let name = fs::read_to_string(&name_path).unwrap_or_default();
        if name.trim() == ENDPOINT {
            let dev = format!("/dev/{}", entry.file_name().to_string_lossy());
            return Ok(PathBuf::from(dev));
        }
    }
    Err(format!(
        "rpmsg endpoint '{ENDPOINT}' not found — create it from sysfs or set RPMSG_DEVICE"
    ))
}
