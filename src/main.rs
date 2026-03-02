use chrono::Local;
use clap::Parser;
use colored::*;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Refresh interval in seconds
    #[arg(short, long, default_value_t = 1)]
    interval: u64,

    /// Network interface to monitor (e.g., wlan0, eth0)
    #[arg(short = 'n', long)]
    interface: Option<String>,

    /// Show extra diagnostics
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Monitor all interfaces (including virtual/bridge)
    #[arg(long, default_value_t = false)]
    all_interfaces: bool,

    /// Device considered disconnected after this many seconds without active confirmation
    #[arg(long, default_value_t = 8)]
    disconnect_timeout: u64,

    /// Require this many missed polls before disconnecting
    #[arg(long, default_value_t = 3)]
    min_missed_polls: u32,

    /// Actively probe local subnets to discover quiet devices
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    active_probe: bool,

    /// Seconds between active discovery sweeps
    #[arg(long, default_value_t = 8)]
    probe_interval: u64,

    /// TCP connect timeout in milliseconds for active discovery probes
    #[arg(long, default_value_t = 120)]
    probe_timeout_ms: u64,

    /// Max hosts to probe per subnet (prevents huge scans on large CIDRs)
    #[arg(long, default_value_t = 256)]
    max_probe_hosts: usize,

    /// Include IPv6 devices (disabled by default to avoid link-local flap noise)
    #[arg(long, default_value_t = false)]
    include_ipv6: bool,

    /// Disable dashboard and print classic line events only
    #[arg(long, default_value_t = false)]
    plain: bool,
}

#[derive(Debug, Clone)]
struct DeviceObservation {
    ip_address: String,
    mac_address: String,
    interface: String,
    state: Option<String>,
}

#[derive(Debug, Clone)]
struct DeviceRecord {
    mac_address: String,
    interface: String,
    ip_address: String,
    online: bool,
    last_active: Instant,
    missed_polls: u32,
    last_probe_generation: u64,
    connect_count: u64,
    disconnect_count: u64,
}

#[derive(Debug, Clone)]
struct LocalSubnet {
    address: Ipv4Addr,
    prefix: u8,
}

#[derive(Debug)]
struct Metrics {
    total_connects: u64,
    total_disconnects: u64,
}

const VIRTUAL_INTERFACE_PREFIXES: &[&str] = &[
    "lo",
    "br-",
    "docker",
    "veth",
    "virbr",
    "vmnet",
    "vboxnet",
    "cni",
    "flannel",
    "zt",
    "tailscale",
    "tun",
    "tap",
    "wg",
    "ifb",
];

const VALID_NEIGHBOR_STATES: &[&str] = &["REACHABLE", "STALE", "DELAY", "PROBE", "PERMANENT"];
const ACTIVE_NEIGHBOR_STATES: &[&str] = &["REACHABLE", "DELAY", "PROBE", "PERMANENT"];
const KNOWN_NEIGHBOR_STATES: &[&str] = &[
    "REACHABLE",
    "STALE",
    "DELAY",
    "PROBE",
    "PERMANENT",
    "FAILED",
    "INCOMPLETE",
];
fn now_stamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn device_key(mac: &str, iface: &str) -> String {
    format!("{}@{}", mac, iface)
}

fn parse_ip_neigh_entries(
    content: &str,
    interface_filter: Option<&str>,
    allowed_interfaces: Option<&HashSet<String>>,
    include_ipv6: bool,
) -> Vec<DeviceObservation> {
    let mut observations = Vec::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let ip = parts[0];
        let mut mac = None;
        let mut iface = None;
        let mut state = None;

        let mut i = 0;
        while i < parts.len() {
            if parts[i] == "lladdr" && i + 1 < parts.len() {
                mac = Some(parts[i + 1]);
            } else if parts[i] == "dev" && i + 1 < parts.len() {
                iface = Some(parts[i + 1]);
            } else if VALID_NEIGHBOR_STATES.contains(&parts[i]) {
                state = Some(parts[i]);
            }
            i += 1;
        }

        let Some(iface_val) = iface else {
            continue;
        };

        if let Some(filter) = interface_filter {
            if iface_val != filter {
                continue;
            }
        } else if let Some(allowed) = allowed_interfaces
            && !allowed.contains(iface_val)
        {
            continue;
        }

        if !is_allowed_ip(ip, include_ipv6) {
            continue;
        }

        let Some(mac_val) = mac else {
            continue;
        };

        if !looks_like_mac(mac_val) {
            continue;
        }

        observations.push(DeviceObservation {
            ip_address: ip.to_string(),
            mac_address: mac_val.to_string(),
            interface: iface_val.to_string(),
            state: state.map(|s| s.to_string()),
        });
    }

    observations
}

