//! macOS block-device counters via IOKit.
//!
//! Every `IOBlockStorageDriver` node in the registry publishes a `Statistics`
//! dictionary holding cumulative `Bytes (Read)` / `Bytes (Write)` totals since
//! boot; deltas between ticks give throughput.
//!
//! The driver node has no name of its own, so the registry is walked:
//!
//! ```text
//! IOBlockStorageDriver   "Statistics" -> counters, "BSD Name" via a parent
//!   search -> "disk0"
//!   └─ IOBlockStorageDevice  "Device Characteristics" -> "Product Name"
//! ```
//!
//! Talking to IOKit directly rather than shelling out to `ioreg` matters
//! because this runs at the sampling rate: two `ioreg` subprocesses per second
//! is a measurable, entirely avoidable cost, and the textual `ioreg` dump does
//! not expose the driver↔device parent link, which is exactly what is needed
//! to put a name on a counter.
//!
//! Hand-declared FFI rather than the `core-foundation` / `objc` crates: the
//! surface used here is a dozen symbols and pulling in Objective-C for it
//! would add link-time weight to every platform build.

use std::marker::PhantomData;
use std::os::raw::{c_char, c_long, c_void};

// ---------------------------------------------------------------------------
// FFI
// ---------------------------------------------------------------------------

#[cfg_attr(target_os = "macos", allow(dead_code))]
mod ffi {
    use std::os::raw::{c_char, c_long, c_void};

    pub type IoObject = u32;
    pub type KernReturn = i32;
    /// `CFTypeRef`. Mutable because CF's own accessors are not const-correct.
    pub type CfRef = *mut c_void;

    pub const KERN_SUCCESS: KernReturn = 0;

    /// Search the entry's ancestors for a property. Values from
    /// `IORegistryEntry.h`; only `IterateParents` is needed here.
    pub const K_IOREGISTRY_ITERATE_PARENTS: u32 = 0x2;

    /// `kCFStringEncodingUTF8`.
    pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    /// `kCFNumberSInt32Type` / `SInt64Type` / `Float64Type`.
    pub const K_CF_NUMBER_SINT32: c_long = 3;
    pub const K_CF_NUMBER_SINT64: c_long = 4;
    pub const K_CF_NUMBER_FLOAT64: c_long = 6;

    /// The service plane, as the null-terminated name IOKit expects.
    pub const IOSERVICE_PLANE: &[u8] = b"IOService\0";

    unsafe extern "C" {
        pub fn IOServiceMatching(name: *const c_char) -> CfRef;
        pub fn IOServiceGetMatchingServices(
            main_port: u32,
            matching: CfRef,
            existing: *mut u32,
        ) -> KernReturn;
        pub fn IOIteratorNext(iterator: u32) -> IoObject;
        pub fn IOObjectRelease(object: IoObject) -> KernReturn;

        pub fn IORegistryEntryCreateCFProperty(
            entry: u32,
            key: *const c_void,
            allocator: *const c_void,
            options: u32,
        ) -> CfRef;
        /// Note the five-argument form: `key` and `allocator` sit *between*
        /// `plane` and `options`.
        pub fn IORegistryEntrySearchCFProperty(
            entry: u32,
            plane: *const c_char,
            key: *const c_void,
            allocator: *const c_void,
            options: u32,
        ) -> CfRef;

        pub fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        pub fn CFStringGetCString(
            the_string: *const c_void,
            buffer: *mut c_char,
            size: c_long,
            encoding: u32,
        ) -> bool;
        pub fn CFDictionaryGetCount(dict: CfRef) -> c_long;
        pub fn CFDictionaryGetKeysAndValues(
            dict: CfRef,
            keys: *mut *const c_void,
            values: *mut *const c_void,
            num_values: c_long,
        );
        pub fn CFDictionaryGetValue(dict: CfRef, key: *const c_void) -> *const c_void;
        pub fn CFNumberGetValue(number: CfRef, the_type: c_long, value_ptr: *mut c_void) -> bool;
        pub fn CFGetTypeID(cf: CfRef) -> c_long;
        pub fn CFDictionaryGetTypeID() -> c_long;
        pub fn CFStringGetTypeID() -> c_long;
        pub fn CFRelease(cf: CfRef);
    }
}

