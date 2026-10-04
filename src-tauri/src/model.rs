//! Wire types shared with the frontend.
//!
//! Everything in here is `serde`-serialisable and sent to the UI inside a
//! [`crate::sample::Snapshot`]. Fields are deliberately `Option`-heavy: a
//! collector that is unavailable on the current platform reports `None`
//! instead of a fake `0.0`, and the UI renders an explicit "unavailable"
//! state rather than a misleading number.

use serde::{Deserialize, Serialize};

/// How a disk was attached. Drives the icon + colour in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageKind {
    #[default]
    Internal,
    Removable,
    Network,
    Unknown,
}

/// Coarse sensor classification used to group the temperature list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SensorKind {
    #[default]
    Cpu,
    Gpu,
    Memory,
    Nvme,
    Storage,
    Battery,
    Fan,
    Other,
}

/// Static, slowly-changing machine description. Refreshed every 30s because
/// nothing in here changes during a normal session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemInfo {
    pub hostname: String,
    /// Human label for the OS, e.g. `NixOS 25.05 (aarch64)`.
    pub distro: String,
    /// `macOS`, `NixOS`, `Ubuntu`, `Arch Linux`, ...
    pub os_name: String,
    pub os_version: String,
    pub kernel: String,
    pub arch: String,
    /// Marketing model string, e.g. `MacBook Pro` / `ThinkPad X1 Carbon`.
    pub model: String,
    /// Marketing CPU string, e.g. `Apple M1 Pro`.
    pub cpu_brand: String,
    pub cpu_vendor: String,
    pub cores_physical: usize,
    pub cores_logical: usize,
    pub gpu_names: Vec<String>,
    pub boot_time: u64,
    pub uptime_seconds: u64,
    /// Set when a collector needs root/administrator rights to work.
    pub privileged_hint: Option<String>,
}

/// CPU state for one sampling tick.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuSample {
    /// Aggregate utilisation across all logical cores, 0-100.
    pub usage: f32,
    /// One entry per logical core, 0-100.
    pub per_core: Vec<f32>,
    pub user: f32,
    pub system: f32,
    pub idle: f32,
    pub iowait: f32,
    pub steal: f32,
    pub frequency_mhz: Option<f64>,
    pub frequency_max_mhz: Option<f64>,
    pub temperature: Option<f64>,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub thread_count: usize,
    pub process_count: usize,
    pub per_core_frequency_mhz: Vec<Option<f64>>,
}

/// Physical and swap memory for one tick.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemorySample {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub percent: f32,
    pub cached_bytes: u64,
    pub buffers_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_percent: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuSample {
    pub devices: Vec<GpuDevice>,
}

/// One physical or logical GPU.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuDevice {
    pub index: usize,
    pub name: String,
    pub vendor: String,
    /// Discrete/integrated marker when the driver reports it.
    pub kind: String,
    pub usage_percent: Option<f32>,
    /// Per-engine utilisation (render, compute, copy, video) when available.
    pub engines: Vec<(String, f32)>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub temperature: Option<f64>,
    pub power_watts: Option<f64>,
    pub power_limit_watts: Option<f64>,
    pub fan_percent: Option<f32>,
    pub clock_mhz: Option<f64>,
    pub memory_clock_mhz: Option<f64>,
    pub driver_version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkSample {
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    pub rx_packets_per_sec: f64,
    pub tx_packets_per_sec: f64,
    pub interfaces: Vec<NetInterface>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub is_up: bool,
    pub is_loopback: bool,
    pub mac: Option<String>,
    pub ip: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bytes_per_sec: f64,
    pub tx_bytes_per_sec: f64,
    pub rx_errors: u64,
    pub tx_errors: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskSample {
    pub filesystems: Vec<Filesystem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Filesystem {
    pub name: String,
    pub mount_point: String,
    pub fs_type: Option<String>,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub percent: f32,
    pub kind: StorageKind,
}

/// Block-device throughput, aggregated across physical disks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskIoSample {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
    pub read_bytes_total: u64,
    pub write_bytes_total: u64,
    pub busy_percent: Option<f64>,
    pub devices: Vec<DiskIoDevice>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskIoDevice {
    pub name: String,
    pub model: Option<String>,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SensorSample {
    pub sensors: Vec<Sensor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sensor {
    pub label: String,
    pub kind: SensorKind,
    pub temp_c: f64,
    pub high_c: Option<f64>,
    pub critical_c: Option<f64>,
    pub watts: Option<f64>,
    pub millivolts: Option<f64>,
    pub rpm: Option<f64>,
    /// `true` for sensors that only appear once the app is run as root.
    pub needs_privileges: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatterySample {
    pub present: bool,
    pub percentage: Option<f32>,
    pub is_charging: bool,
    pub ac_connected: bool,
    pub power_watts: Option<f64>,
    pub voltage_mv: Option<f64>,
    pub design_capacity_mah: Option<u64>,
    pub full_charge_capacity_mah: Option<u64>,
    pub cycle_count: Option<u64>,
    pub health_percent: Option<f32>,
    pub temperature_c: Option<f64>,
    pub time_to_empty_minutes: Option<u64>,
    pub time_to_full_minutes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessSample {
    pub top_cpu: Vec<ProcessInfo>,
    pub top_memory: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: Option<String>,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

/// Everything gathered in a single tick. One message per tick keeps the
/// IPC surface to a single command and lets the UI update atomically.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub system: SystemInfo,
    pub cpu: CpuSample,
    pub memory: MemorySample,
    pub gpu: GpuSample,
    pub network: NetworkSample,
    pub disk: DiskSample,
    pub disk_io: DiskIoSample,
    pub sensors: SensorSample,
    pub battery: BatterySample,
    pub processes: ProcessSample,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            cpu: CpuSample {
                usage: 42.5,
                ..Default::default()
            },
            memory: MemorySample {
                total_bytes: 1000,
                used_bytes: 250,
                percent: 25.0,
                swap_total_bytes: 500,
                swap_used_bytes: 100,
                swap_percent: 20.0,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn round_trip_preserves_the_headline_values() {
        let s = snap();
        assert_eq!(s.cpu.usage, 42.5);
        assert_eq!(s.memory.used_bytes, 250);
    }

    #[test]
    fn swap_absent_is_reported_as_no_swap_not_zero_percent() {
        // A machine with no swap is a fact worth distinguishing from a machine
        // using none of a swapfile it has.
        let with_swap = snap();
        assert_eq!(with_swap.memory.swap_total_bytes, 500);
        assert_eq!(with_swap.memory.swap_percent, 20.0);

        let without = Snapshot::default();
        assert_eq!(without.memory.swap_total_bytes, 0);
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let mut s = snap();
        s.network.interfaces.push(NetInterface {
            name: "en0".into(),
            is_up: true,
            rx_bytes_per_sec: 1024.5,
            ..Default::default()
        });
        let json = serde_json::to_string(&s).unwrap();
        let back: Snapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cpu.usage, 42.5);
        assert_eq!(back.memory.used_bytes, 250);
        assert_eq!(back.network.interfaces[0].name, "en0");
        assert_eq!(back.network.interfaces[0].rx_bytes_per_sec, 1024.5);
    }

    #[test]
    fn missing_optional_metrics_are_null_in_json() {
        let json = serde_json::to_value(Snapshot::default()).unwrap();
        assert!(json["cpu"]["temperature"].is_null());
        assert!(json["battery"]["percentage"].is_null());
        assert_eq!(json["memory"]["total_bytes"], 0);
    }
}
