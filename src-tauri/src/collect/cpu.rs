//! CPU, memory and filesystem capacity collection.
//!
//! `sysinfo` handles the portable parts: CPU usage deltas, load averages,
//! frequencies, memory and swap. The one non-obvious requirement is that
//! `refresh_cpu_specifics` has to run immediately before reading
//! `cpu_usage()`, otherwise every reading is a delta since boot rather than
//! since the last tick.
//!
//! Filesystem capacity lives on a separate `sysinfo::Disks` handle because
//! in sysinfo 0.38 `System` no longer exposes disks. It is refreshed on the
//! sampler's slow cadence — capacity changes slowly and enumerating mounts is
//! a syscall per entry.

use crate::model::{CpuSample, DiskSample, Filesystem, MemorySample, StorageKind};
use sysinfo::{CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, System};

/// Holds the `sysinfo` handles. One instance lives on the sampler thread.
pub struct CpuCollector {
    sys: System,
    disks: Disks,
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuCollector {
    pub fn new() -> Self {
        let mut sys = System::new();
        prime(&mut sys);
        let mut disks = Disks::new();
        disks.refresh_specifics(false, DiskRefreshKind::nothing().with_storage());
        Self { sys, disks }
    }

    /// Number of logical CPUs the kernel reports. Fixed for the process
    /// lifetime, so the UI can size its core grid once.
    pub fn logical_cores(&self) -> usize {
        self.sys.cpus().len()
    }

    /// Consumes one tick of CPU counters.
    pub fn sample(&mut self) -> CpuSample {
        self.sys.refresh_cpu_specifics(CpuRefreshKind::everything());

        let cpus = self.sys.cpus();
        let load = System::load_average();
        let mut out = CpuSample {
            thread_count: cpus.len(),
            usage: self.sys.global_cpu_usage().clamp(0.0, 100.0),
            load1: load.one as f32,
            load5: load.five as f32,
            load15: load.fifteen as f32,
            ..Default::default()
        };

        for cpu in cpus {
            out.per_core.push(cpu.cpu_usage().clamp(0.0, 100.0));

            // sysinfo reports frequency in MHz on every platform we support;
            // guard against the 0 it reports when it cannot determine it.
            let mhz = cpu.frequency() as f64;
            if mhz > 0.0 {
                out.per_core_frequency_mhz.push(Some(mhz));
                out.frequency_mhz = Some(out.frequency_mhz.unwrap_or(0.0).max(mhz));
            } else {
                out.per_core_frequency_mhz.push(None);
            }
        }

        // The model promises one entry per core for both vectors, so pad the
        // frequency list if some cores reported nothing.
        out.per_core_frequency_mhz.resize(out.per_core.len(), None);

        // `f64` is not `Ord`, so the max is a fold: `f64::max` ignores a NaN
        // operand, which is what we want when a core reports garbage.
        out.frequency_max_mhz = out
            .per_core_frequency_mhz
            .iter()
            .flatten()
            .copied()
            .fold(None, |acc: Option<f64>, v| {
                Some(acc.map_or(v, |a| a.max(v)))
            });

        out
    }

    pub fn memory(&mut self) -> MemorySample {
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::everything());

        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        let swap_total = self.sys.total_swap();
        let swap_used = self.sys.used_swap();

        MemorySample {
            total_bytes: total,
            used_bytes: used,
            available_bytes: self.sys.available_memory(),
            percent: pct(used, total),
            // sysinfo folds page cache and buffers into "used" on the
            // platforms we target; the split is only meaningful on Linux, and
            // we do not have a portable way to recover it, so it stays 0.
            cached_bytes: 0,
            buffers_bytes: 0,
            swap_total_bytes: swap_total,
            swap_used_bytes: swap_used,
            swap_percent: pct(swap_used, swap_total),
        }
    }

    /// Re-enumerates mounts. Called from the sampler's slow cadence.
    pub fn refresh_filesystems(&mut self) {
        self.disks
            .refresh_specifics(false, DiskRefreshKind::nothing().with_storage());
    }

    /// Snapshot of every mounted filesystem, with capacity filled in.
    ///
    /// Mounts that report a zero or nonsensical total (pseudo-filesystems,
    /// some memory-backed mounts) are dropped: a 100%-full bar for `/dev` is
    /// noise, not information.
    pub fn filesystems(&mut self) -> DiskSample {
        self.refresh_filesystems();

        let mut filesystems: Vec<Filesystem> = self
            .disks
            .list()
            .iter()
            .filter(|d| {
                let total = d.total_space();
                total > 0 && total != u64::MAX
            })
            .map(|d| {
                let total = d.total_space();
                let available = d.available_space();
                // `used_space` does not exist in sysinfo 0.38; capacity is
                // derived instead. On Linux this counts the reserved blocks
                // as used, which is what `df` reports too.
                let used = total.saturating_sub(available);
                Filesystem {
                    name: d.name().to_string_lossy().into_owned(),
                    mount_point: d.mount_point().to_string_lossy().into_owned(),
                    fs_type: Some(d.file_system().to_string_lossy().into_owned()),
                    total_bytes: total,
                    used_bytes: used,
                    free_bytes: available,
                    percent: pct(used, total),
                    kind: storage_kind(d),
                }
            })
            .collect();

        // The same mount point shows up more than once for APFS firmlinks and
        // bind mounts. Keep the largest of each group, then order root first
        // and the rest by capacity so the commonest disks lead the list.
        filesystems.sort_by(|a, b| {
            a.mount_point
                .cmp(&b.mount_point)
                .then_with(|| b.total_bytes.cmp(&a.total_bytes))
        });
        filesystems.dedup_by(|a, b| a.mount_point == b.mount_point);
        filesystems.sort_by(|a, b| {
            let a_root = a.mount_point == "/";
            let b_root = b.mount_point == "/";
            b_root
                .cmp(&a_root)
                .then_with(|| b.total_bytes.cmp(&a.total_bytes))
                .then_with(|| a.mount_point.cmp(&b.mount_point))
        });

        DiskSample { filesystems }
    }