fn get_observations(
    interface_filter: Option<&str>,
    allowed_interfaces: Option<&HashSet<String>>,
    include_ipv6: bool,
) -> Result<Vec<DeviceObservation>, Box<dyn std::error::Error>> {
    let output = Command::new("ip").args(["neigh", "show"]).output()?;
    if !output.status.success() {
        return Ok(Vec::new());
    }

    let content = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ip_neigh_entries(
        &content,
        interface_filter,
        allowed_interfaces,
        include_ipv6,
    ))
}

fn looks_like_mac(value: &str) -> bool {
    if value.len() != 17 {
        return false;
    }

    value.chars().enumerate().all(|(idx, c)| {
        if [2, 5, 8, 11, 14].contains(&idx) {
            c == ':'
        } else {
            c.is_ascii_hexdigit()
        }
    })
}

fn is_link_local_ipv6(ip: &str) -> bool {
    ip.starts_with("fe80:")
}

fn is_allowed_ip(ip: &str, include_ipv6: bool) -> bool {
    if ip == "::1" || ip.starts_with("ff") {
        return false;
    }

    if let Ok(addr) = ip.parse::<Ipv4Addr>() {
        return !addr.is_loopback()
            && !addr.is_multicast()
            && !addr.is_unspecified()
            && !addr.is_broadcast();
    }

    if !include_ipv6 {
        return false;
    }

    !is_link_local_ipv6(ip)
}

fn ip_rank(ip: &str) -> u8 {
    if ip.parse::<Ipv4Addr>().is_ok() {
        0
    } else if is_link_local_ipv6(ip) {
        2
    } else {
        1
    }
}

fn choose_best_observations(
    observations: Vec<DeviceObservation>,
) -> HashMap<String, DeviceObservation> {
    let mut best = HashMap::new();

    for observation in observations {
        let key = device_key(&observation.mac_address, &observation.interface);
        match best.get(&key) {
            None => {
                best.insert(key, observation);
            }
            Some(existing) => {
                let new_rank = ip_rank(&observation.ip_address);
                let old_rank = ip_rank(&existing.ip_address);
                if new_rank < old_rank {
                    best.insert(key, observation);
                }
            }
        }
    }

    best
}

fn get_default_route_interfaces() -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut interfaces = HashSet::new();
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()?;

    if !output.status.success() {
        return Ok(interfaces);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for window in parts.windows(2) {
            if window[0] == "dev" {
                interfaces.insert(window[1].to_string());
            }
        }
    }

    Ok(interfaces)
}

fn discover_non_virtual_interfaces() -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let mut interfaces = HashSet::new();
    for entry in fs::read_dir("/sys/class/net")? {
        let name = entry?.file_name().to_string_lossy().to_string();
        if is_virtual_interface_name(&name) {
            continue;
        }
        interfaces.insert(name);
    }
    Ok(interfaces)
}

