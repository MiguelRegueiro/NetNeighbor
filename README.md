# NetNeighbor - Network Connection Monitor

A lightweight, efficient command-line application that monitors network connections and disconnections by watching the Linux IP neighbor table. It prints real-time notifications when devices connect to or disconnect from your LAN, with defaults tuned to avoid Docker/VM bridge noise.

## Features

- **Real-time monitoring**: Continuously watches the kernel neighbor table for connected devices
- **Connection detection**: Instantly reports when devices join the network
- **Disconnection detection**: Alerts when devices leave the network (with configurable timeout)
- **Smarter interface defaults**: Monitors routed interfaces by default (usually WiFi/Ethernet), with optional all-interface mode
- **Multi-interface support**: Monitors all interfaces or a specific interface (`-n`)
- **Faster detection defaults**: 1s polling, active discovery probes, and disconnect hysteresis to reduce false flapping
- **Configurable disconnection timeout**: Adjustable timeout for considering devices disconnected
- **Active discovery**: Optional periodic subnet probing to populate neighbor cache and find quieter devices (like phones)
- **MAC-first tracking**: If one device has multiple IPs, only one entry is shown (best IP is selected)
- **Terminal dashboard**: Two-column TUI-style view with current device status and connection/disconnection totals
- **Interface identification**: Shows which interface each device is connected to
- **Timestamped events**: All notifications include precise timestamps
- **Cross-platform compatibility**: Works on Linux systems
- **Lightweight**: Minimal resource usage with efficient change detection algorithm

## Installation

### Prerequisites

- Rust compiler and Cargo package manager (version 1.70 or later)
- Access to the `ip` command (`iproute2`)
- Network interface with active connections

### Building from Source

```bash
# Navigate to the project directory
cd netneighbor

# Build the optimized release version
cargo build --release

# The executable will be located at:
# target/release/netneighbor
```

### Running the Application

After building, you can run the application directly:

```bash
./target/release/netneighbor
```

## Usage

### Basic Usage
Start monitoring with default settings (1-second refresh interval, 8-second disconnection timeout, active probe enabled, routed interfaces only):

```bash
./target/release/netneighbor
```

### Advanced Usage Examples

With custom refresh interval (in seconds):
```bash
./target/release/netneighbor --interval 1    # Check every 1 second
```

With custom disconnection timeout:
```bash
./target/release/netneighbor --disconnect-timeout 5    # Device considered disconnected after 5 seconds not seen
```

Monitor specific network interface:
```bash
./target/release/netneighbor -n wlan0    # Monitor only wlan0 interface
```

Run with custom interval and disconnection timeout:
```bash
./target/release/netneighbor --interval 3 --disconnect-timeout 15
```

Monitor every interface (including Docker/bridge/VM links):
```bash
./target/release/netneighbor --all-interfaces
```

Disable active discovery probes:
```bash
./target/release/netneighbor --active-probe false
```

Use classic line-by-line output instead of dashboard:
```bash
./target/release/netneighbor --plain
```

### Command Line Options

```
USAGE:
    netneighbor [OPTIONS]

OPTIONS:
    -i, --interval <INTERVAL>              Refresh interval in seconds [default: 1]
    -n, --interface <INTERFACE>            Network interface to monitor (e.g., wlan0, eth0)
    --disconnect-timeout <SECONDS>         Device considered disconnected after this many seconds without active confirmation [default: 8]
    --min-missed-polls <N>                 Require this many missed polls before disconnecting [default: 3]
    -v, --verbose                          Show verbose output
    --all-interfaces                       Monitor all interfaces (includes virtual/bridge links)
    --active-probe <true|false>            Actively probe local subnets to discover quiet devices [default: true]
    --probe-interval <SECONDS>             Seconds between active discovery sweeps [default: 8]
    --probe-timeout-ms <MILLISECONDS>      TCP timeout per probe [default: 120]
    --max-probe-hosts <N>                  Max hosts to probe per subnet [default: 256]
    --include-ipv6                         Include IPv6 devices (disabled by default)
    --plain                                Disable dashboard and print classic line events
    -h, --help                             Print help information
    -V, --version                          Print version information
```

## How It Works

The application implements an intelligent monitoring algorithm:

1. **Interface Selection**: Uses `-n` if provided, otherwise routed interfaces by default, or all interfaces with `--all-interfaces`
2. **Neighbor Collection**: Reads `ip neigh show` and parses valid entries with MAC + healthy neighbor states
3. **Active Discovery (Optional)**: Periodically probes local IPv4 subnets to trigger ARP/neighbor resolution for quieter devices
4. **Device Tracking**: Maintains a registry of known devices with last-seen timestamps
5. **Connection/Disconnection Detection**: Prints events when devices appear or exceed disconnection timeout
6. **Continuous Monitoring**: Repeats at the configured interval

## Example Output

Sample output showing device connections and disconnections:

