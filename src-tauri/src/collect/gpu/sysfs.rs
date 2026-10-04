//! AMD/Intel GPU discovery via the DRM sysfs interface.
//!
//! sysfs exposes *counters* (busy time, power, VRAM) but not "usage %" as
//! such. Busy-time deltas are converted to a percentage against wall-clock
//! time here, which is exactly what the kernel's `gpu_busy_percent` and
//! `radeon` fdinfo do internally.

use crate::model::GpuDevice;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DRM_ROOT: &str = "/sys/class/drm";

pub struct SysfsGpus {
    cards: Vec<Card>,
}

struct Card {
    index: usize,
    path: PathBuf,
    name: String,
    vendor: String,
    kind: String,
    driver_version: Option<String>,
    prev_busy_ns: Option<u64>,
    last: Instant,
}

impl Card {
    fn read_u64(&self, rel: &str) -> Option<u64> {
        fs::read_to_string(self.path.join(rel))
            .ok()?
            .trim()
            .parse()
            .ok()
    }
}

impl SysfsGpus {
    pub fn discover() -> Self {
        let mut cards = Vec::new();
        let Ok(entries) = fs::read_dir(DRM_ROOT) else {
            return Self { cards };
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                // `card0`, `card0-DP-1`, `renderD128`, `card0-HDMI-A-1`...
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit())
                })
            })
            .collect();
        // `read_dir` order is arbitrary; sort so GPU indices are stable.
        paths.sort_by_key(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n[4..].parse::<usize>().ok())
                .unwrap_or(usize::MAX)
        });

        for (index, path) in paths.into_iter().enumerate() {
            let Some(dev) = device_dir(&path) else {
                continue;
            };
            let uevent = fs::read_to_string(dev.join("uevent")).unwrap_or_default();
            let map = crate::platform::parse_kv_map(&uevent);
            let driver = fs::read_to_string(path.join("device/driver/module"))
                .ok()
                .and_then(|s| s.trim().rsplit('/').next().map(str::to_string));

            cards.push(Card {
                index,
                path,
                name: map
                    .get("DRIVER")
                    .cloned()
                    .map(|d| pretty_driver_name(&d))
                    .unwrap_or_else(|| map.get("PCI_ID").cloned().unwrap_or_else(|| "GPU".into())),
                vendor: vendor_from_driver(map.get("DRIVER").map(String::as_str)).to_string(),
                kind: "unknown".into(),
                driver_version: driver,
                prev_busy_ns: None,
                last: Instant::now(),
            });
        }

        Self { cards }
    }

    /// Marketing names of every card found at discovery time.
    pub fn names(&self) -> Vec<String> {
        self.cards.iter().map(|c| c.name.clone()).collect()
    }

    pub fn sample(&mut self) -> Vec<GpuDevice> {
        let now = Instant::now();
        self.cards
            .iter_mut()
            .map(|card| {
                // The 64-bit amdgpu attributes live one directory up
                // (`card0/device/gpu_busy_percent`); Intel/others expose
                // nothing, so utilisation stays absent rather than fake.
                let busy = card.read_u64("device/gpu_busy_percent");
                let usage = match (busy, card.prev_busy_ns) {
                    (Some(current), Some(prev)) if current >= prev => {
                        let busy_ns = current - prev;
                        let elapsed_ns = card.last.elapsed().as_nanos() as u64;
                        let pct = if elapsed_ns == 0 {
                            0.0
                        } else {
                            busy_ns as f64 / elapsed_ns as f64 * 100.0
                        };
                        // A busy time greater than wall clock means the
                        // counter was reset or the card shares an engine.
                        Some(pct.clamp(0.0, 100.0) as f32)
                    }
                    (Some(current), _) => {
                        // First sample for this card: no baseline yet.
                        card.prev_busy_ns = Some(current);
                        None
                    }
                    _ => None,
                };
                if let Some(b) = busy {
                    card.prev_busy_ns = Some(b);
                }
                card.last = now;

                let mem_info = read_mem_info(&card.path);

                GpuDevice {
                    index: card.index,
                    name: card.name.clone(),
                    vendor: card.vendor.clone(),
                    kind: card.kind.clone(),
                    usage_percent: usage,
                    engines: Vec::new(),
                    memory_used_bytes: mem_info.map(|m| m.0),
                    memory_total_bytes: mem_info.map(|m| m.1),
                    temperature: None,
                    power_watts: card
                        .read_u64("device/hwmon/hwmon0/power1_average")
                        .map(|u| u as f64 / 1_000_000.0),
                    power_limit_watts: None,
                    // `pwm1` is 0-255 in newer kernels, 0-100 in older ones.
                    fan_percent: card
                        .read_u64("device/hwmon/hwmon0/pwm1")
                        .map(|p| (p as f64 / 255.0 * 100.0).min(100.0) as f32),
                    // `pp_dpm_sclk` is a list of "<current> <max>" pairs in
                    // MHz; only the current value is useful here.
                    clock_mhz: read_current_mhz(&card.path.join("device/pp_dpm_sclk")),
                    memory_clock_mhz: read_current_mhz(&card.path.join("device/pp_dpm_mclk")),
                    driver_version: card.driver_version.clone(),
                }
            })
            .collect()
    }
}