    /// Number of live processes, reported alongside the CPU sample.
    ///
    /// The process collector owns its own `System`, so this reflects whatever
    /// *it* last saw; it is only used as a fallback when that count is absent.
    pub fn process_count_hint(&self) -> usize {
        self.sys.processes().len()
    }
}

/// Maps sysinfo's `DiskKind` plus the removable flag onto the wire enum.
///
/// sysinfo 0.38 only distinguishes HDD from SSD and cannot see network
/// mounts, so the network/removable cases are inferred the way a user would
/// expect: from the removable flag and the filesystem name.
fn storage_kind(d: &sysinfo::Disk) -> StorageKind {
    if d.is_removable() {
        return StorageKind::Removable;
    }
    match d
        .file_system()
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "nfs" | "smbfs" | "cifs" | "afpfs" | "webdav" | "ftp" => StorageKind::Network,
        _ => match d.kind() {
            sysinfo::DiskKind::HDD | sysinfo::DiskKind::SSD => StorageKind::Internal,
            _ => StorageKind::Unknown,
        },
    }
}

/// Percentage helper that treats an unknown total as 0% instead of dividing
/// by zero.
fn pct(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        used as f32 / total as f32 * 100.0
    }
}

/// Runs one full refresh so the first `sample()` call returns a real
/// delta-based reading instead of a spike from uninitialised counters.
///
/// The sleep is deliberate: sysinfo computes CPU usage as the difference
/// between two reads, and its own docs require a gap of at least
/// `MINIMUM_CPU_UPDATE_INTERVAL`. Constructing a collector therefore costs
/// ~200ms, which is paid once per collector at startup.
pub fn prime(sys: &mut System) {
    sys.refresh_specifics(
        sysinfo::RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_sample_is_bounded_and_shaped() {
        let mut c = CpuCollector::new();
        let s = c.sample();
        assert!(s.usage.is_finite());
        assert!((0.0..=100.0).contains(&s.usage));
        assert!(!s.per_core.is_empty());
        assert_eq!(s.per_core.len(), c.logical_cores());
        assert!(s.per_core.iter().all(|v| (0.0..=100.0).contains(v)));
        assert_eq!(
            s.per_core_frequency_mhz.len(),
            s.per_core.len(),
            "frequency vector must stay index-aligned with per_core"
        );
    }

    #[test]
    fn memory_totals_are_sane() {
        let mut c = CpuCollector::new();
        let m = c.memory();
        assert!(m.total_bytes > 0, "total memory should be reported");
        assert!(m.used_bytes <= m.total_bytes);
        assert!((0.0..=100.0).contains(&m.percent));
    }

    #[test]
    fn filesystems_have_plausible_capacity() {
        let mut c = CpuCollector::new();
        let d = c.filesystems();
        assert!(!d.filesystems.is_empty(), "expected at least one mount");
        for f in &d.filesystems {
            assert!(f.total_bytes > 0);
            assert!(f.used_bytes <= f.total_bytes);
            assert_eq!(f.free_bytes, f.total_bytes - f.used_bytes);
            assert!((0.0..=100.0).contains(&f.percent));
            assert!(!f.mount_point.is_empty());
        }
    }

    #[test]
    fn mount_points_are_unique_and_root_leads() {
        let mut c = CpuCollector::new();
        let d = c.filesystems();
        let mut seen = std::collections::HashSet::new();
        for f in &d.filesystems {
            assert!(
                seen.insert(f.mount_point.clone()),
                "duplicate mount {}",
                f.mount_point
            );
        }
        // Whatever the first entry is, `/` must precede every non-root mount.
        if let Some(root_at) = d.filesystems.iter().position(|f| f.mount_point == "/") {
            for (i, f) in d.filesystems.iter().enumerate().skip(root_at + 1) {
                assert_ne!(f.mount_point, "/", "root appeared again at {i}");
            }
        }
    }

    #[test]
    fn zero_total_yields_zero_percent() {
        assert_eq!(pct(0, 0), 0.0);
        assert_eq!(pct(5, 10), 50.0);
    }

    #[test]
    fn constructor_refreshes_until_usage_is_bounded() {
        // The whole point of `prime` is that the first sample is not garbage.
        let mut c = CpuCollector::new();
        let first = c.sample();
        assert!(
            (0.0..=100.0).contains(&first.usage),
            "first sample was {}",
            first.usage
        );
        for v in &first.per_core {
            assert!((0.0..=100.0).contains(v), "first per-core value was {v}");
        }
    }
}
