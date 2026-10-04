//! Minimal System Management Controller reader.
//!
//! Only the read path is implemented: `kSMCGetKeyInfo` + `kSMCReadKey` over
//! the `AppleSMC` user client. That is the only way to get real thermal
//! numbers out of a Mac without a kernel extension.
//!
//! Requires the process to be running as root (or a helper installed with
//! elevated rights). `IOServiceOpen` returns `kIOReturnNotPrivileged`
//! (`0xe00002c2`) for unprivileged processes on current macOS, so everything
//! here degrades to "unavailable" rather than erroring.

use crate::model::{Sensor, SensorKind};

// ---------------------------------------------------------------------------
// Raw IOKit FFI. Declared by hand to avoid dragging the whole
// `core-foundation` + `objc` dependency tree into the binary.
// ---------------------------------------------------------------------------

#[cfg_attr(target_os = "macos", allow(dead_code))]
mod ffi {
    use std::os::raw::{c_char, c_void};

    pub type IoObject = u32;
    pub type IoService = u32;
    pub type IoConnect = u32;
    pub type KernReturn = i32;

    pub const KERN_SUCCESS: KernReturn = 0;

    unsafe extern "C" {
        pub fn IOServiceMatching(name: *const c_char) -> *mut c_void;
        pub fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> IoService;
        pub fn IOServiceOpen(
            service: IoService,
            owner: u32,
            r#type: u32,
            connect: *mut IoConnect,
        ) -> KernReturn;
        pub fn IOServiceClose(connect: IoConnect) -> KernReturn;
        pub fn IOConnectCallStructMethod(
            connect: IoConnect,
            selector: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: *mut u32,
        ) -> KernReturn;
        /// `mach_task_self()`, from libSystem.
        pub fn mach_task_self() -> u32;
    }
}

/// Mirrors `SMCKeyData_p`. Field layout matters: the struct is memcpy'd
/// straight into `IOConnectCallStructMethod`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SmcKeyDataKey {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

/// Mirrors `SMCKeyData_t`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SmcKeyData {
    data_size: u32,
    data_type: u32,
    data_attributes: u8,
}

/// Mirrors `SMCParamStruct`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SmcParamStruct {
    key: SmcKeyDataKey,
    vers: SmcKeyData,
    p_limit_data: SmcKeyData,
    key_info: SmcKeyData,
    result: u32,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

impl SmcParamStruct {
    fn with_key(code: &str) -> Self {
        let mut p = Self::default();
        let b = code.as_bytes();
        p.key.major = b.first().copied().unwrap_or(0);
        p.key.minor = b.get(1).copied().unwrap_or(0);
        p.key.build = b.get(2).copied().unwrap_or(0);
        p.key.reserved = b.get(3).copied().unwrap_or(0);
        p
    }
}

/// `kSMCReadKey` / `kSMCGetKeyInfo` selectors on the `AppleSMC` user client.
const K_SMC_READ_KEY: u32 = 5;
const K_SMC_GET_KEY_INFO: u32 = 9;

// ---------------------------------------------------------------------------
// SMC data types, as FourCC codes.
// ---------------------------------------------------------------------------

const fn fourcc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

const T_UI8: u32 = fourcc(b'u', b'i', b'8', b' ');
const T_UI16: u32 = fourcc(b'u', b'i', b'1', b'6');
const T_UI32: u32 = fourcc(b'u', b'i', b'3', b'2');
const T_SI8: u32 = fourcc(b's', b'i', b'8', b' ');
const T_SI16: u32 = fourcc(b's', b'i', b'1', b'6');
const T_SI32: u32 = fourcc(b's', b'i', b'3', b'2');
/// Signed, 7 integer bits + 8 fractional bits. The common temperature type.
const T_SP78: u32 = fourcc(b's', b'p', b'7', b'8');
/// Signed, 1 integer bit + 14 fractional bits.
const T_FP1F: u32 = fourcc(b'f', b'p', b'1', b'f');
/// Signed, 2 integer bits + 13 fractional bits.
const T_FPE2: u32 = fourcc(b'f', b'p', b'e', b'2');
const T_FLT: u32 = fourcc(b'f', b'l', b't', b' ');

/// An open connection to the SMC user client.
pub struct SmcClient {
    conn: ffi::IoConnect,
}

impl Drop for SmcClient {
    fn drop(&mut self) {
        unsafe { ffi::IOServiceClose(self.conn) };
    }
}