/// Reads the current clock from an amdgpu `pp_dpm_*` file.
///
/// The file lists every state the GPU can switch to, one per line, with the
/// active one marked `*`:
///
/// ```text
/// SCLK: 300Mhz
/// 0: 300Mhz *
/// 1: 510Mhz
/// 2: 800Mhz
/// ```
///
/// The leading `SCLK:` summary line and the per-level lines share a shape, so
/// both are handled. Older kernels omit the `*` marker entirely, in which case
/// the first numeric reading is the active one.
fn parse_dpm_clock(text: &str) -> Option<f64> {
    let mut fallback = None;

    for line in text.lines() {
        let marked = line.contains('*');
        match parse_clock_line(line) {
            Some(mhz) if marked => return Some(mhz),
            // Kernels that omit the `*` marker entirely leave the active state
            // unstated; the first reading is the closest available answer.
            Some(mhz) => fallback = fallback.or(Some(mhz)),
            None => {}
        }
    }

    fallback
}

/// Reads the clock off a single `pp_dpm_*` line.
///
/// Lines come in several shapes depending on kernel and driver version:
///
/// ```text
/// SCLK: 300Mhz     summary line: a label, then the value
/// 0: 300Mhz *      one line per state: level, value, `*` on the active one
/// 300Mhz *         value only
/// 300 2100         `<value> <max>` pair, no unit at all
/// ```
///
/// A leading `<label>:` is therefore dropped before the value is looked for,
/// otherwise its own digits would be mistaken for the clock.
fn parse_clock_line(line: &str) -> Option<f64> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let after_label = tokens
        .iter()
        .position(|t| t.ends_with(':'))
        .map_or(0, |i| i + 1);
    tokens[after_label..].iter().find_map(|t| megahertz(t))
}

/// Parses one bare frequency token: `800Mhz` → `800.0`, `2.10GHz` → `2100.0`,
/// `2100` → `2100.0`, `*` or `1:` → `None`.
fn megahertz(token: &str) -> Option<f64> {
    let digits: String = token
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if digits.is_empty() {
        return None;
    }
    // The unit is matched strictly: a token such as `*` or an unknown suffix
    // must not be read as a bare number.
    let magnitude = match token[digits.len()..].to_ascii_lowercase().as_str() {
        "" | "mhz" | "m" => 1.0,
        "ghz" | "g" => 1000.0,
        _ => return None,
    };
    digits.parse::<f64>().ok().map(|v| v * magnitude)
}

/// Reads the active clock from a real `pp_dpm_*` file.
fn read_current_mhz(path: &std::path::Path) -> Option<f64> {
    parse_dpm_clock(&fs::read_to_string(path).ok()?)
}

/// Resolves the PCI device directory for a DRM card.
fn device_dir(card: &Path) -> Option<PathBuf> {
    let direct = card.join("device");
    if direct.is_dir() {
        return Some(direct);
    }
    // `card0/device` is usually a symlink to the PCI node; follow it and
    // confirm it actually points somewhere.
    fs::canonicalize(&direct).ok().filter(|p| p.is_dir())
}

