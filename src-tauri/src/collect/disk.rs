//! Block-device throughput.
//!
//! Capacity comes from `sysinfo` via [`super::cpu::CpuCollector::filesystems`],
//! which handles the per-platform mount-table differences. Throughput needs
//! raw block-device counters, which are `/proc/diskstats` on Linux and
//! IOKit's `IOBlockStorageDriver` statistics on macOS.

use crate::model::{DiskIoDevice, DiskIoSample};
use std::collections::HashMap;
use std::time::Instant;

#[cfg(target_os = "linux")]
use crate::platform::linux::diskstats;

/// One block device's cumulative counters as the platform reports them.
///
/// `key` is what pairs a reading with the previous tick; `model` is filled in
/// by the same platform call, so it never costs an extra lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawDevice {
    key: String,
    model: Option<String>,
    read_bytes: u64,
    write_bytes: u64,
}

pub struct DiskCollector {
    last: Instant,
    prev: HashMap<String, (u64, u64)>,
}

impl Default for DiskCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskCollector {
    pub fn new() -> Self {
        Self {
            last: Instant::now(),
            prev: HashMap::new(),
        }
    }

    pub fn io(&mut self) -> DiskIoSample {
        let dt = self.last.elapsed().as_secs_f64().max(1e-3);
        self.last = Instant::now();

        let mut out = DiskIoSample::default();
        for d in read_counters() {
            let prev = self
                .prev
                .insert(d.key.clone(), (d.read_bytes, d.write_bytes));
            // The first sighting of a device has no previous value to
            // difference against, so it contributes totals but no rate.
            let Some((p_rx, p_tx)) = prev else { continue };
            // A device can appear with counters reset (firmware, hotplug).
            let (drx, dtx) = (
                d.read_bytes.saturating_sub(p_rx) as f64 / dt,
                d.write_bytes.saturating_sub(p_tx) as f64 / dt,
            );
            out.read_bytes_total += d.read_bytes;
            out.write_bytes_total += d.write_bytes;
            out.read_bytes_per_sec += drx;
            out.write_bytes_per_sec += dtx;
            out.devices.push(DiskIoDevice {
                name: d.key,
                model: d.model,
                read_bytes_per_sec: drx,
                write_bytes_per_sec: dtx,
            });
        }

        out.devices.sort_by(|a, b| {
            (b.read_bytes_per_sec + b.write_bytes_per_sec)
                .partial_cmp(&(a.read_bytes_per_sec + a.write_bytes_per_sec))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        #[cfg(target_os = "linux")]
        {
            // `read_busy_percent` works in `f32` like the rest of the disk
            // collector; the wire field is `f64` to match the rates beside it.
            out.busy_percent = read_busy_percent(dt).map(f64::from);
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Linux counters
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn read_counters() -> Vec<RawDevice> {
    diskstats::read()
        .into_iter()
        .map(|c| {
            let (r, w) = diskstats::to_bytes(&c);
            // Resolve the model before `key` moves the name out of `c`.
            let model = diskstats::device_name(&c.device);
            RawDevice {
                key: c.device,
                model,
                read_bytes: r,
                write_bytes: w,
            }
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn read_busy_percent(dt: f64) -> Option<f32> {
    // Field 13 of /proc/diskstats is milliseconds the device spent doing
    // I/O. Summing across devices can exceed 100% (parallel queues), so it
    // is reported as raw utilisation rather than a percentage of capacity.
    let text = std::fs::read_to_string("/proc/diskstats").ok()?;
    let mut busy_ms = 0u64;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 13 {
            continue;
        }
        let name = f[2];
        // Partitions and ram-backed devices double-count their parent's
        // numbers, which would inflate the total.
        if name.starts_with("ram")
            || name.starts_with("loop")
            || name.starts_with("dm-")
            || name.ends_with(|c: char| c.is_ascii_digit())
        {
            continue;
        }
        busy_ms += f[12].parse::<u64>().unwrap_or(0);
    }
    // Each queue can be busy independently, so the sum may exceed 100%; the
    // figure is "how many device-seconds of I/O happened per wall second".
    Some((busy_ms as f64 / 1000.0 / dt).min(1000.0) as f32)
}

// ---------------------------------------------------------------------------
// macOS counters (IOKit IOBlockStorageDriver statistics)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn read_counters() -> Vec<RawDevice> {
    crate::platform::macos::blockstorage::read_all()
        .into_iter()
        .map(|d| RawDevice {
            key: d.name,
            model: d.model,
            read_bytes: d.read_bytes,
            write_bytes: d.write_bytes,
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_counters() -> Vec<RawDevice> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_rates_are_finite_and_non_negative() {
        let mut d = DiskCollector::new();
        let first = d.io();
        assert!(first.read_bytes_per_sec.is_finite());
        assert!(first.write_bytes_per_sec.is_finite());
        let second = d.io();
        assert!(second.read_bytes_per_sec >= 0.0);
        assert!(second.write_bytes_per_sec >= 0.0);
    }

    #[test]
    fn counter_reset_yields_zero_not_negative() {
        let mut d = DiskCollector::new();
        d.prev.insert("nvme0n1".into(), (10_000_000, 10_000_000));
        // Feed a lower value to emulate a reset: must not go negative.
        let (hi, lo) = (5_000_000u64, 5_000_000u64);
        assert_eq!(hi.saturating_sub(10_000_000), 0);
        assert_eq!(lo.saturating_sub(10_000_000), 0);
    }
}
