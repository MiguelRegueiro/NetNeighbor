use clap::Parser;
use std::collections::HashMap;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use chrono::Local;
use colored::*;
use oui_data::lookup;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Refresh interval in seconds
    #[arg(short, long, default_value_t = 2)]
    interval: u64,

    /// Network interface to monitor (e.g., wlan0, eth0)
    #[arg(short = 'n', long)]  // Changed from -i to -n to avoid conflict with interval
    interface: Option<String>,

    /// Show verbose output
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
    
    /// Monitor all interfaces
    #[arg(long, default_value_t = false)]
    all_interfaces: bool,
    
    /// Disconnection timeout in seconds (device considered disconnected after not seen for this duration)
    #[arg(long, default_value_t = 10)]
    disconnect_timeout: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Device {
    ip_address: String,
    mac_address: String,
    interface: String,
}

impl Device {
    fn new(ip: String, mac: String, interface: String) -> Self {
        Device {
            ip_address: ip,
            mac_address: mac,
            interface,
        }
    }
    
    fn key(&self) -> String {
        format!("{}-{}", self.ip_address, self.mac_address)
    }
}

#[derive(Debug)]
struct TrackedDevice {
    device: Device,
    last_seen: Instant,
}

// Function to format device information with colors for better readability
fn format_device_output(event: &str, ip: &str, mac: &str, interface: &str) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Get vendor information from MAC address
    let vendor = get_vendor_from_mac(mac);

    // Color the event based on connection type
    let event_text = if event == "CONNECTED" {
        format!("[{}] {}", timestamp, "[CONNECTED]".green())
    } else {
        format!("[{}] {}", timestamp, "[DISCONNECTED]".red())
    };

    if let Some(vendor_name) = vendor {
        println!("{} IP: {} | MAC: {} | Vendor: {} | Interface: {}",
                 event_text,
                 ip.blue(),
                 mac.yellow(),
                 vendor_name.cyan(),
                 interface.magenta());
    } else {
        println!("{} IP: {} | MAC: {} | Vendor: Unknown | Interface: {}",
                 event_text,
                 ip.blue(),
                 mac.yellow(),
                 interface.magenta());
    }
}

// Function to get vendor name from MAC address
fn get_vendor_from_mac(mac: &str) -> Option<String> {
    // Use the oui-data crate to look up the vendor
    match lookup(mac) {
        Some(vendor) => Some(vendor.organization().to_string()),
        None => None,
    }
}

fn get_network_devices(interface: Option<&str>) -> Result<Vec<Device>, Box<dyn std::error::Error>> {
    let mut devices = Vec::new();

    // Execute both commands in a single shell to reduce process overhead
    let script = "arp -a -n; echo '===SPLIT==='; ip neigh show";
    let output = Command::new("sh")
        .args(&["-c", script])
        .output()?;

    if output.status.success() {
        let content = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = content.split("===SPLIT===").collect();
        
        if parts.len() >= 2 {
            parse_arp_entries(parts[0], &mut devices, interface)?;
            parse_ip_neigh_entries(parts[1], &mut devices, interface)?;
        }
    }

    Ok(devices)
}

fn parse_arp_entries(content: &str, devices: &mut Vec<Device>, interface_filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    for line in content.lines() {
        // Example ARP entry: "? (192.168.1.1) at aa:bb:cc:dd:ee:ff [ether] on wlan0"
        if line.contains("at") && line.contains("on") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 7 {
                // Extract IP address (removing parentheses)
                let ip = parts[1].trim_start_matches('(').trim_end_matches(')');

                // Extract MAC address (after "at")
                let mac = parts[3];

                // Extract interface (after "on", which is at index 5, interface is at index 6)
                let iface = parts[6];

                // Apply interface filter if specified
                if let Some(filter) = interface_filter {
                    if iface != filter {
                        continue;
                    }
                }

                // Only add if IP looks like a valid network address (not localhost or multicast)
                if !ip.starts_with("127.") && !ip.starts_with("224.") && !ip.starts_with("255.") && ip != "::1" {
                    devices.push(Device::new(ip.to_string(), mac.to_string(), iface.to_string()));
                }
            }
        }
    }
    Ok(())
}

fn parse_ip_neigh_entries(content: &str, devices: &mut Vec<Device>, interface_filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let ip = parts[0];

            // Look for MAC address after "lladdr" and interface after "dev"
            let mut mac = None;
            let mut iface = None;

            let mut i = 0;
            while i < parts.len() {
                if parts[i] == "lladdr" && i + 1 < parts.len() {
                    mac = Some(parts[i + 1]);
                } else if parts[i] == "dev" && i + 1 < parts.len() {
                    iface = Some(parts[i + 1]);
                }
                i += 1;
            }

            // Apply interface filter if specified
            if let Some(filter) = interface_filter {
                if let Some(iface_val) = iface {
                    if iface_val != filter {
                        continue;
                    }
                }
            }

            if let (Some(mac_val), Some(iface_val)) = (mac, iface) {
                devices.push(Device::new(ip.to_string(), mac_val.to_string(), iface_val.to_string()));
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("NetNeighbor - Network Connection Monitor");
    println!("Monitoring every {} seconds", args.interval);
    println!("Disconnection timeout: {} seconds", args.disconnect_timeout);
    if let Some(ref iface) = args.interface {
        println!("Interface: {}", iface);
    } else {
        println!("Monitoring all interfaces");
    }
    println!("Press Ctrl+C to stop\n");

    let mut tracked_devices: HashMap<String, TrackedDevice> = HashMap::new();

    loop {
        match get_network_devices(args.interface.as_deref()) {
            Ok(current_devices) => {
                let now = Instant::now();

                // Create a set of current device keys for O(1) lookup instead of O(n) vector search
                let current_device_keys: std::collections::HashSet<String> = 
                    current_devices.iter().map(|d| d.key()).collect();

                // Process current devices - update last seen time
                for device in current_devices {
                    let key = device.key();

                    // Check if this is a new connection
                    if !tracked_devices.contains_key(&key) {
                        format_device_output("CONNECTED", &device.ip_address, &device.mac_address, &device.interface);
                    }

                    // Update the tracked device with current time
                    tracked_devices.insert(key, TrackedDevice {
                        device,
                        last_seen: now,
                    });
                }

                // Check for disconnections - devices not seen within timeout period
                // Collect keys to remove to avoid borrowing issues
                let mut keys_to_remove = Vec::new();
                
                for (key, tracked_device) in &tracked_devices {
                    if now.duration_since(tracked_device.last_seen).as_secs() > args.disconnect_timeout {
                        // Check if this device is still in current devices (it might have just been updated)
                        if !current_device_keys.contains(key) {
                            keys_to_remove.push((key.clone(), tracked_device.device.clone()));
                        }
                    }
                }

                // Report disconnections and remove from tracking
                for (key, device) in keys_to_remove {
                    format_device_output("DISCONNECTED", &device.ip_address, &device.mac_address, &device.interface);
                    tracked_devices.remove(&key);
                }

                if args.verbose && tracked_devices.is_empty() {
                    println!("[{}] No devices detected", Local::now().format("%Y-%m-%d %H:%M:%S"));
                }
            }
            Err(e) => {
                eprintln!("Error reading network state: {}", e);
            }
        }

        thread::sleep(Duration::from_secs(args.interval));
    }
}