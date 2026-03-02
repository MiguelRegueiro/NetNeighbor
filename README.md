# NetNeighbor

Lightweight Linux CLI for monitoring device connect/disconnect events from the IP neighbor table.

## Quick Start

### Requirements
- Linux
- Rust + Cargo (`1.85+`)
- `ip` command available (`iproute2`)

### Run (recommended)
```bash
cargo run --release
```

### Common commands
```bash
# specific interface
cargo run --release -- -n wlan0

# plain (non-dashboard) output
cargo run --release -- --plain

# faster disconnect behavior
cargo run --release -- --disconnect-timeout 5 --min-missed-polls 2

# disable active probing
cargo run --release -- --active-probe false
```

## What It Does
- Detects device connect/disconnect events in near real-time
- Tracks by MAC to avoid duplicate multi-IP rows
- Defaults to routed interfaces to reduce Docker/VM noise
- Optional active probing to surface quiet devices sooner
- Dashboard mode (`default`) and plain log mode (`--plain`)

## CLI

Use:
```bash
cargo run --release -- --help
```

<details>
<summary>Full option list</summary>

```text
-i, --interval <INTERVAL>              Refresh interval in seconds [default: 1]
-n, --interface <INTERFACE>            Network interface to monitor (e.g., wlan0, eth0)
--disconnect-timeout <SECONDS>         Seconds without active confirmation before disconnect [default: 8]
--min-missed-polls <N>                 Missed polls required before disconnect [default: 3]
-v, --verbose                          Show extra diagnostics
--all-interfaces                       Include virtual/bridge interfaces
--active-probe <true|false>            Local subnet probing [default: true]
--probe-interval <SECONDS>             Probe sweep interval [default: 8]
--probe-timeout-ms <MILLISECONDS>      TCP timeout per probe [default: 120]
--max-probe-hosts <N>                  Max hosts probed per subnet [default: 256]
--include-ipv6                         Include IPv6 devices
--plain                                Disable dashboard and print line events
-h, --help                             Print help
-V, --version                          Print version
```

</details>

## Example Output

```text
[2026-02-12 21:26:43] DEVICE CONNECTED - IP: 192.168.1.40, MAC: c8:a3:62:67:99:b2, Interface: wlo1
[2026-02-12 21:28:46] DEVICE DISCONNECTED - IP: 192.168.1.40, MAC: c8:a3:62:67:99:b2, Interface: wlo1
```

## Contributing
Contribution and release process documentation is in `CONTRIBUTING.md`.

## Security Notes
- Reads system neighbor/network interface information
- Does not modify system network config
- If active probing is enabled (`--active-probe true`), it generates lightweight local probe traffic

## Troubleshooting

<details>
<summary>Common issues</summary>

- No devices detected:
  - Check interface selection (`-n`)
  - Verify `ip neigh show` returns entries
- Too much virtual-interface noise:
  - Use default mode (no `--all-interfaces`) or set `-n <iface>`
- Some mobile devices appear late:
  - Keep probing enabled and lower `--probe-interval`
- Permission errors:
  - Try running with elevated privileges

</details>

<details>
<summary>Useful diagnostics</summary>

```bash
ip addr show
ip neigh show
cargo run --release -- --verbose
```

</details>

## Limitations
- Polling-based detection (not socket/event based)
- Brief between-poll connections may be missed
- Neighbor-cache behavior depends on kernel/device behavior
- Cannot classify disconnect cause (power-off vs roaming vs link loss)

## License
MIT (see `LICENSE`)
