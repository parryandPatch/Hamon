//! macOS-specific enrichment and sensors.

use crate::model::SystemInfo;
use crate::platform::parse_colon_map;
use std::process::Command;

/// The marketing model name (`MacBook Pro`) is not the same thing as the
/// board id (`MacBookPro18,1`). Apple keeps a lookup table in
/// `/System/Library/...` on older releases; on recent ones the best
/// user-facing string available without extra entitlements is whatever
/// `system_profiler` reports, so we start from that.
pub fn decorate(info: &mut SystemInfo) {
    if let Some(hw) = system_profiler_hardware() {
        if info.model.is_empty() {
            info.model = hw.get("Model Name").cloned().unwrap_or_default();
        }
        // Prefer `system_profiler`'s chip string; it is the marketing name.
        if info.cpu_brand.is_empty()
            && let Some(chip) = hw.get("Chip")
        {
            info.cpu_brand = chip.clone();
        }
        if let Some(cores) = hw.get("Total Number of Cores") {
            // "10 (8 Performance and 2 Efficiency)"
            // `hw.physicalcpu` is a plain count on Apple Silicon and on Intel.
            // On multi-socket Macs `hw.packages` looks like "2 x 8"; take the
            // first integer and ignore any "and"/"x" prose around it.
            if info.cores_physical == 0
                && let Some(n) = cores
                    .split_whitespace()
                    .find_map(|t| t.parse::<usize>().ok())
                    .filter(|n| *n > 0)
            {
                info.cores_physical = n;
            }
        }
    }
    if info.model.is_empty()
        && let Some(board) = board_id()
    {
        info.model = board;
    }
}

fn system_profiler_hardware() -> Option<std::collections::HashMap<String, String>> {
    let out = Command::new("/usr/sbin/system_profiler")
        .args(["SPHardwareDataType", "-detailLevel", "basic"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(parse_colon_map(&String::from_utf8_lossy(&out.stdout)))
}

/// `hw.model` from the boot device tree, e.g. `MacBookPro18,1`.
pub fn board_id() -> Option<String> {
    let out = Command::new("/usr/sbin/ioreg")
        .args(["-r", "-d", "1", "-w", "0", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(rest) = line.split_once("\"model\" = <") {
            let id = rest
                .1
                .trim_end_matches('>')
                .trim_matches('"')
                .trim_end_matches('\0')
                .to_string();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    None
}

/// Try the System Management Controller for real die temperatures.
///
/// `IOServiceOpen` on `AppleSMC` returns `kIOReturnNotPrivileged`
/// (`0xe00002c2`) for non-root processes on current macOS, so this is the
/// single place we have to care about elevation on macOS.
pub fn has_privileged_sensors() -> bool {
    !smc::SmcClient::open().is_some_and(|c| c.readable())
}

pub mod smc;

pub mod blockstorage;

pub mod ifaddrs;

pub use smc::smc_temperatures;