```
NetNeighbor - Network Connection Monitor
Monitoring every 2 seconds
Disconnection timeout: 10 seconds
Monitoring routed interfaces: wlo1
Active probing: enabled
Press Ctrl+C to stop

[2026-02-12 21:26:43] DEVICE CONNECTED - IP: 192.168.1.40, MAC: c8:a3:62:67:99:b2, Interface: wlo1
[2026-02-12 21:26:43] DEVICE CONNECTED - IP: 192.168.1.1, MAC: 78:29:ed:2c:3b:ba, Interface: wlo1
[2026-02-12 21:28:46] DEVICE DISCONNECTED - IP: 192.168.1.40, MAC: c8:a3:62:67:99:b2, Interface: wlo1
[2026-02-12 21:29:14] DEVICE CONNECTED - IP: 192.168.1.40, MAC: c8:a3:62:67:99:b2, Interface: wlo1
```

## Common Use Cases

- **Network administration**: Monitor who connects to your network (WiFi and Ethernet)
- **Security auditing**: Track unauthorized device access
- **IoT monitoring**: Watch for smart device connections/disconnections
- **Home networks**: See when family members connect devices
- **Troubleshooting**: Debug connectivity issues by observing patterns
- **Device tracking**: Monitor when specific devices come online/offline

## Permissions

On most systems, the application can run without elevated privileges since reading ARP tables is typically allowed for all users. However, if you encounter permission errors:

```bash
# Try running with sudo if needed
sudo ./target/release/netneighbor
```

## Troubleshooting

### Common Issues

- **No devices detected**: Ensure your network interface is active and connected to a network with devices
- **Permission errors**: Try running with `sudo` (though usually not required)
- **Wrong interface**: Verify the interface name with `ip addr show` or `ifconfig`
- **Too many Docker/VM interfaces shown**: Use default mode (no `--all-interfaces`) or select one interface with `-n`
- **Some phones don't appear quickly**: Keep `--active-probe true` and reduce `--probe-interval` (for example, `--probe-interval 10`)
- **Command not found**: Make sure `ip` is available on your system
- **Delayed disconnection detection**: Some devices may remain in ARP cache longer than expected

### Verifying Network Interfaces

To see available network interfaces:
```bash
ip addr show
# or
ifconfig
```

Look for interfaces with IP addresses assigned (usually in 192.168.x.x, 10.x.x.x, or 172.x.x.x ranges).

### Checking ARP Tables Manually

You can verify the underlying data the application monitors:
```bash
# IP neighbor table
ip neigh show
```

## Performance Notes

- **CPU Usage**: Minimal - mostly sleeping between checks
- **Memory Usage**: Constant regardless of network size
- **Network Impact**: Low - active probing creates lightweight local TCP attempts by default
- **Refresh Interval**: Lower values provide faster detection but use slightly more CPU

Recommended settings:
- **Refresh interval**: 1-2 seconds for real-time monitoring, 3-5 seconds for general use
- **Disconnection timeout**: 5-10 seconds for responsive detection, 15-30 seconds for stability

## Architecture & Implementation

### Components
- **Main Application Loop**: Continuously polls the system's neighbor table at defined intervals
- **Device Parser**: Interprets `ip neigh show` output with state/interface filtering
- **Active Probe Engine**: Performs bounded subnet probing to improve neighbor cache coverage
- **State Tracker**: Maintains a registry of known devices with timestamps of last detection
- **Event Logger**: Formats and prints connection/disconnection events with timestamps
- **CLI Interface**: Handles command-line arguments using the `clap` crate

### Data Structures
- `Device`: Represents a network device with IP address, MAC address, and interface
- `TrackedDevice`: Extends Device with a timestamp of when it was last seen
- `HashMap<String, TrackedDevice>`: Stores all detected devices with their last-seen times

### Optimization Features
- **Efficient Lookups**: Uses HashSet for O(1) average lookup time during disconnection detection
- **Bounded Probing**: Caps probe count per subnet and timeout per probe
- **Smart Parsing**: Filters invalid/incomplete neighbor entries to reduce noisy events

## Security Considerations

- Requires read access to the system neighbor table (typically available to all users)
- Does not store or transmit sensitive network information
- Does not modify system network state
- Command injection risks are mitigated by using safe process spawning
- Input validation on command-line parameters
- Only reads system information, no network traffic is generated

## Dependencies

- `clap`: For command-line argument parsing
- `chrono`: For timestamp formatting
- Standard library: `std::process::Command` for system command execution
- Standard library: `std::collections::HashMap` for state management
- Standard library: `std::time::Instant` for timeout tracking

## Build Process

The application uses Cargo for dependency management and building:
- Development build: `cargo build`
- Release build: `cargo build --release`
- Run directly: `cargo run --bin netneighbor [options]`

## Limitations

- Detection delay depends on polling interval and probe interval
- Cannot distinguish between different types of disconnections (power off, network loss, etc.)
- May miss very brief connections that occur between polling intervals
- Accuracy depends on neighbor cache updates in the kernel
- Requires the `ip` command to be available in PATH
- Some devices (especially mobile devices) may remain in ARP cache longer than expected after disconnection

## Contributing

Contributions are welcome! Feel free to submit issues or pull requests to improve functionality or fix bugs. Areas for improvement might include:

- Real-time socket-based monitoring instead of polling
- MAC address vendor identification
- Persistent storage of connection history
- Web interface for remote monitoring
- Alert mechanisms (email, notifications)
- Network range filtering
- Export to various formats (CSV, JSON)
- Historical statistics and analytics

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Author

Created as a network monitoring utility for tracking WiFi and Ethernet connections and disconnections.