impl SmcClient {
    /// Opens `AppleSMC`. Returns `None` when the service is missing or the
    /// process lacks the right to talk to it.
    pub fn open() -> Option<Self> {
        unsafe {
            // `kIOMainPortDefault` is 0; passing the literal avoids linking
            // against the symbol (deprecated in the 12.0+ SDKs).
            let service =
                ffi::IOServiceGetMatchingService(0, ffi::IOServiceMatching(c"AppleSMC".as_ptr()));
            if service == 0 {
                return None;
            }
            let mut conn: ffi::IoConnect = 0;
            let kr = ffi::IOServiceOpen(service, ffi::mach_task_self(), 0, &mut conn);
            if kr != ffi::KERN_SUCCESS {
                return None;
            }
            Some(Self { conn })
        }
    }

    /// Cheap probe used to decide whether to advertise a "needs root" hint.
    pub fn readable(&self) -> bool {
        self.key_info("TC0P").is_some()
    }

    fn call(&self, selector: u32, code: &str) -> Option<SmcParamStruct> {
        unsafe {
            let input = SmcParamStruct::with_key(code);
            let mut output = SmcParamStruct::default();
            let mut out_size = std::mem::size_of::<SmcParamStruct>() as u32;
            let kr = ffi::IOConnectCallStructMethod(
                self.conn,
                selector,
                &input as *const _ as *const std::os::raw::c_void,
                std::mem::size_of::<SmcParamStruct>() as u32,
                &mut output as *mut _ as *mut std::os::raw::c_void,
                &mut out_size,
            );
            (kr == ffi::KERN_SUCCESS).then_some(output)
        }
    }

    fn key_info(&self, code: &str) -> Option<SmcDataInfo> {
        let out = self.call(K_SMC_GET_KEY_INFO, code)?;
        (out.key_info.data_size > 0).then_some(SmcDataInfo {
            data_size: out.key_info.data_size.min(32) as usize,
            data_type: out.key_info.data_type,
        })
    }

    /// Reads a key and decodes it to an `f64` in the key's natural unit.
    pub fn read_f64(&self, code: &str) -> Option<f64> {
        let info = self.key_info(code)?;
        let payload = self.call(K_SMC_READ_KEY, code)?;
        decode(&payload.bytes[..info.data_size], info.data_type)
    }
}

#[derive(Clone, Copy)]
struct SmcDataInfo {
    data_size: usize,
    data_type: u32,
}

/// Decodes an SMC payload according to its declared data type.
///
/// Every branch is length-guarded because a key can report a data size that
/// disagrees with its type on some firmware revisions.
fn decode(bytes: &[u8], data_type: u32) -> Option<f64> {
    match data_type {
        // Signed fixed-point: one byte of sign/magnitude plus fraction bits.
        T_SP78 => match bytes {
            [lo, hi, ..] => Some(i16::from_le_bytes([*lo, *hi]) as f64 / 256.0),
            _ => None,
        },
        T_FPE2 => match bytes {
            [lo, hi, ..] => Some(i16::from_le_bytes([*lo, *hi]) as f64 / 4.0),
            _ => None,
        },
        T_FP1F => match bytes {
            [lo, hi, ..] => Some(i16::from_le_bytes([*lo, *hi]) as f64 / 16_384.0),
            _ => None,
        },
        T_UI8 => bytes.first().map(|b| *b as f64),
        T_UI16 => match bytes {
            [lo, hi, ..] => Some(u16::from_le_bytes([*lo, *hi]) as f64),
            _ => None,
        },
        T_UI32 => match bytes {
            [a, b, c, d, ..] => Some(u32::from_le_bytes([*a, *b, *c, *d]) as f64),
            _ => None,
        },
        T_SI8 => bytes.first().map(|b| *b as i8 as f64),
        T_SI16 => match bytes {
            [lo, hi, ..] => Some(i16::from_le_bytes([*lo, *hi]) as f64),
            _ => None,
        },
        T_SI32 => match bytes {
            [a, b, c, d, ..] => Some(i32::from_le_bytes([*a, *b, *c, *d]) as f64),
            _ => None,
        },
        T_FLT => match bytes {
            [a, b, c, d, ..] => Some(f32::from_le_bytes([*a, *b, *c, *d]) as f64),
            _ => None,
        },
        _ => None,
    }
}

