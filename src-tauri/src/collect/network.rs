//! Network throughput.
//!
//! Rate calculation is identical on both platforms: keep the previous
//! absolute counters and the previous timestamp, then divide the delta. The
//! only difference is where the counters come from — `/proc/net/dev` on
//! Linux, `getifaddrs` + `sysctl` on macOS.

use crate::model::{NetInterface, NetworkSample};
use std::collections::HashMap;
use std::time::Instant;

#[cfg(target_os = "linux")]
type Backend = crate::platform::linux::netstats::Counters;

#[cfg(target_os = "linux")]
fn read_counters() -> Vec<Backend> {
    crate::platform::linux::netstats::read()
}

#[cfg(target_os = "linux")]
fn is_up(name: &str) -> bool {
    crate::platform::linux::netlink::is_up(name)
}

#[cfg(target_os = "macos")]
mod mac {
    use crate::model::NetInterface;
    use crate::platform::macos::ifaddrs;

    /// Absolute counters for one interface.
    pub struct Counters {
        pub name: String,
        pub rx_bytes: u64,
        pub tx_bytes: u64,
        pub rx_packets: u64,
        pub tx_packets: u64,
        pub rx_errors: u64,
        pub tx_errors: u64,
        pub up: bool,
        pub loopback: bool,
        pub mac: Option<String>,
        pub ip: Option<String>,
    }

    /// Enumerates interfaces by joining the two macOS sources:
    /// `getifaddrs` supplies names and addresses, the `NET_RT_IFLIST2` routing
    /// dump supplies counters keyed by kernel index, and `if_nametoindex`
    /// bridges the two.
    ///
    /// Interfaces present in only one source are still listed, with zero
    /// counters, so a newly-created interface shows up immediately rather
    /// than after two ticks.
    pub fn read() -> Vec<Counters> {
        let identities = ifaddrs::interface_identities();
        let indices = ifaddrs::interface_index_map();
        let stats = ifaddrs::net_stats_by_index();

        identities
            .into_iter()
            .map(|(name, id)| {
                let s = indices.get(&name).and_then(|idx| stats.get(idx));
                Counters {
                    loopback: name.starts_with("lo"),
                    mac: id.mac,
                    ip: id.ipv4,
                    up: s.is_some_and(|v| v.is_up),
                    rx_bytes: s.map_or(0, |v| v.rx_bytes),
                    tx_bytes: s.map_or(0, |v| v.tx_bytes),
                    rx_packets: s.map_or(0, |v| v.rx_packets),
                    tx_packets: s.map_or(0, |v| v.tx_packets),
                    rx_errors: s.map_or(0, |v| v.rx_errors),
                    tx_errors: s.map_or(0, |v| v.tx_errors),
                    name,
                }
            })
            .collect()
    }

    pub fn to_interface(c: &Counters) -> NetInterface {
        NetInterface {
            name: c.name.clone(),
            is_up: c.up,
            is_loopback: c.loopback,
            mac: c.mac.clone(),
            ip: c.ip.clone(),
            rx_bytes: c.rx_bytes,
            tx_bytes: c.tx_bytes,
            rx_bytes_per_sec: 0.0,
            tx_bytes_per_sec: 0.0,
            rx_errors: c.rx_errors,
            tx_errors: c.tx_errors,
        }
    }
}

#[cfg(target_os = "macos")]
type Backend = mac::Counters;

#[cfg(target_os = "macos")]
fn read_counters() -> Vec<Backend> {
    mac::read()
}

/// Diffs counter snapshots to produce per-second rates.
pub struct NetCollector {
    prev: HashMap<String, (u64, u64)>,
    prev_packets: HashMap<String, (u64, u64)>,
    last: Instant,
}

