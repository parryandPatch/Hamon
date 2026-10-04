//! NVML, loaded at runtime via `dlopen`.
//!
//! Dynamic loading is deliberate: linking NVML at build time would make the
//! app refuse to start on machines without the NVIDIA driver, and bundling it
//! would be a licensing problem. If the library is missing, the collector
//! simply reports no NVIDIA GPUs.

use crate::model::GpuDevice;
use libloading::Library;
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};

/// Opaque handle returned by `nvmlDeviceGetHandleByIndex_v2`.
/// Opaque device handle. `repr(transparent)` over a pointer so passing it
/// by value to the NVML ABI is identical to passing the raw pointer, while
/// still keeping the raw pointer out of the rest of the file.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct NvmlDevice(*mut c_void);
unsafe impl Send for NvmlDevice {}

/// Value/flag constants used below. These are stable parts of the NVML ABI.
const SUCCESS: c_int = 0;

const TEMPERATURE_GPU: c_int = 0;

type FnInit = unsafe extern "system" fn() -> c_int;
type FnShutdown = unsafe extern "system" fn() -> c_int;
type FnCount = unsafe extern "system" fn(*mut c_int) -> c_int;
type FnHandleByIndex = unsafe extern "system" fn(c_int, *mut NvmlDevice) -> c_int;
type FnName = unsafe extern "system" fn(NvmlDevice, *mut c_char, c_int) -> c_int;
type FnUuid = unsafe extern "system" fn(NvmlDevice, *mut c_char, c_int) -> c_int;
type FnTemp = unsafe extern "system" fn(NvmlDevice, c_int, *mut c_int) -> c_int;
type FnUtil = unsafe extern "system" fn(NvmlDevice, *mut UtilRates) -> c_int;
type FnPower = unsafe extern "system" fn(NvmlDevice, *mut c_uint) -> c_int;
type FnPowerLimit = unsafe extern "system" fn(NvmlDevice, *mut c_uint) -> c_int;
type FnFanSpeed = unsafe extern "system" fn(NvmlDevice, *mut c_int) -> c_int;
type FnClock = unsafe extern "system" fn(NvmlDevice, c_int, *mut c_int) -> c_int;
type FnMem = unsafe extern "system" fn(NvmlDevice, *mut MemoryInfo) -> c_int;
type FnDriverVersion = unsafe extern "system" fn(*mut c_char, c_int) -> c_int;

/// Runtime handle. Holds the `dlopen` handle for the process lifetime.
///
/// `lib` is never read directly — it exists so the dynamic library stays
/// loaded for as long as the function pointers below are callable.
pub struct Nvml {
    _lib: Library,
    init: FnInit,
    shutdown: FnShutdown,
    count: FnCount,
    handle_by_index: FnHandleByIndex,
    name: FnName,
    uuid: FnUuid,
    temp: FnTemp,
    util: FnUtil,
    power: FnPower,
    power_limit: FnPowerLimit,
    fan: FnFanSpeed,
    clock: FnClock,
    mem: FnMem,
    driver_version: FnDriverVersion,
    driver: Option<String>,
}

/// NVML reports string buffers with a fixed capacity; 96 bytes covers the
/// longest documented value (`nvmlDeviceGetName`).
const STR_LEN: usize = 96;

/// `nvmlUtilization_t` — two `unsigned int` percentages, in that order.
///
/// Modelled as a struct rather than `[c_uint; 2]` so the field order is
/// checked by the compiler against the NVML headers' intent.
#[repr(C)]
struct UtilRates {
    gpu: c_uint,
    memory: c_uint,
}

/// `nvmlMemory_t` — `total`, `free`, `used`, all `unsigned long long`.
///
/// The layout is identical on LP64 and LLP64 for this struct, so a `u64`
/// field type is correct on both.
#[repr(C)]
struct MemoryInfo {
    total: u64,
    free: u64,
    used: u64,
}

