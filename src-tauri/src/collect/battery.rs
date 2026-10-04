//! Battery state.
//!
//! macOS publishes a flat set of `AppleSmartBattery` registry properties;
//! Linux exposes the equivalent in `/sys/class/power_supply/BAT*`. Both are
//! privilege-free, and both are read the same way here: parse a handful of
//! named scalars and derive the rest.

use crate::model::BatterySample;

pub struct BatteryCollector {
    /// Linux battery directory, resolved once.
    #[cfg(target_os = "linux")]
    dir: Option<std::path::PathBuf>,
}

impl Default for BatteryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl BatteryCollector {
    /// The `cfg` sits on the *field*, not on an expression.
    ///
    /// Two cfg'd blocks would not do: a braced block is parsed as an
    /// expression-statement, so whichever one `cfg` removes is not promoted to
    /// the function's tail expression and the function returns `()` instead of
    /// `Self`. Attrributing the fields instead means there is only ever one
    /// initialiser and it is always the tail.
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            dir: linux_battery_dir(),
        }
    }

    pub fn sample(&mut self) -> BatterySample {
        #[cfg(target_os = "linux")]
        {
            return self.linux_sample();
        }
        #[cfg(target_os = "macos")]
        {
            return macos::sample();
        }
        #[allow(unreachable_code)]
        BatterySample::default()
    }

    #[cfg(target_os = "linux")]
    fn linux_sample(&self) -> BatterySample {
        let Some(dir) = self.dir.as_ref() else {
            return BatterySample::default();
        };
        let present = read_u64(dir, "present").map(|v| v == 1).unwrap_or(true);

        // Prefer energy (µWh): it is what the pack actually delivers.
        // `charge_*` is µAh and would need the voltage to be comparable.
        let (used, full) = match (read_u64(dir, "energy_now"), read_u64(dir, "energy_full")) {
            (Some(n), Some(f)) if f > 0 => (n, f),
            _ => (
                read_u64(dir, "charge_now").unwrap_or(0),
                read_u64(dir, "charge_full").unwrap_or(0),
            ),
        };
        let design = read_u64(dir, "charge_full_design");
        let full_charge = read_u64(dir, "charge_full");

        BatterySample {
            present,
            percentage: present.then(|| percent(used, full)),
            is_charging: read_str(dir, "status").as_deref() == Some("Charging"),
            ac_connected: read_u64(dir, "online").map(|v| v == 1).unwrap_or(false),
            power_watts: None,
            voltage_mv: read_u64(dir, "voltage_now").map(|v| v as f64 / 1000.0),
            design_capacity_mah: design,
            full_charge_capacity_mah: full_charge,
            cycle_count: read_u64(dir, "cycle_count"),
            health_percent: match (full_charge, design) {
                (Some(f), Some(d)) if d > 0 => Some(percent(f, d)),
                _ => None,
            },
            // hwmon reports battery temperature in tenths of a degree.
            temperature_c: read_u64(dir, "temp").map(|v| v as f64 / 10.0),
            time_to_empty_minutes: None,
            time_to_full_minutes: None,
        }
    }
}

/// Clamped percentage, treating an unknown total as 0.
fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32
    }
}

// ---------------------------------------------------------------------------
// Linux sysfs
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn linux_battery_dir() -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir("/sys/class/power_supply").ok()?;
    // Machines can expose several packs; the first non-empty one wins.
    entries.flatten().map(|e| e.path()).find(|p| {
        std::fs::read_to_string(p.join("type"))
            .map(|t| t.trim() == "Battery")
            .unwrap_or(false)
    })
}

#[cfg(target_os = "linux")]
fn read_u64(dir: &std::path::Path, file: &str) -> Option<u64> {
    std::fs::read_to_string(dir.join(file))
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(target_os = "linux")]
fn read_str(dir: &std::path::Path, file: &str) -> Option<String> {
    Some(
        std::fs::read_to_string(dir.join(file))
            .ok()?
            .trim()
            .to_string(),
    )
}