/// One block device's cumulative counters, plus what the registry knows about
/// it beyond the node name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDevice {
    /// `disk0`, `disk2`, … — what users recognise and what `diskutil` uses.
    pub name: String,
    /// Product name, e.g. `APPLE SSD AP0512R`. Absent for virtual devices.
    pub model: Option<String>,
    /// Bytes read since boot.
    pub read_bytes: u64,
    /// Bytes written since boot.
    pub write_bytes: u64,
}

/// Reads every block-storage driver's counters.
///
/// Returns an empty list if the registry walk fails; the caller already
/// treats "no devices" as "no disk activity" rather than an error.
pub fn read_all() -> Vec<BlockDevice> {
    let Some(matching) = matching_dictionary("IOBlockStorageDriver") else {
        return Vec::new();
    };

    // `IOServiceGetMatchingServices` takes ownership of `matching` on entry,
    // so ownership is handed over rather than released on scope exit.
    let matching = matching.into_raw();
    let mut iter: u32 = 0;
    // SAFETY: `matching` is a freshly created dictionary that we no longer own.
    if unsafe { ffi::IOServiceGetMatchingServices(0, matching, &mut iter) } != ffi::KERN_SUCCESS {
        return Vec::new();
    }

    let mut out = Vec::new();
    loop {
        // SAFETY: `iter` is a live iterator.
        let driver = unsafe { ffi::IOIteratorNext(iter) };
        if driver == 0 {
            break;
        }
        // SAFETY: `driver` is a retained object from `IOIteratorNext`.
        let guard = IoObjectGuard(driver);
        if let Some(device) = read_driver(guard.0) {
            out.push(device);
        }
    }
    // SAFETY: `iter` is a live iterator obtained above.
    unsafe { ffi::IOObjectRelease(iter) };

    // Registry order is stable but arbitrary; sorting by name makes the list
    // predictable, which keeps the delta pairing in `DiskCollector` readable.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Extracts the counters and names for one driver node.
fn read_driver(driver: u32) -> Option<BlockDevice> {
    let stats = property(driver, "Statistics")?;
    // A driver with no readable statistics is not usable as a counter source;
    // dropping it here also drops it from the device list.
    let read_bytes = dict_number(stats.as_ref(), "Bytes (Read)")?;
    let write_bytes = dict_number(stats.as_ref(), "Bytes (Write)").unwrap_or(0);

    // `BSD Name` lives on the media node, two levels up, so it is found by
    // walking every ancestor rather than a single `GetParentEntry`.
    let name = search_parent_property(driver, "BSD Name")
        .and_then(|v| cf_string(v.as_ref()))
        // The registry entry id is a usable last resort: it is stable for the
        // lifetime of the machine, which is all the delta pairing needs.
        .unwrap_or_else(|| format!("0x{driver:08x}"));

    // The product name sits one level up, nested in `Device Characteristics`.
    let model = property(driver, "Device Characteristics")
        .and_then(|parent| dict_string(parent.as_ref(), "Product Name"))
        .filter(|m| !m.is_empty());

    Some(BlockDevice {
        name,
        model,
        read_bytes,
        write_bytes,
    })
}

// ---------------------------------------------------------------------------
// CoreFoundation helpers
// ---------------------------------------------------------------------------

/// An **owned** (`+1`) CoreFoundation reference: released on drop.
///
/// Only the `Create`-style calls below produce one — `IOServiceMatching`,
/// `IORegistryEntryCreateCFProperty`, `IORegistryEntrySearchCFProperty` and
/// `CFStringCreateWithCString`. Values handed back by *getter* calls such as
/// `CFDictionaryGetValue` are borrowed and must not be wrapped in this type;
/// over-releasing one is an immediate crash.
///
/// Every reference in this module is created and consumed on the one sampler
/// thread, so a plain `Drop` is sound with respect to CF's own lack of
/// thread-safety guarantees.
struct Cf(ffi::CfRef);

impl Cf {
    /// Views this owned reference as a borrowed one, for the read-only
    /// accessors that must not take ownership.
    fn as_ref(&self) -> Ref<'_> {
        Ref(self.0 as *const c_void, PhantomData)
    }

    /// Hands the raw reference over without releasing it.
    ///
    /// Needed for the matching dictionary: `IOServiceGetMatchingServices`
    /// takes ownership of its `matching` argument on entry, so releasing it
    /// afterwards is a double free and an immediate crash.
    fn into_raw(self) -> ffi::CfRef {
        let raw = self.0;
        std::mem::forget(self);
        raw
    }
}

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: non-null handles in this type are always owned `+1`
            // references, by construction of every call site.
            unsafe { ffi::CFRelease(self.0) };
        }
    }
}