impl Nvml {
    /// Tries each known install location. Returns `None` when no usable
    /// NVML is present, which is the normal case on Apple Silicon and on
    /// AMD-only Linux boxes.
    pub fn load() -> Option<Self> {
        const CANDIDATES: &[&str] = &[
            "libnvidia-ml.so.1",
            "libnvidia-ml.so",
            "/usr/lib/x86_64-linux-gnu/libnvidia-ml.so.1",
            "/usr/lib64/libnvidia-ml.so.1",
        ];
        for path in CANDIDATES {
            // SAFETY: loading a well-known system library; all symbols are
            // resolved below and only called with correct signatures.
            // SAFETY: each candidate is a well-known system library; every
            // symbol is resolved through `lib.get` below and only ever called
            // with the signatures NVML documents for it.
            let Ok(lib) = (unsafe { Library::new(*path) }) else {
                continue;
            };
            let ok = unsafe {
                macro_rules! sym {
                    ($name:literal, $ty:ty) => {
                        lib.get::<$ty>(concat!($name, "\0").as_bytes())
                            .ok()
                            .map(|s| *s)
                    };
                }
                match (
                    sym!("nvmlInit_v2", FnInit),
                    sym!("nvmlShutdown", FnShutdown),
                    sym!("nvmlDeviceGetCount_v2", FnCount),
                    sym!("nvmlDeviceGetHandleByIndex_v2", FnHandleByIndex),
                    sym!("nvmlDeviceGetName", FnName),
                    sym!("nvmlDeviceGetUUID", FnUuid),
                    sym!("nvmlDeviceGetTemperature", FnTemp),
                    sym!("nvmlDeviceGetUtilizationRates", FnUtil),
                    sym!("nvmlDeviceGetPowerUsage", FnPower),
                    sym!("nvmlDeviceGetEnforcedPowerLimit", FnPowerLimit),
                    sym!("nvmlDeviceGetFanSpeed", FnFanSpeed),
                    sym!("nvmlDeviceGetClockInfo", FnClock),
                    sym!("nvmlDeviceGetMemoryInfo", FnMem),
                    sym!("nvmlSystemGetDriverVersion", FnDriverVersion),
                ) {
                    (
                        Some(a),
                        Some(b),
                        Some(c),
                        Some(d),
                        Some(e),
                        Some(f),
                        Some(g),
                        Some(h),
                        Some(i),
                        Some(j),
                        Some(k),
                        Some(l),
                        Some(m),
                        Some(n),
                    ) => Some(Self {
                        _lib: lib,
                        init: a,
                        shutdown: b,
                        count: c,
                        handle_by_index: d,
                        name: e,
                        uuid: f,
                        temp: g,
                        util: h,
                        power: i,
                        power_limit: j,
                        fan: k,
                        clock: l,
                        mem: m,
                        driver_version: n,
                        driver: None,
                    }),
                    _ => None,
                }
            };
            if let Some(mut nvml) = ok {
                // SAFETY: symbols are from a successfully loaded NVML.
                let rc = unsafe { (nvml.init)() };
                if rc != SUCCESS {
                    log::info!("nvml found at {path} but init failed ({rc})");
                    continue;
                }
                nvml.driver = nvml.read_driver_version();
                log::info!("nvml loaded from {path}");
                return Some(nvml);
            }
        }
        None
    }

    /// Marketing names of every device NVML can see.
    ///
    /// Cheaper than a full [`Nvml::sample`]: only two NVML calls per card
    /// instead of a dozen.
    pub fn device_names(&self) -> Vec<String> {
        let mut count: c_int = 0;
        // SAFETY: `count` is a valid out-pointer.
        if unsafe { (self.count)(&mut count) } != SUCCESS {
            return Vec::new();
        }
        (0..count.max(0))
            .filter_map(|i| {
                let mut dev = NvmlDevice(std::ptr::null_mut());
                // SAFETY: `dev` is a valid out-pointer.
                if unsafe { (self.handle_by_index)(i, &mut dev) } != SUCCESS {
                    return None;
                }
                Some(self.display_name(&self.read_string(|buf| unsafe {
                    (self.name)(dev, buf.as_mut_ptr(), STR_LEN as c_int)
                })))
            })
            .collect()
    }

    fn read_driver_version(&self) -> Option<String> {
        let mut buf = vec![0i8; STR_LEN];
        // SAFETY: `buf` is `STR_LEN` bytes, which matches `nvmlReturnValue`.
        if unsafe { (self.driver_version)(buf.as_mut_ptr(), STR_LEN as c_int) } != SUCCESS {
            return None;
        }
        let c: *const c_char = buf.as_ptr().cast();
        // SAFETY: NVML NUL-terminates on success.
        let s = unsafe { CStr::from_ptr(c) }.to_string_lossy().into_owned();
        (!s.is_empty()).then_some(s)
    }

    /// One tick of data for every visible NVIDIA GPU.
    pub fn sample(&mut self) -> Result<Vec<GpuDevice>, String> {
        let mut count: c_int = 0;
        // SAFETY: `count` is a valid out-pointer.
        if unsafe { (self.count)(&mut count) } != SUCCESS {
            return Err("nvmlDeviceGetCount failed".into());
        }

        let mut out = Vec::with_capacity(count.max(0) as usize);
        for i in 0..count.max(0) {
            let mut dev = NvmlDevice(std::ptr::null_mut());
            // SAFETY: `dev` is a valid out-pointer.
            if unsafe { (self.handle_by_index)(i, &mut dev) } != SUCCESS {
                continue;
            }
            out.push(self.read_device(dev));
        }
        Ok(out)
    }