/// `(used_bytes, total_bytes)` when the driver exposes VRAM size.
///
/// `mem_info_vram_used` / `mem_info_vram_total` are amdgpu-specific. Intel
/// exposes `mem_info_vram_total` only, so `used` is reported as unknown
/// rather than faked as zero.
fn read_mem_info(card: &Path) -> Option<(u64, u64)> {
    let total = read_u64(&card.join("device/mem_info_vram_total"))?;
    let used = read_u64(&card.join("device/mem_info_vram_used"));
    used.map(|u| (u, total)).or(Some((0, total)))
}

fn read_u64(path: &std::path::Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn vendor_from_driver(driver: Option<&str>) -> &'static str {
    match driver {
        Some("amdgpu") => "AMD",
        Some("radeon") => "AMD",
        Some("i915") | Some("xe") => "Intel",
        Some("nouveau") => "NVIDIA",
        Some("nvidia") => "NVIDIA",
        Some("virtio_gpu") => "Virtio",
        Some("vmwgfx") => "VMware",
        Some("msm") | Some("qcom") => "Qualcomm",
        _ => "Unknown",
    }
}

/// sysfs exposes driver names, not marketing names; these are as close as
/// the kernel gets.
fn pretty_driver_name(driver: &str) -> String {
    match driver {
        "amdgpu" | "radeon" => "AMD Radeon".into(),
        "i915" | "xe" => "Intel Graphics".into(),
        "nouveau" => "NVIDIA (nouveau)".into(),
        "nvidia" => "NVIDIA Graphics".into(),
        "virtio_gpu" => "Virtio GPU".into(),
        other => other.to_string(),
    }
}

/// Kept for clarity at the call site; `FrameCounter` is unused because
/// frame counts are only meaningful with a compositor-specific source.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpm_clock_parser_prefers_marked_line() {
        // The real format from `/sys/class/drm/card0/device/pp_dpm_sclk`:
        // the `SCLK:` summary line plus one line per level, `*` on the active
        // one. The summary line must not win.
        let text = "SCLK: 300Mhz\n0: 300Mhz\n1: 510Mhz\n2: 800Mhz *\n3: 1175Mhz\n";
        assert_eq!(parse_dpm_clock(text), Some(800.0));
    }

    #[test]
    fn dpm_clock_falls_back_to_first_line() {
        // Some kernels do not emit the `*` marker at all.
        let text = "0: 300Mhz\n1: 510Mhz\n2: 800Mhz\n";
        assert_eq!(parse_dpm_clock(text), Some(300.0));
    }

    #[test]
    fn dpm_clock_units_are_normalised_to_mhz() {
        assert_eq!(parse_dpm_clock("0: 2.10GHz *\n"), Some(2100.0));
        assert_eq!(parse_dpm_clock("0: 800Mhz *\n"), Some(800.0));
        // The state index must never be read as the clock.
        assert_eq!(parse_clock_line("1:"), None);
        assert_eq!(parse_clock_line("3: 1175Mhz *"), Some(1175.0));
        assert_eq!(megahertz("not-a-number"), None);
        assert_eq!(megahertz("*"), None);
        assert_eq!(megahertz(""), None);
    }

    #[test]
    fn dpm_clock_of_garbage_is_none() {
        assert_eq!(parse_dpm_clock(""), None);
        assert_eq!(parse_dpm_clock("\n\n"), None);
        assert_eq!(parse_dpm_clock("no numbers here *\n"), None);
    }

    #[test]
    fn vendor_mapping_is_total() {
        assert_eq!(vendor_from_driver(Some("amdgpu")), "AMD");
        assert_eq!(vendor_from_driver(Some("i915")), "Intel");
        assert_eq!(vendor_from_driver(Some("nouveau")), "NVIDIA");
        assert_eq!(vendor_from_driver(None), "Unknown");
        assert_eq!(vendor_from_driver(Some("brand-new-driver")), "Unknown");
    }

    #[test]
    fn discovery_never_panics_and_indexes_densely() {
        let g = SysfsGpus::discover();
        for (i, c) in g.cards.iter().enumerate() {
            assert_eq!(c.index, i);
            assert!(!c.name.is_empty());
        }
    }

    #[test]
    fn sampling_without_hwmon_is_safe() {
        let mut g = SysfsGpus::discover();
        let out = g.sample();
        for d in out {
            assert!(d.power_watts.is_none_or(|w| w.is_finite()));
            assert!(d.fan_percent.is_none_or(|f| (0.0..=100.0).contains(&f)));
        }
    }
}