fn is_virtual_interface_name(name: &str) -> bool {
    VIRTUAL_INTERFACE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn should_monitor_interface(
    iface: &str,
    interface_filter: Option<&str>,
    allowed_interfaces: Option<&HashSet<String>>,
) -> bool {
    if let Some(filter) = interface_filter {
        iface == filter
    } else if let Some(allowed) = allowed_interfaces {
        allowed.contains(iface)
    } else {
        true
    }
}

fn local_subnets(
    interface_filter: Option<&str>,
    allowed_interfaces: Option<&HashSet<String>>,
) -> Result<Vec<LocalSubnet>, Box<dyn std::error::Error>> {
    let output = Command::new("ip")
        .args(["-o", "-4", "addr", "show", "up"])
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let mut subnets = Vec::new();
    let content = String::from_utf8_lossy(&output.stdout);

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let iface = parts[1];
        if !should_monitor_interface(iface, interface_filter, allowed_interfaces) {
            continue;
        }

        let mut cidr = None;
        for window in parts.windows(2) {
            if window[0] == "inet" {
                cidr = Some(window[1]);
                break;
            }
        }
        let Some(cidr) = cidr else {
            continue;
        };

        let Some((ip_str, prefix_str)) = cidr.split_once('/') else {
            continue;
        };

        let Ok(address) = ip_str.parse::<Ipv4Addr>() else {
            continue;
        };

        let Ok(prefix) = prefix_str.parse::<u8>() else {
            continue;
        };

        if prefix > 30 {
            continue;
        }

        subnets.push(LocalSubnet { address, prefix });
    }

    Ok(subnets)
}

fn enumerate_probe_targets(subnet: &LocalSubnet, max_hosts: usize) -> Vec<Ipv4Addr> {
    if subnet.prefix == 0 || subnet.prefix > 30 {
        return Vec::new();
    }

    let host_bits = 32u32.saturating_sub(subnet.prefix as u32);
    let total_ips = 1u64 << host_bits;
    if total_ips <= 2 {
        return Vec::new();
    }

    let host_count = (total_ips - 2) as usize;
    let ip_u32 = u32::from(subnet.address);
    let mask = if subnet.prefix == 0 {
        0
    } else {
        u32::MAX << (32 - subnet.prefix)
    };

    let network = ip_u32 & mask;
    let first_host = network.saturating_add(1);

    let mut targets = Vec::new();

    if host_count <= max_hosts {
        for offset in 0..host_count {
            let candidate = Ipv4Addr::from(first_host + offset as u32);
            if candidate != subnet.address {
                targets.push(candidate);
            }
        }
        return targets;
    }

    for i in 0..max_hosts {
        let offset = (i * host_count) / max_hosts;
        let candidate = Ipv4Addr::from(first_host + offset as u32);
        if candidate != subnet.address {
            targets.push(candidate);
        }
    }

    targets.sort_unstable();
    targets.dedup();
    targets
}

fn active_probe_subnets(
    interface_filter: Option<&str>,
    allowed_interfaces: Option<&HashSet<String>>,
    timeout_ms: u64,
    max_hosts: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let subnets = local_subnets(interface_filter, allowed_interfaces)?;
    if subnets.is_empty() {
        return Ok(0);
    }

    let timeout = Duration::from_millis(timeout_ms.max(20));
    let mut handles: VecDeque<thread::JoinHandle<()>> = VecDeque::new();
    let mut launched = 0usize;
    let max_parallel = 96usize;

    for subnet in &subnets {
        for target in enumerate_probe_targets(subnet, max_hosts) {
            let addr = SocketAddr::V4(SocketAddrV4::new(target, 443));
            let timeout_copy = timeout;
            handles.push_back(thread::spawn(move || {
                let _ = TcpStream::connect_timeout(&addr, timeout_copy);
            }));
            launched += 1;

            if handles.len() >= max_parallel
                && let Some(handle) = handles.pop_front()
            {
                let _ = handle.join();
            }
        }
    }

    while let Some(handle) = handles.pop_front() {
        let _ = handle.join();
    }

    Ok(launched)
}

fn refresh_neighbor_state(
    ip_address: &str,
    interface: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = Command::new("ip")
        .args(["neigh", "get", ip_address, "dev", interface])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    for token in content.split_whitespace() {
        if KNOWN_NEIGHBOR_STATES.contains(&token) {
            return Ok(Some(token.to_string()));
        }
    }
    Ok(None)
}

fn truncate_cell(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.chars().count() <= width {
        return value.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }

    let mut s = value.chars().take(width - 1).collect::<String>();
    s.push('…');
    s
}