/// The temperature keys we care about, mapped to the label shown in the UI.
///
/// `TA0P`/`TA1P` are package sensors, `TC0P`/`TC0D`/`TCXC` are Intel CPU
/// proximity/die/fastest-core sensors, `TG0P` is the discrete GPU, and the
/// `TpXX` family are the per-cluster sensors Apple Silicon exposes. Keys that
/// don't exist on a given machine simply return `None`.
const TEMP_KEYS: &[(&str, &str, SensorKind)] = &[
    ("TA0P", "CPU Package", SensorKind::Cpu),
    ("TA1P", "CPU Package 2", SensorKind::Cpu),
    ("TC0P", "CPU Proximity", SensorKind::Cpu),
    ("TC0D", "CPU Die", SensorKind::Cpu),
    ("TC1C", "CPU Core 1", SensorKind::Cpu),
    ("TC2C", "CPU Core 2", SensorKind::Cpu),
    ("TCXC", "CPU Fastest Core", SensorKind::Cpu),
    ("TG0P", "GPU", SensorKind::Gpu),
    ("TG1P", "GPU Package", SensorKind::Gpu),
    ("TW0P", "NVMe Drive", SensorKind::Nvme),
    ("TM0P", "Memory", SensorKind::Memory),
    ("TB0T", "Battery", SensorKind::Battery),
];

/// Plausible range for a reading we decoded correctly. Anything outside is
/// almost certainly a key whose real type differs from our assumption.
const PLAUSIBLE_C: (f64, f64) = (-40.0, 150.0);

/// Reads every known temperature key over a single connection.
pub fn smc_temperatures() -> Vec<Sensor> {
    let Some(client) = SmcClient::open() else {
        return Vec::new();
    };
    read_temperatures(&client)
}

/// Reads all known temperature keys using an already-open connection.
///
/// The collector opens the connection once and reuses it, because
/// `IOServiceOpen` is not free and the availability never changes
/// mid-run.
pub fn read_temperatures(client: &SmcClient) -> Vec<Sensor> {
    let mut out = Vec::new();
    for (code, label, kind) in TEMP_KEYS {
        let Some(temp) = client.read_f64(code) else {
            continue;
        };
        if temp < PLAUSIBLE_C.0 || temp > PLAUSIBLE_C.1 {
            log::debug!("smc: implausible reading for {code}: {temp}");
            continue;
        }
        out.push(Sensor {
            label: (*label).to_string(),
            kind: *kind,
            temp_c: temp,
            high_c: None,
            critical_c: None,
            watts: None,
            millivolts: None,
            rpm: None,
            needs_privileges: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layout_matches_apple_headers() {
        assert_eq!(std::mem::size_of::<SmcKeyDataKey>(), 6);
        assert_eq!(std::mem::size_of::<SmcKeyData>(), 12);
        assert_eq!(std::mem::size_of::<SmcParamStruct>(), 88);
    }

    #[test]
    fn decodes_sp78_fixed_point() {
        // 0x2C00 = 11264; 11264 / 256 = 44.0
        assert_eq!(decode(&[0x00, 0x2c], T_SP78), Some(44.0));
        // Negative: 0xF000 -> -4096/256 = -16.0
        assert_eq!(decode(&[0x00, 0xf0], T_SP78), Some(-16.0));
    }

    #[test]
    fn decodes_fpe2_fixed_point() {
        // 0x000A = 10; 10 / 4 = 2.5
        assert_eq!(decode(&[0x0a, 0x00], T_FPE2), Some(2.5));
    }

    #[test]
    fn decodes_integers() {
        assert_eq!(decode(&[0x2a], T_UI8), Some(42.0));
        assert_eq!(decode(&[0x2a, 0x01], T_UI16), Some(298.0));
        assert_eq!(decode(&[0x01, 0x00, 0x00, 0x00], T_UI32), Some(1.0));
        assert_eq!(decode(&[0xff], T_SI8), Some(-1.0));
        assert_eq!(decode(&[0xff, 0xff], T_SI16), Some(-1.0));
    }

    #[test]
    fn decodes_float() {
        let v = decode(&f32::to_le_bytes(51.5), T_FLT).unwrap();
        assert!((v - 51.5).abs() < 1e-3, "got {v}");
    }

    #[test]
    fn unknown_types_and_short_buffers_return_none() {
        assert!(decode(&[0, 0], fourcc(b'z', b'z', b'z', b'z')).is_none());
        assert!(decode(&[], T_UI16).is_none());
        assert!(decode(&[1], T_UI32).is_none());
        assert!(decode(&[1], T_SP78).is_none());
        assert!(decode(&[1], T_FLT).is_none());
    }
}
