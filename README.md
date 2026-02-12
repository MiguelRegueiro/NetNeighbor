# NetNeighbor - Network Connection Monitor

A lightweight, efficient command-line application that monitors network connections and disconnections by watching ARP (Address Resolution Protocol) and IP neighbor tables. The application prints real-time notifications to the terminal when devices connect to or disconnect from the network, supporting both WiFi and Ethernet connections.

## Features

- **Real-time monitoring**: Continuously watches ARP/IP neighbor tables for connected devices
- **Connection detection**: Instantly reports when devices join the network
- **Disconnection detection**: Alerts when devices leave the network (with configurable timeout)
- **Multi-interface support**: Monitors all network interfaces simultaneously or filter to specific ones
- **Technology agnostic**: Detects WiFi, Ethernet, and other network connections
- **Configurable refresh rate**: Customizable polling interval to balance accuracy and system resources
- **Configurable disconnection timeout**: Adjustable timeout for considering devices disconnected
- **Interface identification**: Shows which interface each device is connected to
- **Timestamped events**: All notifications include precise timestamps
- **Cross-platform compatibility**: Works on Linux systems
- **Lightweight**: Minimal resource usage with efficient change detection algorithm

## Installation

### Prerequisites

- Rust compiler and Cargo package manager (version 1.70 or later)
- Access to system network commands (`ip`, `arp`)
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
Start monitoring with default settings (2-second refresh interval, 10-second disconnection timeout):

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

### Command Line Options

```
USAGE:
    netneighbor [OPTIONS]

OPTIONS:
    -i, --interval <INTERVAL>              Refresh interval in seconds [default: 2]
    -n, --interface <INTERFACE>            Network interface to monitor (e.g., wlan0, eth0)
        --disconnect-timeout <SECONDS>     Disconnection timeout in seconds (device considered disconnected after not seen for this duration) [default: 10]
    -v, --verbose                          Show verbose output
        --all-interfaces                   Monitor all interfaces [default: true if no interface specified]
    -h, --help                             Print help information
    -V, --version                          Print version information
```

## How It Works

The application implements an intelligent monitoring algorithm:

1. **Multi-source Data Collection**: Gathers data from both ARP table and IP neighbor table for comprehensive device detection
2. **Device Tracking**: Maintains a registry of known devices with their last-seen timestamps
3. **Connection Detection**: Identifies new devices when they appear in ARP/neighbor tables
4. **Disconnection Detection**: Considers devices disconnected if not seen for longer than the timeout period
5. **Interface Identification**: Reports which network interface each device is connected to
6. **Event Reporting**: Prints timestamped notifications for each connection/disconnection event
7. **Continuous Monitoring**: Repeats the process at the specified interval

The application intelligently combines data from `arp -a -n` and `ip neigh show` commands for the most comprehensive device detection.

## Example Output

Sample output showing device connections and disconnections:

```
NetNeighbor - Network Connection Monitor
Monitoring every 2 seconds
Disconnection timeout: 10 seconds
Monitoring all interfaces
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
- **Command not found**: Make sure `ip` or `arp` commands are available on your system
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
# ARP table
arp -a -n

# IP neighbor table
ip neigh show
```

## Performance Notes

- **CPU Usage**: Minimal - mostly sleeping between checks
- **Memory Usage**: Constant regardless of network size
- **Network Impact**: Zero - only reads local system tables, no network traffic generated
- **Refresh Interval**: Lower values provide faster detection but use slightly more CPU

Recommended settings:
- **Refresh interval**: 1-2 seconds for real-time monitoring, 3-5 seconds for general use
- **Disconnection timeout**: 5-10 seconds for responsive detection, 15-30 seconds for stability

## Architecture & Implementation

### Components
- **Main Application Loop**: Continuously polls the system's neighbor and ARP tables at defined intervals
- **Device Parser**: Interprets output from multiple system commands (`arp -a -n` and `ip neigh show`)
- **State Tracker**: Maintains a registry of known devices with timestamps of last detection
- **Event Logger**: Formats and prints connection/disconnection events with timestamps
- **CLI Interface**: Handles command-line arguments using the `clap` crate

### Data Structures
- `Device`: Represents a network device with IP address, MAC address, and interface
- `TrackedDevice`: Extends Device with a timestamp of when it was last seen
- `HashMap<String, TrackedDevice>`: Stores all detected devices with their last-seen times

### Optimization Features
- **Single Shell Execution**: Combines ARP and IP neighbor commands into one shell call to reduce process overhead
- **Efficient Lookups**: Uses HashSet for O(1) average lookup time during disconnection detection
- **Smart Parsing**: Optimized string processing with minimal allocations during parsing

## Security Considerations

- Requires read access to system ARP tables (typically available to all users)
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

- Detection delay depends on polling interval
- Cannot distinguish between different types of disconnections (power off, network loss, etc.)
- May miss very brief connections that occur between polling intervals
- Accuracy depends on ARP table update timing in the kernel
- Requires network commands (`ip`, `arp`) to be available in PATH
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