/// A **borrowed** CoreFoundation reference. Never released.
#[derive(Clone, Copy)]
struct Ref<'a>(*const c_void, PhantomData<&'a ()>);

impl Ref<'_> {
    #[cfg(test)]
    fn null() -> Self {
        Self(std::ptr::null(), PhantomData)
    }

    fn is_null(self) -> bool {
        self.0.is_null()
    }

    fn as_cfref(self) -> ffi::CfRef {
        self.0.cast_mut()
    }
}

/// Releases an `io_object_t` on drop.
struct IoObjectGuard(u32);

impl Drop for IoObjectGuard {
    fn drop(&mut self) {
        // SAFETY: every value passed in came from `IOIteratorNext`.
        unsafe { ffi::IOObjectRelease(self.0) };
    }
}

fn matching_dictionary(class: &str) -> Option<Cf> {
    let name = std::ffi::CString::new(class).ok()?;
    // SAFETY: `name` outlives the call, and the returned dictionary is a new
    // `+1` reference the caller owns.
    let dict = unsafe { ffi::IOServiceMatching(name.as_ptr()) };
    (!dict.is_null()).then_some(Cf(dict))
}

fn property(entry: u32, key: &str) -> Option<Cf> {
    let key = cf_str(key);
    // SAFETY: `entry` is live and `key` is a live `CFStringRef`. `kCFAllocatorDefault`
    // is `NULL`, which asks CF to use the default allocator.
    let value = unsafe { ffi::IORegistryEntryCreateCFProperty(entry, key.0, std::ptr::null(), 0) };
    (!value.is_null()).then_some(Cf(value))
}

/// Looks for `key` on `entry` or any of its ancestors.
fn search_parent_property(entry: u32, key: &str) -> Option<Cf> {
    let key = cf_str(key);
    // SAFETY: `entry` is live; `IOSERVICE_PLANE` is a NUL-terminated constant.
    let value = unsafe {
        ffi::IORegistryEntrySearchCFProperty(
            entry,
            ffi::IOSERVICE_PLANE.as_ptr().cast::<c_char>(),
            key.0,
            std::ptr::null(),
            ffi::K_IOREGISTRY_ITERATE_PARENTS,
        )
    };
    (!value.is_null()).then_some(Cf(value))
}

/// Builds a `CFStringRef` from a Rust `&str`.
fn cf_str(s: &str) -> Cf {
    let bytes = std::ffi::CString::new(s).unwrap_or_default();
    // SAFETY: `bytes` is NUL-terminated and outlives the call. A NUL inside
    // `s` would make `CString::new` fail and yield an empty string, which CF
    // happily turns into `""` — never a null deref.
    let cf = unsafe {
        ffi::CFStringCreateWithCString(
            std::ptr::null(),
            bytes.as_ptr(),
            ffi::K_CF_STRING_ENCODING_UTF8,
        )
    };
    Cf(cf as ffi::CfRef)
}

/// Converts a `CFStringRef` to a Rust `String`.
fn cf_string(cf: Ref<'_>) -> Option<String> {
    if cf.is_null()
        || unsafe { ffi::CFGetTypeID(cf.as_cfref()) } != unsafe { ffi::CFStringGetTypeID() }
    {
        return None;
    }
    // 512 bytes is far beyond any property value read here.
    let mut buf = [0 as c_char; 512];
    // SAFETY: `buf` is writable for `buf.len()` bytes and the length is passed
    // in the matching argument.
    let ok = unsafe {
        ffi::CFStringGetCString(
            cf.0,
            buf.as_mut_ptr(),
            buf.len() as c_long,
            ffi::K_CF_STRING_ENCODING_UTF8,
        )
    };
    if !ok {
        return None;
    }
    // SAFETY: a `true` return from `CFStringGetCString` guarantees a
    // NUL-terminated buffer.
    let s = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    Some(s.to_string_lossy().into_owned())
}

