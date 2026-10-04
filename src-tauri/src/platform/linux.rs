//! Linux-specific enrichment: distro identity, hwmon sensors, disk I/O.

use crate::model::SystemInfo;
use crate::platform::parse_kv_map;
use std::fs;

pub fn decorate(info: &mut SystemInfo) {
    if let Some(release) = os_release() {
        info.distro = release
            .get("PRETTY_NAME")
            .cloned()
            .or_else(|| {
                let name = release.get("NAME")?;
                let version = release
                    .get("VERSION_ID")
                    .or_else(|| release.get("VERSION"))?;
                Some(format!("{name} {version}"))
            })
            .unwrap_or_else(|| release.get("ID").cloned().unwrap_or_default());
        info.os_name = release
            .get("NAME")
            .cloned()
            .unwrap_or_else(|| "Linux".into());
        info.os_version = release
            .get("VERSION_ID")
            .or_else(|| release.get("VERSION"))
            .cloned()
            .unwrap_or_default();
    } else {
        info.distro = "Linux".into();
        info.os_name = "Linux".into();
    }

    // `/etc/os-release` is not the whole story on NixOS: the machine may be
    // running a kernel built elsewhere, and DMI gives a friendlier board name.
    if info.model.is_empty() {
        info.model = dmi_product_name().unwrap_or_default();
    }
}

/// `os-release(5)` lookup order.
fn os_release() -> Option<std::collections::HashMap<String, String>> {
    for path in ["/etc/os-release", "/usr/lib/os-release"] {
        if let Ok(text) = fs::read_to_string(path) {
            let map = parse_kv_map(&text);
            if !map.is_empty() {
                return Some(map);
            }
        }
    }
    None
}

