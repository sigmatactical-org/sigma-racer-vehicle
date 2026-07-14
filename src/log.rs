//! Stderr logging with the daemon's `sigma-racer-vehicle:` prefix.
//!
//! The daemon's only log sink is stderr (collected by systemd/journald); this
//! macro keeps the prefix consistent across every module.

/// Write one prefixed line to stderr, `format!`-style.
macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!("sigma-racer-vehicle: {}", format_args!($($arg)*))
    };
}

pub(crate) use log;