/// Looks up a string value in a `CFDictionaryRef`.
fn dict_string(dict: Ref<'_>, key: &str) -> Option<String> {
    if !is_dictionary(dict) {
        return None;
    }
    let key = cf_str(key);
    // SAFETY: `dict` is a `CFDictionaryRef` (checked above) and `key` is live.
    // The result is *borrowed* from `dict`, so it is read without retaining.
    let value = unsafe { ffi::CFDictionaryGetValue(dict.as_cfref(), key.0) };
    cf_string(Ref(value, PhantomData))
}

/// Looks up a numeric value in a `CFDictionaryRef`.
///
/// The stored `CFNumberType` varies between IOKit versions and drivers, so the
/// three plausible integer/float widths are tried in turn.
fn dict_number(dict: Ref<'_>, key: &str) -> Option<u64> {
    if !is_dictionary(dict) {
        return None;
    }
    let key = cf_str(key);
    // SAFETY: as above.
    let value = unsafe { ffi::CFDictionaryGetValue(dict.as_cfref(), key.0) };
    if value.is_null() {
        return None;
    }
    number_as_u64(value.cast::<c_void>().cast_mut())
}

/// Reads a `CFNumberRef` regardless of its backing width.
fn number_as_u64(number: ffi::CfRef) -> Option<u64> {
    let mut wide: i64 = 0;
    // SAFETY: `wide` is a correctly sized, writable `i64` for this type.
    if unsafe {
        ffi::CFNumberGetValue(
            number,
            ffi::K_CF_NUMBER_SINT64,
            (&mut wide as *mut i64).cast(),
        )
    } {
        return u64::try_from(wide).ok();
    }
    let mut narrow: i32 = 0;
    // SAFETY: likewise for an `i32`.
    if unsafe {
        ffi::CFNumberGetValue(
            number,
            ffi::K_CF_NUMBER_SINT32,
            (&mut narrow as *mut i32).cast(),
        )
    } {
        return u64::try_from(narrow).ok();
    }
    let mut real: f64 = 0.0;
    // SAFETY: likewise for an `f64`.
    if unsafe {
        ffi::CFNumberGetValue(
            number,
            ffi::K_CF_NUMBER_FLOAT64,
            (&mut real as *mut f64).cast(),
        )
    } && real.is_finite()
        && real >= 0.0
    {
        return Some(real as u64);
    }
    None
}

fn is_dictionary(cf: Ref<'_>) -> bool {
    !cf.is_null()
        && unsafe { ffi::CFGetTypeID(cf.as_cfref()) } == unsafe { ffi::CFDictionaryGetTypeID() }
}