fn clear_screen_and_home(stdout: &mut io::Stdout) -> io::Result<()> {
    write!(stdout, "\x1b[2J\x1b[H")?;
    Ok(())
}

fn terminal_size() -> (usize, usize) {
    let output = Command::new("sh")
        .args(["-c", "stty size 2>/dev/null"])
        .output();
    if let Ok(out) = output
        && out.status.success()
    {
        let content = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() == 2
            && let (Ok(rows), Ok(cols)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>())
        {
            return (cols.max(80), rows.max(24));
        }
    }
    (140, 40)
}

fn draw_dashboard(
    stdout: &mut io::Stdout,
    records: &HashMap<String, DeviceRecord>,
    metrics: &Metrics,
    start_time: Instant,
    status_line: &str,
) -> io::Result<()> {
    let (width, height) = terminal_size();

    let left_width = ((width as f32) * 0.72) as usize;
    let right_width = width.saturating_sub(left_width + 1);

    let mut rows: Vec<&DeviceRecord> = records.values().collect();
    rows.sort_by(|a, b| match (a.online, b.online) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.mac_address.cmp(&b.mac_address),
    });

    clear_screen_and_home(stdout)?;

    let online = rows.iter().filter(|r| r.online).count();
    let uptime = start_time.elapsed().as_secs();

    writeln!(
        stdout,
        "{} | online: {} | known: {} | uptime: {}s",
        "NetNeighbor Dashboard".bold().cyan(),
        online,
        rows.len(),
        uptime
    )?;
    writeln!(stdout, "{}", status_line.bright_black())?;
    writeln!(stdout, "{}", "-".repeat(width.min(220)).bright_black())?;

    let left_header = format!(
        "{:<13} {:<18} {:<17} {:<10} {:>11} {:>15}",
        "STATUS", "IP", "MAC", "IFACE", "CONNECTIONS", "DISCONNECTIONS"
    );

    writeln!(
        stdout,
        "{}{}{}",
        truncate_cell(&left_header, left_width).bold().yellow(),
        "|".bright_black(),
        truncate_cell("GLOBAL STATS", right_width).bold().yellow()
    )?;

    let mut right_lines = Vec::new();
    right_lines.push(format!(
        "Connections (All Devices): {}",
        metrics.total_connects
    ));
    right_lines.push(format!(
        "Disconnections (All Devices): {}",
        metrics.total_disconnects
    ));

    let body_lines_needed = rows.len().max(2);
    let max_body_lines = height.saturating_sub(6);
    let body_lines = body_lines_needed.min(max_body_lines).max(1);

    for line_idx in 0..body_lines {
        let left = if line_idx < rows.len() {
            let r = rows[line_idx];
            let status = if r.online {
                "CONNECTED"
            } else {
                "DISCONNECTED"
            };
            format!(
                "{:<13} {:<18} {:<17} {:<10} {:>11} {:>15}",
                status,
                r.ip_address,
                r.mac_address,
                r.interface,
                r.connect_count,
                r.disconnect_count
            )
        } else {
            String::new()
        };

        let right = if line_idx < right_lines.len() {
            right_lines[line_idx].clone()
        } else {
            String::new()
        };

        let left_cell = truncate_cell(&left, left_width);
        let right_cell = truncate_cell(&right, right_width);

        let colored_left = if line_idx < rows.len() {
            let r = rows[line_idx];
            let status_plain = if r.online {
                "CONNECTED"
            } else {
                "DISCONNECTED"
            };
            let status_colored = if r.online {
                status_plain.green().bold().to_string()
            } else {
                status_plain.red().bold().to_string()
            };

            let mut row_colored = left_cell.replacen(status_plain, &status_colored, 1);
            let disc_plain = format!("{:>15}", r.disconnect_count);
            if let Some((head, _)) = row_colored.rsplit_once(&disc_plain) {
                row_colored = format!("{}{}", head, disc_plain);
            }
            row_colored
        } else if line_idx % 2 == 0 {
            left_cell.to_string()
        } else {
            left_cell.bright_black().to_string()
        };

        let colored_right = if line_idx == 0 {
            right_cell.green().bold().to_string()
        } else if line_idx == 1 {
            right_cell.red().bold().to_string()
        } else if line_idx == 3 {
            right_cell.yellow().bold().to_string()
        } else {
            right_cell.to_string()
        };

        writeln!(
            stdout,
            "{}{}{}",
            colored_left,
            "|".bright_black(),
            colored_right
        )?;
    }

    writeln!(stdout, "{}", "-".repeat(width.min(220)).bright_black())?;
    writeln!(stdout, "{}", "Ctrl+C to stop".bright_black())?;
    stdout.flush()?;
    Ok(())
}

