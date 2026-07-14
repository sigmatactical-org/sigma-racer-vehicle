# sigma-racer-vehicle

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.97.0-blue.svg)](https://www.rust-lang.org)

Linux daemon for the Sigma Racer cockpit: reads M7 safety-bus CAN (or a simulator), maintains `VehicleState`, and publishes NDJSON telemetry on a Unix socket for `sigma-racer-cluster`.

## Binary

- **`sigma-racer-vehicle`** — runs as `sigma-racer-vehicle` on the Wingman image

## Signal sources

| `VEHICLE_SOURCE` | Mode |
|------------------|------|
| `sim` (default) | Built-in ride simulator |
| `socketcan` | Live CAN on `CAN_IFACE` (requires `can-socket` feature) |

Environment variables accept `SIGMA_RACER_WINGMAN_*` prefixes (image default) or bare names.

## Build

```bash
cargo build --release --features can-socket
```

## Dependencies

- [`sigma-racer-telemetry`](../sigma-racer-telemetry) — VSS state and IPC protocol
- [`sigma-racer-sidearm`](../sigma-racer-sidearm) — M7 CAN contract (via telemetry)

## Brand & artwork

© Sigma Tactical Group. **All rights reserved.**

The Sigma Tactical Group name, logos, marks, artwork, and visual identity are **proprietary**. They are not covered by this repository's source-code license. See [BRANDING.md](BRANDING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