fn dmi_product_name() -> Option<String> {
    let raw = fs::read_to_string("/sys/devices/virtual/dmi/id/product_name").ok()?;
    let name = raw.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Whether any `/sys/class/hwmon` entry is unreadable, which on most
/// distributions means thermal/power data needs root.
pub fn has_privileged_sensors() -> bool {
    match hwmon::enumerate() {
        Ok(entries) => entries.is_empty(),
        Err(_) => true,
    }
}

/// Reads `/sys/class/hwmon/hwmon*/`.
pub mod hwmon {
    use crate::model::{Sensor, SensorKind};
    use std::path::Path;

    const HWMON_ROOT: &str = "/sys/class/hwmon";

    pub struct HwmonEntry {
        pub dir: std::path::PathBuf,
        pub name: String,
        pub temp_index: usize,
    }

    /// Every hwmon device, sorted by index so ordering is stable between
    /// ticks (otherwise widgets would reshuffle on every frame).
    pub fn enumerate() -> std::io::Result<Vec<HwmonEntry>> {
        let mut out = Vec::new();
        for item in fs::read_dir(HWMON_ROOT)? {
            let item = item?;
            let dir = item.path();
            if !dir.starts_with(HWMON_ROOT) {
                continue;
            }
            let name = fs::read_to_string(dir.join("name")).unwrap_or_default();
            let name = name.trim().to_string();
            if name.is_empty() {
                continue;
            }
            let mut temp_index = 0usize;
            while dir.join(format!("temp{}_input", temp_index + 1)).exists() {
                temp_index += 1;
            }
            out.push(HwmonEntry {
                dir,
                name,
                temp_index,
            });
        }
        out.sort_by(|a, b| a.dir.cmp(&b.dir));
        Ok(out)
    }

    fn read_f64(path: &Path) -> Option<f64> {
        fs::read_to_string(path).ok()?.trim().parse().ok()
    }

    /// Read a `temp{n}_*` attribute file.
    fn read_temp(dir: &Path, idx: usize, suffix: &str) -> Option<f64> {
        // hwmon temps are millidegrees Celsius.
        read_f64(&dir.join(format!("temp{idx}_{suffix}"))).map(|v| v / 1000.0)
    }

    /// Classify a hwmon chip name so the UI can group sensors sensibly.
    pub fn classify(chip: &str) -> SensorKind {
        let c = chip.to_ascii_lowercase();
        if c.contains("nvme") || c.contains("drivetemp") {
            SensorKind::Nvme
        } else if c.contains("k10temp")
            || c.contains("coretemp")
            || c.contains("zenpower")
            || c.contains("cpu_thermal")
            || c.contains("x86_pkg_temp")
            || c.contains("soc_thermal")
            || c.contains("cpu")
        {
            SensorKind::Cpu
        } else if c.contains("amdgpu")
            || c.contains("radeon")
            || c.contains("nouveau")
            || c.contains("i915")
            || c.contains("nvidia")
        {
            SensorKind::Gpu
        } else if c.contains("acpitz") || c.contains("thermal") {
            SensorKind::Other
        } else if c.contains("battery") || c.contains("bat") {
            SensorKind::Battery
        } else {
            SensorKind::Other
        }
    }

    /// Label for `tempN`, e.g. `Package id 0`, `Core 3`, `GPU`.
    fn temp_label(entry: &HwmonEntry, idx: usize) -> String {
        let label = fs::read_to_string(entry.dir.join(format!("temp{idx}_label")))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if !label.is_empty() {
            if entry.name == "acpitz" || entry.name == "k10temp" {
                // acpitz is always "Package id 0"; not worth repeating.
                return label;
            }
            return format!("{} {label}", entry.name);
        }
        if idx == 1 {
            entry.name.clone()
        } else {
            format!("{} {idx}", entry.name)
        }
    }

    /// All temperature sensors the kernel exposes.
    pub fn temperatures() -> Vec<Sensor> {
        let Ok(entries) = enumerate() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries {
            for idx in 1..=entry.temp_index {
                let Some(temp_c) = read_temp(&entry.dir, idx, "input") else {
                    continue;
                };
                // The kernel reports bogus huge values when a sensor is
                // absent on some firmware; keep the list plausible.
                if !(-50.0..200.0).contains(&temp_c) {
                    continue;
                }
                out.push(Sensor {
                    label: temp_label(&entry, idx),
                    kind: classify(&entry.name),
                    temp_c,
                    high_c: read_temp(&entry.dir, idx, "max"),
                    critical_c: read_temp(&entry.dir, idx, "crit"),
                    watts: None,
                    millivolts: None,
                    rpm: None,
                    needs_privileges: false,
                });
            }
        }
        out
    }

    /// Fan speeds from `fan*_input`, reported as RPM.
    pub fn fans() -> Vec<(String, f64)> {
        let Ok(entries) = enumerate() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries {
            let mut idx = 1usize;
            while let Some(rpm) = read_f64(&entry.dir.join(format!("fan{idx}_input"))) {
                if rpm > 0.0 {
                    out.push((format!("{} fan {idx}", entry.name), rpm));
                }
                idx += 1;
                if idx > 32 {
                    break;
                }
            }
        }
        out
    }

    use std::fs;
}

/// Per-device block I/O counters from `/proc/diskstats`.
pub mod diskstats {
    use crate::model::DiskIoDevice;
    use std::fs;

    /// Columns 3 (sectors read) and 7 (sectors written) from `/proc/diskstats`.
    /// Sector size is always 512 bytes regardless of the device's logical
    /// block size, so we multiply rather than using `stat -c %o`.
    const SECTOR_BYTES: u64 = 512;

    pub struct SectorCounts {
        pub device: String,
        pub read_sectors: u64,
        pub write_sectors: u64,
    }

    pub fn read() -> Vec<SectorCounts> {
        let Ok(text) = fs::read_to_string("/proc/diskstats") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            // major minor name reads reads_merged sectors_read ms_reading writes ...
            if f.len() < 10 {
                continue;
            }
            let name = f[2].to_string();
            // Skip partitions and ram/dm devices; we want whole disks.
            if name.ends_with(|c: char| c.is_ascii_digit())
                || name.starts_with("ram")
                || name.starts_with("dm-")
            {
                continue;
            }
            let read_sectors = f[5].parse::<u64>().unwrap_or(0);
            let write_sectors = f[9].parse::<u64>().unwrap_or(0);
            out.push(SectorCounts {
                device: name,
                read_sectors,
                write_sectors,
            });
        }
        out
    }

    pub fn to_bytes(c: &SectorCounts) -> (u64, u64) {
        (
            c.read_sectors.saturating_mul(SECTOR_BYTES),
            c.write_sectors.saturating_mul(SECTOR_BYTES),
        )
    }

    pub fn device_name(dev: &str) -> Option<String> {
        let path = format!("/sys/block/{dev}/device/model");
        fs::read_to_string(path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn empty_device(dev: &str) -> DiskIoDevice {
        DiskIoDevice {
            name: dev.to_string(),
            model: device_name(dev),
            read_bytes_per_sec: 0.0,
            write_bytes_per_sec: 0.0,
        }
    }
}

/// Network per-interface counters from `/proc/net/dev`.
pub mod netstats {
    use super::netlink;
    use crate::model::NetInterface;
    use std::fs;

    /// Byte and packet counters for one interface.
    pub struct Counters {
        pub name: String,
        pub rx_bytes: u64,
        pub tx_bytes: u64,
        pub rx_packets: u64,
        pub tx_packets: u64,
        pub rx_errors: u64,
        pub tx_errors: u64,
    }

    pub fn read() -> Vec<Counters> {
        let Ok(text) = fs::read_to_string("/proc/net/dev") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for line in text.lines().skip(2) {
            let Some((name, rest)) = line.split_once(':') else {
                continue;
            };
            let f: Vec<u64> = rest
                .split_whitespace()
                .map(|v| v.parse::<u64>().unwrap_or(0))
                .collect();
            if f.len() < 16 {
                continue;
            }
            out.push(Counters {
                name: name.trim().to_string(),
                rx_bytes: f[0],
                rx_packets: f[1],
                rx_errors: f[2],
                tx_bytes: f[8],
                tx_packets: f[9],
                tx_errors: f[10],
            });
        }
        out
    }

    pub fn to_interface(c: &Counters, up: bool) -> NetInterface {
        let mac = fs::read_to_string(format!("/sys/class/net/{}/address", c.name))
            .ok()
            .map(|s| s.trim().to_string());
        NetInterface {
            name: c.name.clone(),
            is_up: up,
            is_loopback: c.name == "lo",
            mac: mac.filter(|m| m != "00:00:00:00:00:00"),
            ip: netlink::ipv4_for(&c.name),
            rx_bytes: c.rx_bytes,
            tx_bytes: c.tx_bytes,
            rx_bytes_per_sec: 0.0,
            tx_bytes_per_sec: 0.0,
            rx_errors: c.rx_errors,
            tx_errors: c.tx_errors,
        }
    }
}

/// `getifaddrs(3)` binding, used only for interface addresses and state.
pub mod netlink {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int, c_uint, c_void};

    /// `struct sockaddr_in` as glibc and musl declare it.
    ///
    /// Deliberately *not* the BSD/macOS layout: that one prefixes the family with
    /// an 8-bit `sin_len` and makes `sin_family` a `u8`. Copying it here is a quiet
    /// failure — `sin_addr` still lands at offset 4 on both layouts, so the address
    /// reads correctly, but the family comparison never matches and every interface
    /// looks like it has no address. macOS has its own binding in
    /// `platform/macos/ifaddrs.rs`.
    #[repr(C)]
    struct SockaddrIn {
        sin_family: u16,
        sin_port: u16,
        sin_addr: [u8; 4],
        sin_zero: [u8; 8],
    }

    #[repr(C)]
    struct Ifaddrs {
        ifa_next: *mut Ifaddrs,
        ifa_name: *mut c_char,
        ifa_flags: c_uint,
        ifa_addr: *mut SockaddrIn,
        ifa_netmask: *mut c_void,
        ifa_dstaddr: *mut c_void,
        ifa_data: *mut c_void,
    }

    // SAFETY: both functions are plain libc entry points with no Rust-visible
    // preconditions beyond the out-pointer and the ownership handoff below.
    unsafe extern "C" {
        fn getifaddrs(ifap: *mut *mut Ifaddrs) -> c_int;
        fn freeifaddrs(ifa: *mut Ifaddrs);
    }

    const AF_INET: u16 = 2;

    /// First IPv4 address bound to `iface`, formatted dotted-quad.
    pub fn ipv4_for(iface: &str) -> Option<String> {
        let mut head: *mut Ifaddrs = std::ptr::null_mut();
        // SAFETY: `head` is a valid out-pointer; the list is freed exactly once
        // on the success path and walked only while non-null.
        let rc = unsafe { getifaddrs(&mut head) };
        if rc != 0 || head.is_null() {
            return None;
        }
        let mut found = None;
        let mut cur = head;
        while !cur.is_null() {
            // SAFETY: the kernel guarantees each node's `ifa_name` is a
            // NUL-terminated string and `ifa_addr` matches its family.
            unsafe {
                let entry = &*cur;
                if !entry.ifa_name.is_null() && !entry.ifa_addr.is_null() {
                    let name = CStr::from_ptr(entry.ifa_name).to_string_lossy();
                    let sa = &*entry.ifa_addr;
                    if name == iface && sa.sin_family == AF_INET {
                        found = Some(format!(
                            "{}.{}.{}.{}",
                            sa.sin_addr[0], sa.sin_addr[1], sa.sin_addr[2], sa.sin_addr[3]
                        ));
                        break;
                    }
                }
                cur = entry.ifa_next;
            }
        }
        // SAFETY: `head` came from `getifaddrs` and is freed once.
        unsafe { freeifaddrs(head) };
        found
    }

    /// Whether the interface reports `IFF_UP` / `IFF_RUNNING`.
    pub fn is_up(iface: &str) -> bool {
        let path = format!("/sys/class/net/{iface}/operstate");
        std::fs::read_to_string(path)
            .map(|s| matches!(s.trim(), "up" | "unknown"))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::{hwmon, netlink};
    use crate::model::SensorKind;

    #[test]
    fn finds_loopbacks_ipv4_address() {
        // The only address a test can rely on being present, on any machine.
        // This is the test that catches a wrong `sockaddr_in` layout: a
        // BSD-shaped struct still reads `sin_addr` correctly, so the address
        // looks fine, but the family never compares equal and this returns
        // `None` instead.
        assert_eq!(
            netlink::ipv4_for("lo").as_deref(),
            Some("127.0.0.1"),
            "getifaddrs binding disagrees with the platform's sockaddr_in"
        );
    }

    #[test]
    fn an_unknown_interface_has_no_address() {
        assert_eq!(netlink::ipv4_for("definitely-not-an-interface"), None);
    }

    #[test]
    fn classifies_known_chips() {
        assert_eq!(hwmon::classify("coretemp"), SensorKind::Cpu);
        assert_eq!(hwmon::classify("k10temp"), SensorKind::Cpu);
        assert_eq!(hwmon::classify("amdgpu"), SensorKind::Gpu);
        assert_eq!(hwmon::classify("nouveau"), SensorKind::Gpu);
        assert_eq!(hwmon::classify("nvme"), SensorKind::Nvme);
        assert_eq!(hwmon::classify("acpitz"), SensorKind::Other);
        assert_eq!(hwmon::classify("battery"), SensorKind::Battery);
        assert_eq!(hwmon::classify("somethingelse"), SensorKind::Other);
    }
}