fn print_event_line(kind: &str, record: &DeviceRecord) {
    println!(
        "[{}] DEVICE {} - IP: {} | MAC: {} | Interface: {}",
        now_stamp(),
        kind,
        record.ip_address,
        record.mac_address,
        record.interface
    );
}

fn run_monitor(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let interface_filter = args.interface.as_deref();

    let allowed_interfaces = if interface_filter.is_some() || args.all_interfaces {
        None
    } else {
        let mut defaults = get_default_route_interfaces()?;
        if defaults.is_empty() {
            defaults = discover_non_virtual_interfaces()?;
        }
        Some(defaults)
    };

    let monitored_label = if let Some(iface) = interface_filter {
        format!("Interface: {}", iface)
    } else if args.all_interfaces {
        "Monitoring all interfaces (--all-interfaces)".to_string()
    } else if let Some(ifaces) = &allowed_interfaces {
        if ifaces.is_empty() {
            "Monitoring non-virtual interfaces".to_string()
        } else {
            let mut sorted = ifaces.iter().cloned().collect::<Vec<_>>();
            sorted.sort();
            format!("Monitoring routed interfaces: {}", sorted.join(", "))
        }
    } else {
        "Monitoring all interfaces".to_string()
    };

    let mut records: HashMap<String, DeviceRecord> = HashMap::new();
    let mut metrics = Metrics {
        total_connects: 0,
        total_disconnects: 0,
    };

    let mut last_probe_at: Option<Instant> = None;
    let mut probe_generation: u64 = 0;
    let start_time = Instant::now();

    if args.plain {
        println!("NetNeighbor - Network Connection Monitor");
        println!("Monitoring every {} seconds", args.interval);
        println!("Disconnection timeout: {} seconds", args.disconnect_timeout);
        println!("{}", monitored_label);
        println!(
            "Active probing: {}",
            if args.active_probe {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("Press Ctrl+C to stop\n");
    }

    let mut stdout = io::stdout();
    let poll_interval = Duration::from_secs(args.interval.max(1));

    loop {
        if args.active_probe
            && last_probe_at
                .map(|t| t.elapsed().as_secs() >= args.probe_interval)
                .unwrap_or(true)
        {
            let launched = active_probe_subnets(
                interface_filter,
                allowed_interfaces.as_ref(),
                args.probe_timeout_ms,
                args.max_probe_hosts,
            )
            .unwrap_or(0);
            last_probe_at = Some(Instant::now());
            probe_generation = probe_generation.saturating_add(1);

            if args.verbose && args.plain {
                println!(
                    "[{}] Active probe launched {} checks",
                    now_stamp(),
                    launched
                );
            }
        }

        let observations = get_observations(
            interface_filter,
            allowed_interfaces.as_ref(),
            args.include_ipv6,
        )?;
        let best_observations = choose_best_observations(observations);

        for (key, obs) in &best_observations {
            let is_active = obs
                .state
                .as_deref()
                .map(|s| ACTIVE_NEIGHBOR_STATES.contains(&s))
                .unwrap_or(false);

            if !is_active {
                continue;
            }

            let record = records.entry(key.clone()).or_insert_with(|| DeviceRecord {
                mac_address: obs.mac_address.clone(),
                interface: obs.interface.clone(),
                ip_address: obs.ip_address.clone(),
                online: false,
                last_active: Instant::now(),
                missed_polls: 0,
                last_probe_generation: probe_generation,
                connect_count: 0,
                disconnect_count: 0,
            });

            let was_online = record.online;
            record.online = true;
            record.ip_address = obs.ip_address.clone();
            record.last_active = Instant::now();
            record.missed_polls = 0;
            record.last_probe_generation = probe_generation;

            if !was_online {
                metrics.total_connects = metrics.total_connects.saturating_add(1);
                record.connect_count = record.connect_count.saturating_add(1);
                if args.plain {
                    print_event_line("CONNECTED", record);
                }
            }
        }

        for (key, record) in &mut records {
            if best_observations.contains_key(key) {
                continue;
            }

            if !record.online {
                continue;
            }

            record.missed_polls = record.missed_polls.saturating_add(1);
            let elapsed = record.last_active.elapsed().as_secs();
            let probe_gate_ok = if args.active_probe {
                probe_generation > record.last_probe_generation
            } else {
                true
            };

            if record.missed_polls >= args.min_missed_polls
                && elapsed >= args.disconnect_timeout
                && probe_gate_ok
            {
                if let Ok(Some(state)) =
                    refresh_neighbor_state(&record.ip_address, &record.interface)
                    && state != "FAILED"
                    && state != "INCOMPLETE"
                {
                    record.missed_polls = 0;
                    record.last_probe_generation = probe_generation;
                    if ACTIVE_NEIGHBOR_STATES.contains(&state.as_str()) {
                        record.last_active = Instant::now();
                    }
                    continue;
                }

                record.online = false;
                metrics.total_disconnects = metrics.total_disconnects.saturating_add(1);
                record.disconnect_count = record.disconnect_count.saturating_add(1);
                if args.plain {
                    print_event_line("DISCONNECTED", record);
                }
            }
        }

        records.retain(|_, rec| rec.online || rec.last_active.elapsed().as_secs() < 300);

        if !args.plain {
            let status_line = format!(
                "{} | refresh={}s | timeout={}s | missed_polls={} | probe={} {}s | ipv6={}",
                monitored_label,
                args.interval,
                args.disconnect_timeout,
                args.min_missed_polls,
                if args.active_probe { "on" } else { "off" },
                args.probe_interval,
                if args.include_ipv6 { "on" } else { "off" }
            );
            draw_dashboard(&mut stdout, &records, &metrics, start_time, &status_line)?;
        }

        thread::sleep(poll_interval);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    run_monitor(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_virtual_interfaces_by_name() {
        assert!(is_virtual_interface_name("docker0"));
        assert!(is_virtual_interface_name("br-12345"));
        assert!(!is_virtual_interface_name("wlo1"));
        assert!(!is_virtual_interface_name("eno1"));
    }

    #[test]
    fn mac_validation_works() {
        assert!(looks_like_mac("aa:bb:cc:dd:ee:ff"));
        assert!(!looks_like_mac("aa-bb-cc-dd-ee-ff"));
        assert!(!looks_like_mac("invalid"));
    }

    #[test]
    fn best_observation_prefers_ipv4_for_same_mac() {
        let observations = vec![
            DeviceObservation {
                ip_address: "fe80::1234".to_string(),
                mac_address: "aa:bb:cc:dd:ee:ff".to_string(),
                interface: "wlo1".to_string(),
                state: Some("REACHABLE".to_string()),
            },
            DeviceObservation {
                ip_address: "192.168.1.20".to_string(),
                mac_address: "aa:bb:cc:dd:ee:ff".to_string(),
                interface: "wlo1".to_string(),
                state: Some("REACHABLE".to_string()),
            },
        ];

        let best = choose_best_observations(observations);
        let key = device_key("aa:bb:cc:dd:ee:ff", "wlo1");
        assert_eq!(best.get(&key).unwrap().ip_address, "192.168.1.20");
    }

    #[test]
    fn parse_skips_ipv6_when_disabled() {
        let content = "fe80::1 dev wlo1 lladdr aa:bb:cc:dd:ee:ff STALE\n192.168.1.2 dev wlo1 lladdr aa:bb:cc:dd:ee:01 REACHABLE";
        let parsed = parse_ip_neigh_entries(content, None, None, false);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].ip_address, "192.168.1.2");
    }
}