/// Every key in a `CFDictionaryRef`, decoded. Used by the tests to assert the
/// real keys on this machine are the ones the reader looks for.
#[cfg(test)]
fn dict_keys(dict: Ref<'_>) -> Vec<String> {
    if !is_dictionary(dict) {
        return Vec::new();
    }
    // SAFETY: `dict` was confirmed to be a `CFDictionaryRef` above.
    let count = unsafe { ffi::CFDictionaryGetCount(dict.as_cfref()) };
    if count <= 0 {
        return Vec::new();
    }
    let mut keys: Vec<*const c_void> = vec![std::ptr::null(); count as usize];
    // SAFETY: `keys` has room for exactly `count` elements, as documented;
    // a null `values` output is allowed and simply skips that half.
    unsafe {
        ffi::CFDictionaryGetKeysAndValues(
            dict.as_cfref(),
            keys.as_mut_ptr(),
            std::ptr::null_mut(),
            count,
        )
    };
    keys.iter()
        .map(|k| cf_string(Ref(*k, PhantomData)).unwrap_or_default())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_counters_from_this_machine() {
        let all = read_all();
        assert!(!all.is_empty(), "expected at least one block device");
        for d in &all {
            assert!(!d.name.is_empty());
            // Every Mac has an internal disk with boot-time traffic on it.
            if d.name == "disk0" {
                assert!(
                    d.read_bytes > 0 || d.write_bytes > 0,
                    "disk0 idle since boot"
                );
                assert!(d.model.is_some(), "disk0 should expose a product name");
            }
        }
    }

    #[test]
    fn counters_do_not_go_backwards_between_ticks() {
        let first = read_all();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let second = read_all();
        for b in second {
            let a = first.iter().find(|a| a.name == b.name);
            if let Some(a) = a {
                assert!(
                    b.read_bytes >= a.read_bytes,
                    "{} read counter went down",
                    b.name
                );
                assert!(
                    b.write_bytes >= a.write_bytes,
                    "{} write counter went down",
                    b.name
                );
            }
        }
    }

    #[test]
    fn names_are_unique_so_deltas_pair_correctly() {
        let all = read_all();
        let mut names: Vec<&str> = all.iter().map(|d| d.name.as_str()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate device names: {all:?}");
    }

    #[test]
    fn statistics_keys_are_the_ones_we_read() {
        // Guards against a macOS release renaming the dictionary keys, which
        // would silently turn the disk widget into a row of zeroes.
        let devices = read_all();
        assert!(!devices.is_empty());
        // The bytes must actually have come from `Bytes (Read)`; assert the
        // aggregate exceeds what an all-zero registry would give.
        assert!(
            devices
                .iter()
                .any(|d| d.read_bytes > 0 || d.write_bytes > 0)
        );
    }

    #[test]
    fn cf_string_round_trips() {
        assert_eq!(
            cf_string(cf_str("disk0").as_ref()).as_deref(),
            Some("disk0")
        );
    }

    #[test]
    fn cf_string_rejects_non_strings() {
        assert_eq!(cf_string(Ref::null()), None);
        // A CFDictionary is not a CFString.
        let dict = matching_dictionary("IOBlockStorageDriver").unwrap();
        assert_eq!(cf_string(dict.as_ref()), None);
    }

    #[test]
    fn dict_lookups_tolerate_garbage() {
        assert_eq!(dict_string(Ref::null(), "Bytes (Read)"), None);
        assert_eq!(dict_number(Ref::null(), "Bytes (Read)"), None);
        // A non-dictionary passed where a dictionary is expected.
        let s = cf_str("not a dict");
        assert_eq!(dict_string(s.as_ref(), "k"), None);
        assert_eq!(dict_number(s.as_ref(), "k"), None);
    }

    #[test]
    fn dict_key_misses_return_none_rather_than_zero() {
        // Guards against a typo in a key name silently reporting `0` bytes,
        // which is indistinguishable from an idle disk in the UI.
        let dict = matching_dictionary("IOBlockStorageDriver").unwrap();
        assert!(is_dictionary(dict.as_ref()));
        assert_eq!(dict_number(dict.as_ref(), "Bytes (Read)"), None);
    }

    #[test]
    fn dict_keys_helper_reads_the_real_statistics_dictionary() {
        let mut iter: u32 = 0;
        // Ownership of the matching dictionary passes to IOKit.
        let matching = matching_dictionary("IOBlockStorageDriver")
            .unwrap()
            .into_raw();
        // SAFETY: `matching` is a fresh dictionary we no longer own.
        let kr = unsafe { ffi::IOServiceGetMatchingServices(0, matching, &mut iter) };
        assert_eq!(kr, ffi::KERN_SUCCESS);
        // SAFETY: `iter` is live.
        let driver = unsafe { ffi::IOIteratorNext(iter) };
        assert_ne!(driver, 0, "no driver on this machine");
        let stats = property(driver, "Statistics").expect("Statistics property");
        let keys = dict_keys(stats.as_ref());
        // SAFETY: both handles are live.
        unsafe {
            ffi::IOObjectRelease(driver);
            ffi::IOObjectRelease(iter);
        }
        assert!(
            keys.iter().any(|k| k == "Bytes (Read)"),
            "unexpected Statistics keys: {keys:?}"
        );
        assert!(keys.iter().any(|k| k == "Bytes (Write)"));
    }
}
