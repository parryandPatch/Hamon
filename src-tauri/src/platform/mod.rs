//! Platform-specific helpers.
//!
//! Each submodule is gated at the *module* level so an unsupported target
//! never even compiles the code, keeping macOS builds free of dead Linux
//! string parsing (and vice versa).

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub mod unsupported {
    use crate::model::SystemInfo;

    pub fn decorate(_info: &mut SystemInfo) {}
    pub fn has_privileged_sensors() -> bool {
        false
    }
}

/// Thin `cfg` facade so callers do not need `cfg` blocks of their own.
pub fn decorate_system(info: &mut crate::model::SystemInfo) {
    #[cfg(target_os = "macos")]
    macos::decorate(info);
    #[cfg(target_os = "linux")]
    linux::decorate(info);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    unsupported::decorate(info);
}

/// True when some collector needs administrator/root rights on this machine.
pub fn has_privileged_sensors() -> bool {
    #[cfg(target_os = "macos")]
    return macos::has_privileged_sensors();
    #[cfg(target_os = "linux")]
    return linux::has_privileged_sensors();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    unsupported::has_privileged_sensors()
}

/// Read `key=value` pairs out of a `Key: value` / `a=b` style text blob.
/// Used for `os-release`, `lsblk`-ish output and similar.
pub fn parse_kv_map(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(
                k.trim().trim_matches('"').to_string(),
                v.trim().trim_matches('"').to_string(),
            );
        }
    }
    map
}

/// Parse `Key: value` pairs (macOS `system_profiler`, `sysctl`, ...).
pub fn parse_colon_map(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim_end();
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

/// Best-effort human byte formatting used in logs and error messages.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_os_release() {
        let text = r#"
NAME="NixOS"
VERSION="25.05 (Evaristo)"
ID=nixos
PRETTY_NAME="NixOS 25.05 (Evaristo)"
"#;
        let map = parse_kv_map(text);
        assert_eq!(map.get("ID").map(String::as_str), Some("nixos"));
        assert_eq!(
            map.get("PRETTY_NAME").map(String::as_str),
            Some("NixOS 25.05 (Evaristo)")
        );
    }

    #[test]
    fn parses_colon_map_ignoring_nested_lines() {
        let text = "  Chip: Apple M1 Pro\n  Memory: 32 GB\n\nHardware:\n";
        let map = parse_colon_map(text);
        assert_eq!(map.get("Chip").map(String::as_str), Some("Apple M1 Pro"));
        assert_eq!(map.get("Memory").map(String::as_str), Some("32 GB"));
    }

    #[test]
    fn human_bytes_scales() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
    }
}