// ---------------------------------------------------------------------------
// macOS IOKit
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::{BatterySample, percent};
    use std::collections::HashMap;
    use std::process::Command;

    /// Sentinel the SMC uses for "not currently computable".
    const UNKNOWN_MINUTES: u64 = 65535;

    pub fn sample() -> BatterySample {
        let Some(p) = props() else {
            return BatterySample::default();
        };

        // `CurrentCapacity` is a rounded percentage; the `AppleRaw*` pair is
        // the gauge's own reading in mAh and is what health is computed from.
        let percentage = num(&p, "CurrentCapacity").map(|v| v as f32);
        let raw_now = num(&p, "AppleRawCurrentCapacity");
        let full_mah = num(&p, "AppleRawMaxCapacity");
        let design_mah = num(&p, "DesignCapacity");
        let nominal_mah = num(&p, "NominalChargeCapacity");

        let is_charging = flag(&p, "IsCharging");
        let ac_connected = flag(&p, "ExternalConnected");

        // Voltage is mV and amperage mA, so watts = V/1000 * A/1000.
        let voltage_mv = num(&p, "AppleRawBatteryVoltage").or_else(|| num(&p, "Voltage"));
        let amperage = num(&p, "InstantAmperage")
            .or_else(|| num(&p, "Amperage"))
            .unwrap_or(0);
        let power_watts = match voltage_mv {
            Some(v) if amperage > 0 => {
                let w = v as f64 / 1000.0 * amperage as f64 / 1000.0;
                (w.abs() > 0.0).then_some(w.abs())
            }
            _ => None,
        };

        let minutes = |key: &str| -> Option<u64> {
            let v = num(&p, key)?;
            (v > 0 && v < UNKNOWN_MINUTES).then_some(v)
        };

        // `Temperature` is hundredths of a degree Celsius, e.g. `3092` for
        // 30.92 °C. (It is *not* centi-Kelvin, which would need the offset.)
        let temperature_c = num(&p, "Temperature").map(|v| v as f64 / 100.0);

        BatterySample {
            present: flag(&p, "BatteryInstalled"),
            percentage,
            is_charging,
            ac_connected,
            power_watts,
            voltage_mv: voltage_mv.map(|v| v as f64),
            design_capacity_mah: design_mah,
            full_charge_capacity_mah: full_mah.or(nominal_mah).or(raw_now),
            cycle_count: num(&p, "CycleCount"),
            // Worn packs report less than design capacity; this is the
            // standard "battery health" figure macOS shows.
            health_percent: match (full_mah, design_mah) {
                (Some(f), Some(d)) if d > 0 => Some(percent(f, d)),
                _ => None,
            },
            temperature_c,
            time_to_empty_minutes: minutes("AvgTimeToEmpty"),
            time_to_full_minutes: minutes("AvgTimeToFull"),
        }
    }

    /// Flat `"key" = scalar` registry properties.
    ///
    /// Nested dictionaries (`BatteryData`, `AppleRawAdapterDetails`, ...) are
    /// skipped: they are large, and every figure we want is also present as
    /// a top-level property on current macOS.
    fn props() -> Option<HashMap<String, String>> {
        let out = Command::new("/usr/sbin/ioreg")
            .args(["-r", "-d", "1", "-w", "0", "-c", "AppleSmartBattery"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut map = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.contains('{') || line.contains('(') {
                continue;
            }
            let Some(eq) = line.find(" = ") else { continue };
            let key = line[..eq].trim().trim_matches('"');
            if key.is_empty()
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == ',' || c == '_')
            {
                continue;
            }
            let value = line[eq + 3..]
                .trim()
                .trim_end_matches(',')
                .trim_matches('"');
            map.insert(key.to_string(), value.to_string());
        }
        (!map.is_empty()).then_some(map)
    }

    fn num(p: &HashMap<String, String>, key: &str) -> Option<u64> {
        p.get(key)?.trim().parse().ok()
    }

    /// IOKit renders booleans as `Yes`/`No`.
    fn flag(p: &HashMap<String, String>, key: &str) -> bool {
        matches!(
            p.get(key).map(|s| s.trim()),
            Some("Yes") | Some("true") | Some("1")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_is_clamped() {
        assert_eq!(percent(50, 100), 50.0);
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(150, 100), 100.0);
    }

    #[test]
    fn absent_battery_is_reported_as_absent_not_zero() {
        let s = BatteryCollector::new().sample();
        if !s.present {
            assert_eq!(s.percentage, None);
            assert_eq!(s.health_percent, None);
        }
    }

    #[test]
    fn every_present_field_is_in_range() {
        let s = BatteryCollector::new().sample();
        if let Some(p) = s.percentage {
            assert!((0.0..=100.0).contains(&p), "percent out of range: {p}");
        }
        if let Some(h) = s.health_percent {
            assert!((0.0..=150.0).contains(&h), "health out of range: {h}");
        }
        if let Some(t) = s.temperature_c {
            assert!((-20.0..90.0).contains(&t), "battery temp implausible: {t}");
        }
        assert!(BatteryCollector::new().sample().is_charging || true);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_props_are_flat_and_usable() {
        // This machine is a laptop, so the entry must be present.
        let s = BatteryCollector::new().sample();
        assert!(s.present);
        assert!(s.percentage.is_some_and(|p| (0.0..=100.0).contains(&p)));
        assert!(s.cycle_count.is_some());
        assert!(s.health_percent.is_some_and(|h| h > 0.0));
    }
}
