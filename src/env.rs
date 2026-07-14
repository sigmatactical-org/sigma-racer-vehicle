//! Environment variables — `SIGMA_RACER_WINGMAN_*` with bare-name fallback.

use std::env;

/// Read `SIGMA_RACER_WINGMAN_<primary>` falling back to bare `<primary>`;
/// empty values count as unset.
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

/// [`var`] with a default for unset/empty variables.
pub fn var_or(primary: &str, default: &str) -> String {
    var(primary).unwrap_or_else(|| default.into())
}

/// Boolean [`var`]: `1`, `true`, `TRUE`, or `yes` enable the flag.
pub fn flag(primary: &str) -> bool {
    matches!(
        var(primary).as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes")
    )
}