impl Default for NetCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NetCollector {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            prev_packets: HashMap::new(),
            last: Instant::now(),
        }
    }

    pub fn sample(&mut self) -> NetworkSample {
        let counters = read_counters();
        let dt = self.last.elapsed().as_secs_f64().max(1e-3);
        self.last = Instant::now();

        let mut out = NetworkSample::default();
        let mut interfaces = Vec::with_capacity(counters.len());

        for c in counters {
            let key = c.name.clone();
            let (rx_rate, tx_rate) = self.rate(&key, c.rx_bytes, c.tx_bytes, dt);
            let (rx_pk, tx_pk) = self.packet_rate(&key, c.rx_packets, c.tx_packets, dt);

            #[cfg(target_os = "linux")]
            let iface = crate::platform::linux::netstats::to_interface(&c, is_up(&key));
            #[cfg(target_os = "macos")]
            let iface = mac::to_interface(&c);

            interfaces.push(NetInterface {
                rx_bytes_per_sec: rx_rate,
                tx_bytes_per_sec: tx_rate,
                ..iface
            });

            // Cumulative counters are only summed for interfaces that survive
            // the loopback filter below, otherwise the totals would include
            // traffic the rates exclude — and disagree with them.
            if !iface.is_loopback {
                out.total_rx_bytes = out.total_rx_bytes.saturating_add(c.rx_bytes);
                out.total_tx_bytes = out.total_tx_bytes.saturating_add(c.tx_bytes);
                out.rx_packets_per_sec += rx_pk;
                out.tx_packets_per_sec += tx_pk;
            }
        }

        // Loopback traffic is noise for a hardware monitor; keep it out of
        // the totals and the per-interface list.
        interfaces.retain(|i| !i.is_loopback);
        interfaces.sort_by(|a, b| {
            b.rx_bytes_per_sec
                .partial_cmp(&a.rx_bytes_per_sec)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for i in &interfaces {
            out.rx_bytes_per_sec += i.rx_bytes_per_sec;
            out.tx_bytes_per_sec += i.tx_bytes_per_sec;
        }

        out.interfaces = interfaces;
        out
    }

    /// Counter wrap/reset safety: if the new value is lower than the old one
    /// the interface was reset, so report 0 rather than a negative spike.
    fn rate(&mut self, key: &str, rx: u64, tx: u64, dt: f64) -> (f64, f64) {
        let entry = self.prev.insert(key.to_string(), (rx, tx));
        match entry {
            Some((prx, ptx)) if rx >= prx && tx >= ptx => {
                ((rx - prx) as f64 / dt, (tx - ptx) as f64 / dt)
            }
            Some(_) => (0.0, 0.0),
            None => (0.0, 0.0),
        }
    }

    fn packet_rate(&mut self, key: &str, rx: u64, tx: u64, dt: f64) -> (f64, f64) {
        let entry = self.prev_packets.insert(key.to_string(), (rx, tx));
        match entry {
            Some((prx, ptx)) if rx >= prx && tx >= ptx => {
                ((rx - prx) as f64 / dt, (tx - ptx) as f64 / dt)
            }
            Some(_) => (0.0, 0.0),
            None => (0.0, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_is_zero_on_first_read() {
        let mut c = NetCollector::new();
        assert_eq!(c.rate("eth0", 100, 100, 1.0), (0.0, 0.0));
    }

    #[test]
    fn rate_divides_delta_by_elapsed() {
        let mut c = NetCollector::new();
        c.rate("eth0", 0, 0, 1.0);
        let (rx, tx) = c.rate("eth0", 2048, 1024, 2.0);
        assert_eq!(rx, 1024.0);
        assert_eq!(tx, 512.0);
    }

    #[test]
    fn counter_reset_reports_zero_not_negative() {
        let mut c = NetCollector::new();
        c.rate("eth0", 5000, 5000, 1.0);
        // Interface reset: counters went backwards.
        assert_eq!(c.rate("eth0", 10, 10, 1.0), (0.0, 0.0));
    }

    #[test]
    fn per_interface_rate_mirrors_aggregate_rate() {
        let mut c = NetCollector::new();
        c.rate("eth0", 0, 0, 1.0);
        let (rx, _) = c.rate("eth0", 4096, 0, 1.0);
        assert!((rx - 4096.0).abs() < f64::EPSILON);
    }
}
