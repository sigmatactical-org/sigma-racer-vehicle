# sigma-racer-vehicle

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
