//! Temperature and fan sensors.
//!
//! Linux reads `/sys/class/hwmon`, which needs no privileges. macOS needs
//! the System Management Controller, which requires root, so a non-elevated
//! process reports nothing here and the UI shows an explanatory state.

use crate::model::{Sensor, SensorKind, SensorSample};

pub struct SensorCollector {
    /// Cached on macOS because opening an SMC connection per tick would be
    /// wasteful; the availability never changes mid-run.
    #[cfg(target_os = "macos")]
    smc: Option<crate::platform::macos::smc::SmcClient>,
    /// Set once we know we cannot read sensors, to stop re-probing.
    #[cfg(target_os = "macos")]
    unavailable: bool,
}

impl Default for SensorCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorCollector {
    /// The `cfg` sits on the *fields*, not on expressions — see the note on
    /// `BatteryCollector::new`. Two cfg'd blocks here would leave the function
    /// returning `()` on whichever platform lost the first block.
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            smc: crate::platform::macos::smc::SmcClient::open(),
            #[cfg(target_os = "macos")]
            unavailable: false,
        }
    }

    pub fn sample(&mut self) -> SensorSample {
        let mut sensors = self.platform_sensors();

        // Fans are a separate hwmon interface; they are reported as sensors
        // with no temperature so the UI can render them in one list.
        #[cfg(target_os = "linux")]
        for (label, rpm) in crate::platform::linux::hwmon::fans() {
            sensors.push(Sensor {
                label,
                kind: SensorKind::Fan,
                temp_c: 0.0,
                high_c: None,
                critical_c: None,
                watts: None,
                millivolts: None,
                rpm: Some(rpm),
                needs_privileges: false,
            });
        }

        sensors.sort_by(|a, b| {
            kind_order(a.kind)
                .cmp(&kind_order(b.kind))
                .then_with(|| a.label.cmp(&b.label))
        });

        SensorSample { sensors }
    }

    #[cfg(target_os = "linux")]
    fn platform_sensors(&self) -> Vec<Sensor> {
        crate::platform::linux::hwmon::temperatures()
    }

    #[cfg(target_os = "macos")]
    fn platform_sensors(&mut self) -> Vec<Sensor> {
        if self.unavailable {
            return Vec::new();
        }
        let Some(client) = self.smc.as_ref() else {
            self.unavailable = true;
            return Vec::new();
        };
        crate::platform::macos::smc::read_temperatures(client)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn platform_sensors(&self) -> Vec<Sensor> {
        Vec::new()
    }
}

/// CPU package/core temperature, promoted out of the sensor list for the
/// headline CPU temperature readout.
impl SensorCollector {
    pub fn cpu_temperature(&self, sample: &SensorSample) -> Option<f64> {
        sample
            .sensors
            .iter()
            .filter(|s| s.kind == SensorKind::Cpu)
            // "Package id 0" is the hottest meaningful reading on Intel.
            .max_by(|a, b| {
                a.temp_c
                    .partial_cmp(&b.temp_c)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.temp_c)
    }
}

fn kind_order(kind: SensorKind) -> u8 {
    match kind {
        SensorKind::Cpu => 0,
        SensorKind::Gpu => 1,
        SensorKind::Memory => 2,
        SensorKind::Nvme => 3,
        SensorKind::Storage => 4,
        SensorKind::Battery => 5,
        SensorKind::Fan => 6,
        SensorKind::Other => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_sorted_and_labelled() {
        let mut c = SensorCollector::new();
        let s = c.sample();
        for w in s.sensors.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ok = kind_order(a.kind) < kind_order(b.kind)
                || (kind_order(a.kind) == kind_order(b.kind) && a.label <= b.label);
            assert!(ok, "sensors out of order: {} then {}", a.label, b.label);
        }
        assert!(s.sensors.iter().all(|x| !x.label.is_empty()));
    }

    #[test]
    fn cpu_temperature_prefers_the_hottest_cpu_sensor() {
        let sample = SensorSample {
            sensors: vec![
                Sensor {
                    label: "Core 1".into(),
                    kind: SensorKind::Cpu,
                    temp_c: 45.0,
                    ..Default::default()
                },
                Sensor {
                    label: "Package".into(),
                    kind: SensorKind::Cpu,
                    temp_c: 61.0,
                    ..Default::default()
                },
                Sensor {
                    label: "GPU".into(),
                    kind: SensorKind::Gpu,
                    temp_c: 70.0,
                    ..Default::default()
                },
            ],
        };
        let c = SensorCollector::new();
        assert_eq!(c.cpu_temperature(&sample), Some(61.0));
    }

    #[test]
    fn cpu_temperature_is_none_without_cpu_sensors() {
        let sample = SensorSample {
            sensors: vec![Sensor {
                label: "GPU".into(),
                kind: SensorKind::Gpu,
                temp_c: 70.0,
                ..Default::default()
            }],
        };
        assert_eq!(SensorCollector::new().cpu_temperature(&sample), None);
    }
}
