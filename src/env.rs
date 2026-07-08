//! Environment variables — `SIGMA_RACER_WINGMAN_*` with bare-name fallback.

use std::env;

pub fn var(primary: &str) -> Option<String> {
    // Prefer the namespaced SIGMA_RACER_WINGMAN_* variable, then the bare name.
    // The owned `sigma` string is dropped at the end of the call — no leak.
    let sigma = format!("SIGMA_RACER_WINGMAN_{primary}");
    for name in [sigma.as_str(), primary] {
        if let Ok(value) = env::var(name)
            && !value.is_empty()
        {
            return Some(value);
        }
    }
    None
}

pub fn var_or(primary: &str, default: &str) -> String {
    var(primary).unwrap_or_else(|| default.into())
}

pub fn flag(primary: &str) -> bool {
    matches!(
        var(primary).as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes")
    )
}
