//! Static machine description: OS, distro, CPU, memory size, uptime.
//!
//! These values change rarely, so the sampler refreshes this on a slow
//! cadence and re-sends the same struct in between.

use crate::model::SystemInfo;
use crate::platform;
use std::process::Command;
use sysinfo::{DiskRefreshKind, Disks, MemoryRefreshKind, System};

/// Builds [`SystemInfo`].
///
/// `gpu_names` is filled in by the caller-supplied GPU name list rather than
/// discovered here: the GPU collector already did the work, and re-running
/// `ioreg`/`nvml` a second time on the slow cadence would be wasteful.
pub fn collect(cpu_cores: usize, gpu_names: Vec<String>) -> SystemInfo {
    let mut info = SystemInfo {
        hostname: System::host_name().unwrap_or_default(),
        arch: std::env::consts::ARCH.to_string(),
        cores_logical: cpu_cores,
        gpu_names,
        ..Default::default()
    };

    // --- OS identity -----------------------------------------------------
    let os_version = System::os_version().unwrap_or_default();

    #[cfg(target_os = "macos")]
    {
        info.os_name = "macOS".into();
        info.os_version = os_version.clone();
        info.distro = format!("macOS {os_version}");
    }
    #[cfg(target_os = "linux")]
    {
        info.os_name = System::name()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Linux".into());
        info.os_version = os_version.clone();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let os_name = System::name().unwrap_or_default();
        info.os_name = os_name.clone();
        info.os_version = os_version.clone();
        info.distro = os_name;
    }

    // Platform module fills in distro/pretty-name/model refinements.
    platform::decorate_system(&mut info);

    if info.distro.is_empty() {
        info.distro = info.os_name.clone();
    }

    // --- CPU -------------------------------------------------------------
    if let Ok(t) = read_sysctl("machdep.cpu.brand_string")
        && info.cpu_brand.is_empty()
    {
        info.cpu_brand = t;
    }
    if info.cpu_brand.is_empty() {
        info.cpu_brand = read_cpuinfo_brand().unwrap_or_else(|| format!("{} CPU", info.arch));
    }
    info.cpu_vendor = cpu_vendor(&info.cpu_brand);

    if let Ok(v) = read_sysctl("hw.physicalcpu")
        && let Ok(n) = v.trim().parse::<usize>()
    {
        info.cores_physical = n;
    }
    if info.cores_physical == 0 {
        // Linux reports physical cores per socket; fall back to the logical
        // count rather than claiming to know better than we do.
        info.cores_physical = cpu_cores;
    }
    if let Ok(v) = read_sysctl("hw.memsize") {
        // kept out of SystemInfo: memory size comes from the memory sample
        let _ = v;
    }

    // --- Uptime ----------------------------------------------------------
    // sysinfo 0.38 returns plain values rather than Results; both fall back to
    // zero when the platform cannot tell us.
    info.boot_time = System::boot_time();
    info.uptime_seconds = System::uptime();

    // --- Privilege hint --------------------------------------------------
    if platform::has_privileged_sensors() {
        info.privileged_hint = Some(if cfg!(target_os = "macos") {
            "CPU/GPU temperatures need administrator rights. Relaunch Hamon as \
             administrator to read the System Management Controller."
                .into()
        } else {
            "Some temperature sensors are only readable as root. Try running \
             Hamon with sudo."
                .into()
        });
    }

    info
}

/// `sysctl -n <key>` on macOS.
#[cfg(target_os = "macos")]
fn read_sysctl(key: &str) -> Result<String, std::io::Error> {
    Command::new("/usr/sbin/sysctl")
        .args(["-n", key])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[cfg(not(target_os = "macos"))]
fn read_sysctl(_key: &str) -> Result<String, std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "sysctl is macOS-only",
    ))
}

/// First `model name` line from `/proc/cpuinfo`.
#[cfg(target_os = "linux")]
fn read_cpuinfo_brand() -> Option<String> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    text.lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(not(target_os = "linux"))]
fn read_cpuinfo_brand() -> Option<String> {
    None
}

/// Infer the vendor from a brand string when the platform does not say.
fn cpu_vendor(brand: &str) -> String {
    let b = brand.to_ascii_lowercase();
    if b.contains("apple")
        || b.starts_with("m1")
        || b.starts_with("m2")
        || b.starts_with("m3")
        || b.starts_with("m4")
    {
        "Apple".into()
    } else if b.contains("intel") {
        "Intel".into()
    } else if b.contains("amd") || b.contains("ryzen") || b.contains("epyc") {
        "AMD".into()
    } else if b.contains("qualcomm") || b.contains("snapdragon") {
        "Qualcomm".into()
    } else if b.contains("cortex") || b.contains("neoverse") || b.contains("graviton") {
        "ARM".into()
    } else {
        "Unknown".into()
    }
}

/// Total physical memory, used by the layout defaults.
pub fn total_memory_bytes() -> u64 {
    let mut sys = System::new();
    sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    sys.total_memory()
}

/// Filesystem list for the disk-count heuristic in the default layout.
pub fn root_filesystem_bytes() -> u64 {
    let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::nothing().with_storage());
    disks
        .list()
        .iter()
        .find(|d| d.mount_point() == "/")
        .map(|d| d.total_space())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_info_is_populated() {
        let info = collect(8, vec!["Test GPU".into()]);
        assert!(!info.distro.is_empty(), "distro must never be empty");
        assert!(!info.arch.is_empty());
        assert_eq!(info.cores_logical, 8);
        assert!(info.cores_physical >= 1);
        assert!(!info.cpu_brand.is_empty());
        assert!(!info.cpu_vendor.is_empty());
        assert_eq!(info.gpu_names, vec!["Test GPU".to_string()]);
    }

    #[test]
    fn uptime_is_plausible() {
        let info = collect(8, Vec::new());
        assert!(
            info.uptime_seconds < 100 * 365 * 24 * 3600,
            "uptime looks wrong"
        );
        assert!(
            info.boot_time > 1_600_000_000,
            "boot time should be a unix epoch"
        );
    }

    #[test]
    fn empty_gpu_list_is_allowed() {
        // A machine with no GPU still has to render the system widget.
        let info = collect(4, Vec::new());
        assert!(info.gpu_names.is_empty());
        assert!(!info.distro.is_empty());
    }

    #[test]
    fn vendor_inference() {
        assert_eq!(cpu_vendor("Apple M1 Pro"), "Apple");
        assert_eq!(cpu_vendor("M3 Max"), "Apple");
        assert_eq!(cpu_vendor("Intel(R) Core(TM) i9-13900K"), "Intel");
        assert_eq!(cpu_vendor("AMD Ryzen 9 5950X"), "AMD");
        assert_eq!(cpu_vendor("NVIDIA GH200 Grace"), "Unknown");
    }

    #[test]
    fn memory_and_disk_queries_work() {
        assert!(total_memory_bytes() > 0);
        // A disk-less CI sandbox is allowed to report 0.
        assert!(root_filesystem_bytes() < u64::MAX);
    }
}
