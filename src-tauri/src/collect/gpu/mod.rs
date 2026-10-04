//! GPU collection.
//!
//! Three independent backends, all merged into one device list:
//!
//! * **NVIDIA** — NVML loaded dynamically at runtime, so the app works on
//!   machines without the driver installed.
//! * **macOS** — `IOAccelerator` registry properties.
//! * **Linux** — `/sys/class/drm/card*/` for AMD/Intel, plus the NVML path.

use crate::model::{GpuDevice, GpuSample};
use std::collections::HashMap;

mod nvidia;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod sysfs;

#[cfg(target_os = "macos")]
mod iokit;

pub struct GpuCollector {
    nvml: Option<nvidia::Nvml>,
    #[cfg(target_os = "linux")]
    sysfs: sysfs::SysfsGpus,
    #[cfg(target_os = "macos")]
    iokit: iokit::IokitGpus,
}

impl Default for GpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuCollector {
    pub fn new() -> Self {
        Self {
            nvml: nvidia::Nvml::load(),
            #[cfg(target_os = "linux")]
            sysfs: sysfs::SysfsGpus::discover(),
            #[cfg(target_os = "macos")]
            iokit: iokit::IokitGpus::discover(),
        }
    }

    /// Names of every GPU this machine exposes.
    ///
    /// Discovered once at startup rather than read per tick, because the set
    /// of devices cannot change without a reboot (a `dGPU`/`eGPU` switch does
    /// not add or remove an accelerator, it only changes which is busy).
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        if self.nvml.is_some() {
            // NVML's device list is filled in by the first `sample()`, so the
            // names are whatever the backend already knows statically.
            names.extend(self.nvml.iter().flat_map(|n| n.device_names()));
        }
        #[cfg(target_os = "linux")]
        names.extend(self.sysfs.names());
        #[cfg(target_os = "macos")]
        names.extend(self.iokit.names());
        disambiguate(&mut names);
        names
    }

    pub fn sample(&mut self) -> GpuSample {
        let mut devices: Vec<GpuDevice> = Vec::new();

        if let Some(nvml) = self.nvml.as_mut() {
            match nvml.sample() {
                Ok(list) => devices.extend(list),
                Err(e) => log::warn!("nvml sample failed: {e}"),
            }
        }

        #[cfg(target_os = "linux")]
        devices.extend(self.sysfs.sample());
        #[cfg(target_os = "macos")]
        devices.extend(self.iokit.sample());

        // Backends report the marketing string they were given; numbering the
        // duplicates happens once, here, so a device cannot end up with
        // "Apple M1 Pro #2" from two different layers of suffixing.
        let mut names: Vec<String> = Vec::new();
        for (i, d) in devices.iter_mut().enumerate() {
            d.index = i;
            names.push(d.name.clone());
        }
        disambiguate(&mut names);
        for (d, name) in devices.iter_mut().zip(names) {
            d.name = name;
        }

        GpuSample { devices }
    }
}

/// Appends ` #2`, ` #3`, … to repeated names, leaving the first untouched.
///
/// Mutates in place so callers that already hold parallel vectors (the device
/// list and its name list) can share one pass.
fn disambiguate(names: &mut [String]) {
    // Counted on the original strings, so a name that already gained a suffix
    // is never counted again ("Apple M1 Pro #2" must not become "#3").
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    for name in names.iter() {
        // `count` is the number of earlier occurrences, so the suffix is
        // one-based.
        let count = seen.entry(name.clone()).or_insert(0);
        out.push(if *count > 0 {
            format!("{name} #{}", *count + 1)
        } else {
            name.clone()
        });
        *count += 1;
    }
    names.clone_from_slice(&out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_never_panics() {
        let mut c = GpuCollector::new();
        let s = c.sample();
        // Every device must carry a name the UI can render.
        assert!(s.devices.iter().all(|d| !d.name.is_empty()));
    }

    #[test]
    fn duplicate_names_are_disambiguated() {
        let mut names = vec![
            "Apple M1".to_string(),
            "Apple M1".to_string(),
            "AMD".to_string(),
        ];
        disambiguate(&mut names);
        assert_eq!(names, vec!["Apple M1", "Apple M1 #2", "AMD"]);
    }

    #[test]
    fn already_suffixed_names_are_not_double_suffixed() {
        // The old per-backend implementation produced "Apple M1 #2 #2" because
        // it counted the name it had just rewritten.
        let mut names = vec!["Apple M1 #2".to_string(), "Apple M1 #2".to_string()];
        disambiguate(&mut names);
        assert_eq!(names, vec!["Apple M1 #2", "Apple M1 #2 #2"]);
    }

    #[test]
    fn triple_duplicates_get_sequential_suffixes() {
        let mut names = vec!["GPU".into(), "GPU".into(), "GPU".into(), "GPU".into()];
        disambiguate(&mut names);
        assert_eq!(names, vec!["GPU", "GPU #2", "GPU #3", "GPU #4"]);
    }

    #[test]
    fn discover_reports_names_without_sampling() {
        let c = GpuCollector::new();
        let names = c.names();
        assert!(
            names.iter().all(|n| !n.is_empty()),
            "GPU names are rendered directly in the UI: {names:?}"
        );
        // Names must already be unique after disambiguation, otherwise the
        // system-info widget shows two identical rows.
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "duplicate GPU names: {names:?}");
    }
}