    fn read_device(&self, dev: NvmlDevice) -> GpuDevice {
        let name =
            self.read_string(|buf| unsafe { (self.name)(dev, buf.as_mut_ptr(), STR_LEN as c_int) });
        let uuid =
            self.read_string(|buf| unsafe { (self.uuid)(dev, buf.as_mut_ptr(), STR_LEN as c_int) });

        let temp = self.temp(dev, TEMPERATURE_GPU).map(|t| t as f64);
        let fan = self.fan(dev).map(|f| f as f32);

        // `nvmlUtilization_t` is two adjacent unsigned ints: gpu, then memory.
        let mut util = UtilRates { gpu: 0, memory: 0 };
        // SAFETY: NVML writes exactly one `nvmlUtilization_t` here.
        let util_ok = unsafe { (self.util)(dev, &mut util) } == SUCCESS;
        let usage = util_ok.then_some(util.gpu as f32);
        let engines = if util_ok {
            {
                vec![
                    ("graphics".to_string(), util.gpu as f32),
                    ("memory".to_string(), util.memory as f32),
                ]
            }
        } else {
            Default::default()
        };

        // `nvmlMemory_t` is total/free/used. Older drivers only fill the first
        // two, so `used` is derived rather than trusted.
        let mut mem = MemoryInfo {
            total: 0,
            free: 0,
            used: 0,
        };
        // SAFETY: valid out-pointer.
        let mem_ok = unsafe { (self.mem)(dev, &mut mem) } == SUCCESS;
        let mem_total = mem_ok.then_some(mem.total);
        let mem_used = mem_ok.then(|| mem.total.saturating_sub(mem.free));

        let mut mw: c_uint = 0;
        // SAFETY: valid out-pointer.
        let power_w = if unsafe { (self.power)(dev, &mut mw) } == SUCCESS {
            Some(mw as f64 / 1000.0)
        } else {
            None
        };

        let mut limit_mw: c_uint = 0;
        // SAFETY: valid out-pointer.
        let power_limit =
            if unsafe { (self.power_limit)(dev, &mut limit_mw) } == SUCCESS && limit_mw > 0 {
                Some(limit_mw as f64 / 1000.0)
            } else {
                None
            };

        // `NVML_CLOCK_GRAPHICS` == 0, `NVML_CLOCK_MEM` == 2.
        let clock_mhz = self.clock(dev, 0).map(|c| c as f64);
        let mem_clock_mhz = self.clock(dev, 2).map(|c| c as f64);

        // A leading "NVIDIA " prefix carries no information.
        let display_name = self.display_name(&name);

        GpuDevice {
            index: 0,
            name: display_name,
            vendor: "NVIDIA".into(),
            kind: "discrete".into(),
            usage_percent: usage,
            engines,
            memory_used_bytes: mem_used,
            memory_total_bytes: mem_total,
            temperature: temp,
            power_watts: power_w,
            power_limit_watts: power_limit,
            fan_percent: fan,
            clock_mhz,
            memory_clock_mhz: mem_clock_mhz,
            driver_version: self.driver.clone().or(uuid),
        }
    }

    /// Trims and de-brands a name NVML reported, with a usable fallback.
    fn display_name(&self, raw: &Option<String>) -> String {
        raw.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("NVIDIA GPU")
            .trim_start_matches("NVIDIA ")
            .to_string()
    }

    fn read_string(&self, f: impl Fn(&mut [i8]) -> c_int) -> Option<String> {
        let mut buf = vec![0i8; STR_LEN];
        if f(&mut buf) != SUCCESS {
            return None;
        }
        // SAFETY: NVML NUL-terminates its string output on success.
        let c: *const c_char = buf.as_ptr().cast();
        let s = unsafe { CStr::from_ptr(c) }
            .to_string_lossy()
            .trim()
            .to_string();
        (!s.is_empty()).then_some(s)
    }

    fn temp(&self, dev: NvmlDevice, sensor: c_int) -> Option<c_int> {
        let mut v: c_int = 0;
        // SAFETY: valid out-pointer.
        (unsafe { (self.temp)(dev, sensor, &mut v) } == SUCCESS && v > 0).then_some(v)
    }

    /// Not all cards expose a fan (p passively cooled ones do not), so
    /// absence is normal and not an error.
    fn fan(&self, dev: NvmlDevice) -> Option<c_int> {
        let mut v: c_int = 0;
        // SAFETY: valid out-pointer.
        (unsafe { (self.fan)(dev, &mut v) } == SUCCESS && v > 0).then_some(v)
    }

    /// `NVML_CLOCK_GRAPHICS` == 0, `NVML_CLOCK_MEM` == 2.
    fn clock(&self, dev: NvmlDevice, kind: c_int) -> Option<c_int> {
        let mut v: c_int = 0;
        // SAFETY: valid out-pointer.
        (unsafe { (self.clock)(dev, kind, &mut v) } == SUCCESS && v > 0).then_some(v)
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        // SAFETY: shutdown is idempotent per the NVML spec.
        unsafe { (self.shutdown)() };
    }
}
