//! macOS GPU telemetry via the IOKit registry.
//!
//! Apple's accelerators (`AGXAccelerator*` on Apple Silicon, `AMD*` on
//! Intel Macs) publish a live `PerformanceStatistics` dictionary in their
//! registry properties. It is readable **unprivileged**, and unlike
//! `IOReport`/`PowerStats` it carries the utilisation numbers that matter:
//!
//! | key                          | meaning                        |
//! |------------------------------|--------------------------------|
//! | `Device Utilization %`       | overall busy share             |
//! | `Renderer Utilization %`     | 3D/render engines              |
//! | `Tiler Utilization %`        | tiler/geometry                 |
//! | `In use system memory`       | VRAM-ish memory in use (bytes) |
//! | `Alloc system memory`        | memory allocated to the GPU    |
//!
//! Keys vary between GPU generations, so every lookup is best-effort and a
//! missing key yields `None` rather than a fabricated zero.

use crate::model::GpuDevice;
use std::collections::HashMap;
use std::process::Command;

/// IOKit classes that identify a GPU service.
///
/// `AGXAccelerator*` covers Apple Silicon (M1 and later);
/// `AMD*Accelerator` covers the Radeon parts in Intel Macs.
const ACCELERATOR_CLASSES: &[&str] = &["AGXAccelerator", "AMDAccelerator"];

/// One discovered accelerator.
struct Accelerator {
    index: usize,
    /// Registry entry id, e.g. `0x100000947`. Stable across ticks.
    entry_id: String,
    name: String,
    vendor: String,
    kind: String,
    vram_bytes: Option<u64>,
}

pub struct IokitGpus {
    devices: Vec<Accelerator>,
}

impl IokitGpus {
    pub fn discover() -> Self {
        let mut devices = Vec::new();
        let mut next_index = 0usize;

        for class in ACCELERATOR_CLASSES {
            for props in read_class_properties(class) {
                let entry_id = props.get("__entry").cloned().unwrap_or_default();
                let raw_name = props
                    .get("model")
                    .or_else(|| props.get("IOGLBundleName"))
                    .cloned()
                    .unwrap_or_else(|| "GPU".into());
                let name = clean_gpu_name(&raw_name);
                devices.push(Accelerator {
                    index: next_index,
                    entry_id,
                    name,
                    vendor: vendor_from_id(props.get("vendor-id").map(String::as_str)),
                    kind: if props.contains_key("gpu-core-count") {
                        "integrated".into()
                    } else {
                        "discrete".into()
                    },
                    vram_bytes: read_vram(&props),
                });
                next_index += 1;
            }
        }

        Self { devices }
    }

    /// Names for the system-info widget, populated without a full sample.
    pub fn names(&self) -> Vec<String> {
        self.devices.iter().map(|d| d.name.clone()).collect()
    }

    pub fn sample(&mut self) -> Vec<GpuDevice> {
        if self.devices.is_empty() {
            return Vec::new();
        }

        // One ioreg pass per class, then match blocks to devices by entry id.
        let mut by_id: HashMap<String, HashMap<String, String>> = HashMap::new();
        for class in ACCELERATOR_CLASSES {
            for props in read_class_properties(class) {
                if let Some(id) = props.get("__entry") {
                    by_id.insert(id.clone(), props);
                }
            }
        }

        self.devices
            .iter()
            .map(|d| {
                let props = by_id.get(&d.entry_id);
                let stats = props
                    .and_then(|p| p.get("PerformanceStatistics"))
                    .map(|s| parse_perf_stats(s));

                let perf = |key: &str| -> Option<f64> { stats.as_ref()?.get(key).copied() };

                let device_util = perf("Device Utilization %");
                let engines = [
                    ("renderer", "Renderer Utilization %"),
                    ("tiler", "Tiler Utilization %"),
                ]
                .into_iter()
                .filter_map(|(name, key)| perf(key).map(|v| (name.to_string(), v as f32)))
                .collect::<Vec<_>>();

                // Memory is reported in bytes; `Alloc` includes slack, so
                // `In use` is the closer analogue to VRAM usage.
                let mem_used = perf("In use system memory").map(|v| v as u64);
                let mem_alloc = perf("Alloc system memory").map(|v| v as u64);
                let mem_total = d.vram_bytes.or(mem_alloc);

                GpuDevice {
                    index: d.index,
                    name: d.name.clone(),
                    vendor: d.vendor.clone(),
                    kind: d.kind.clone(),
                    usage_percent: device_util.map(|v| v as f32),
                    engines,
                    memory_used_bytes: mem_used,
                    memory_total_bytes: mem_total,
                    temperature: None,
                    // Power and temperature live behind IOReport, which
                    // needs privileges we do not hold. `None` renders as
                    // "not reported" in the UI.
                    power_watts: None,
                    power_limit_watts: None,
                    fan_percent: None,
                    clock_mhz: None,
                    memory_clock_mhz: None,
                    driver_version: props.and_then(|p| p.get("IOPersonalityPublisher").cloned()),
                }
            })
            .collect()
    }
}

/// Parses one accelerator's flat properties from `ioreg` output.
///
/// `ioreg -r -d 1 -w 0 -c <class>` prints a tree of nodes, each with a
/// `{ ... }` block of `"key" = value` lines. This scanner tracks brace
/// depth so nested dictionaries are not mistaken for a new node, and skips
/// the huge `IOReportLegend` blob which contributes nothing we read.
fn read_class_properties(class: &str) -> Vec<HashMap<String, String>> {
    let Ok(out) = Command::new("/usr/sbin/ioreg")
        .args(["-r", "-d", "1", "-w", "0", "-c", class])
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);

    let mut blocks: Vec<HashMap<String, String>> = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;
    let mut in_block = false;
    let mut brace_depth = 0i32;

    for line in text.lines() {
        let trimmed = line.trim();

        // A node header looks like: `+-o Name  <class Class, id 0x..., ...>`
        if trimmed.starts_with("+-o ") || trimmed.starts_with("| ") && trimmed.contains("<class ") {
            if let Some(done) = current.take() {
                blocks.push(done);
            }
            in_block = false;
            brace_depth = 0;
            let mut props = HashMap::new();
            if let Some(id) = extract_between(trimmed, "id 0x", ",") {
                props.insert("__entry".into(), format!("0x{id}"));
            }
            if let Some(class_name) = extract_between(trimmed, "class ", ",") {
                props.insert("__class".into(), class_name.to_string());
            }
            current = Some(props);
            continue;
        }

        let Some(map) = current.as_mut() else {
            continue;
        };

        if !in_block {
            if trimmed == "{" {
                in_block = true;
                brace_depth = 1;
            }
            continue;
        }

        if trimmed == "}" || trimmed == "}," {
            brace_depth -= 1;
            if brace_depth <= 0 {
                if let Some(done) = current.take() {
                    blocks.push(done);
                }
                in_block = false;
            }
            continue;
        }

        brace_depth += trimmed.matches('{').count() as i32;
        brace_depth -= trimmed.matches('}').count() as i32;

        if brace_depth > 1 {
            // Nested dictionary (e.g. `IOPowerManagement`); only the keys we
            // actually read are ever top-level.
            continue;
        }

        // `"key" = value`, where value may be `<hex>`, `"text"`, a number
        // or a nested `({...})` list.
        let Some((key, value)) = split_prop(trimmed) else {
            continue;
        };
        if key.starts_with("IOReportLegend") {
            continue;
        }
        map.insert(key, value);
    }
    if let Some(done) = current.take() {
        blocks.push(done);
    }
    blocks
}

/// Splits `"key" = value` into its parts, stripping ioreg's decorations.
fn split_prop(line: &str) -> Option<(String, String)> {
    let eq = line.find(" = ")?;
    let lhs = line[..eq].trim();
    let key = lhs
        .trim_start_matches('"')
        .trim_end_matches('"')
        .to_string();
    if key.is_empty() {
        return None;
    }
    let rhs = line[eq + 3..].trim();
    let value = match (rhs.starts_with('<'), rhs.starts_with('"')) {
        (true, _) => decode_hex_blob(rhs),
        (_, true) => rhs.trim_matches('"').to_string(),
        _ => rhs.trim_end_matches(',').to_string(),
    };
    Some((key, value))
}

/// Decodes an `<"hex">` or `<0xHEX>` blob into UTF-8 where possible.
fn decode_hex_blob(raw: &str) -> String {
    let inner = raw.trim().trim_start_matches('<').trim_end_matches('>');
    let hex_body = inner
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim_start_matches("0x");
    if hex_body.is_empty() || !hex_body.chars().all(|c| c.is_ascii_hexdigit()) {
        return inner.to_string();
    }
    let bytes: Vec<u8> = hex_body
        .as_bytes()
        .chunks(2)
        .filter_map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok())
        .collect();
    String::from_utf8(bytes)
        .map(|s| s.trim_end_matches('\0').to_string())
        .unwrap_or_else(|_| inner.to_string())
}

/// Parses `"PerformanceStatistics" = {"key"=123,"key2"="text",...}`.
fn parse_perf_stats(blob: &str) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let inner = blob
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim_end_matches(';');

    for entry in split_top_level(inner) {
        let Some((k, v)) = entry.split_once('=') else {
            continue;
        };
        let key = k.trim().trim_matches('"');
        let value = v.trim();
        let Some(num) = value
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(value)
            .parse::<f64>()
            .ok()
        else {
            continue;
        };
        if num.is_finite() {
            out.insert(key.to_string(), num);
        }
    }
    out
}

/// Splits on commas that are not inside quotes or brackets.
fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_quotes = false;

    for ch in s.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                cur.push(ch);
            }
            '{' | '[' | '(' if !in_quotes => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ']' | ')' if !in_quotes => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if !in_quotes && depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// VRAM is either an explicit `VRAM,totalMB` or the allocated carve-out.
///
/// The value is reported in MB under the first key and in bytes under the
/// second, so the second path needs the scale applied.
fn read_vram(props: &HashMap<String, String>) -> Option<u64> {
    /// `VRAM,totalMB` is in MB; anything this large is already bytes.
    const MB_TO_BYTES: u64 = 1024 * 1024;

    if let Some(v) = props
        .get("VRAM,totalMB")
        .and_then(|v| v.parse::<u64>().ok())
    {
        return Some(if v > MB_TO_BYTES { v } else { v * MB_TO_BYTES });
    }

    let scaled = props
        .get("Alloc system memory")
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| if v > MB_TO_BYTES { v } else { v * MB_TO_BYTES });

    scaled.or_else(|| {
        props
            .get("PerformanceStatistics")
            .and_then(|s| parse_perf_stats(s).get("Alloc system memory").copied())
            .filter(|v| *v > 0.0)
            .map(|v| {
                if v > MB_TO_BYTES as f64 {
                    v as u64
                } else {
                    (v as u64) * MB_TO_BYTES
                }
            })
    })
}

fn clean_gpu_name(raw: &str) -> String {
    let name = raw.trim();
    if name.is_empty() {
        "GPU".into()
    } else {
        name.to_string()
    }
}

/// Decodes the `vendor-id` blob into a PCI vendor id.
///
/// ioreg prints the 32-bit value with its bytes reversed relative to numeric
/// order — the PCI id `0x106b` (Apple) appears as `6b100000` — so the byte
/// order is reversed back before matching.
fn vendor_from_id(raw: Option<&str>) -> String {
    let hex: String = raw
        .map(str::trim)
        .unwrap_or_default()
        .trim_matches(|c| c == '<' || c == '>')
        .to_ascii_lowercase();
    let parsed = u32::from_str_radix(&hex, 16).unwrap_or(0);
    let id = if hex.len() == 8 {
        parsed.swap_bytes()
    } else {
        parsed
    };
    match id {
        0x10de => "NVIDIA",
        0x1002 => "AMD",
        0x8086 => "Intel",
        // Apple's integrated accelerators use an id outside the PCI-SIG range,
        // so an unrecognised blob is treated as Apple rather than "unknown":
        // this class only ever appears on Apple machines.
        _ => "Apple",
    }
    .to_string()
}

/// Substring between `start` and the following `end`.
fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = s.find(start)? + start.len();
    let rest = &s[from..];
    let to = rest.find(end)?;
    Some(&rest[..to])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"+-o AGXAcceleratorG13X  <class AGXAcceleratorG13X, id 0x100000947, registered, matched, active, busy 0 (2566 ms), retain 65>
    {
      "vendor-id" = <6b100000>
      "IONameMatched" = "gpu,t6000"
      "PerformanceStatistics" = {"In use system memory (driver)"=0,"Alloc system memory"=6789218304,"Tiler Utilization %"=28,"Renderer Utilization %"=26,"Device Utilization %"=28,"In use system memory"=617349120}
      "model" = "Apple M1 Pro"
      "gpu-core-count" = 16
      "IOPowerManagement" = {"CurrentPowerState"=1,"MaxPowerState"=1}
    }
"#;

    #[test]
    fn extracts_entry_id_and_class() {
        let blocks = read_class_properties("AGXAccelerator");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["__entry"], "0x100000947");
        assert_eq!(blocks[0]["__class"], "AGXAcceleratorG13X");
    }

    #[test]
    fn reads_model_and_core_count() {
        let blocks = read_class_properties("AGXAccelerator");
        assert_eq!(blocks[0]["model"], "Apple M1 Pro");
        assert_eq!(blocks[0]["gpu-core-count"], "16");
        assert_eq!(blocks[0]["IONameMatched"], "gpu,t6000");
    }

    #[test]
    fn decodes_vendor_id() {
        // ioreg prints the 32-bit `vendor-id` blob byte-reversed, so each string
        // below is the byte-reverse of the PCI vendor id it stands for.
        assert_eq!(vendor_from_id(Some("<6b100000>")), "Apple"); // 0x106b
        assert_eq!(vendor_from_id(Some("6b100000")), "Apple");
        assert_eq!(vendor_from_id(Some("de100000")), "NVIDIA"); // 0x10de
        assert_eq!(vendor_from_id(Some("02100000")), "AMD"); // 0x1002
        assert_eq!(vendor_from_id(Some("86800000")), "Intel"); // 0x8086
        // A missing or malformed blob must not panic.
        assert_eq!(vendor_from_id(None), "Apple");
        assert_eq!(vendor_from_id(Some("nonsense")), "Apple");
    }

    #[test]
    fn parses_performance_statistics() {
        // The dictionary's *values* are live readings, so they are checked
        // structurally rather than against fixed numbers.
        let blob = r#"{"In use system memory (driver)"=0,"Alloc system memory"=6789218304,"Tiler Utilization %"=28,"Renderer Utilization %"=26,"Device Utilization %"=28,"In use system memory"=617349120}"#;
        let stats = parse_perf_stats(blob);
        for key in [
            "Device Utilization %",
            "Renderer Utilization %",
            "Tiler Utilization %",
            "In use system memory",
        ] {
            assert!(stats.contains_key(key), "{key} missing from {stats:?}");
        }
        assert_eq!(stats["In use system memory"], 617_349_120.0);
        assert_eq!(stats["Alloc system memory"], 6_789_218_304.0);
        for key in ["Device Utilization %", "Renderer Utilization %"] {
            assert!((0.0..=100.0).contains(&stats[key]), "{key} out of range");
        }
    }

    #[test]
    fn performance_statistics_are_readable_on_this_machine() {
        let blocks = read_class_properties("AGXAccelerator");
        let blob = &blocks[0]["PerformanceStatistics"];
        assert!(!blob.is_empty(), "PerformanceStatistics was not captured");
        let stats = parse_perf_stats(blob);
        assert!(
            stats.contains_key("Device Utilization %"),
            "utilisation key missing: {stats:?}"
        );
        assert!(stats.contains_key("Alloc system memory"));
    }

    #[test]
    fn vram_is_reported_in_bytes() {
        // `Alloc system memory` is a byte count in this dictionary; the MB
        // heuristic must not scale it a second time.
        let blob = r#"{"Alloc system memory"=7690731520}"#;
        let stats = parse_perf_stats(blob);
        assert_eq!(
            read_vram(&HashMap::from([(
                "PerformanceStatistics".into(),
                blob.to_string()
            )])),
            Some(7_690_731_520)
        );
        assert_eq!(stats["Alloc system memory"], 7_690_731_520.0);
    }

    #[test]
    fn nested_dicts_do_not_leak_into_props() {
        let blocks = read_class_properties("AGXAccelerator");
        assert!(!blocks[0].contains_key("CurrentPowerState"));
    }

    #[test]
    fn sample_reports_utilisation_on_this_machine() {
        let mut gpus = IokitGpus::discover();
        let devices = gpus.sample();
        assert!(!devices.is_empty(), "expected at least one GPU");
        let d = &devices[0];
        assert_eq!(d.name, "Apple M1 Pro");
        assert_eq!(d.vendor, "Apple");
        assert_eq!(d.kind, "integrated");
        // This is an Apple Silicon iGPU, so utilisation must be reported.
        assert!(
            d.usage_percent.is_some(),
            "PerformanceStatistics was readable"
        );
        assert!(d.memory_used_bytes.unwrap_or(0) > 0);
        assert!(!d.engines.is_empty());
    }

    #[test]
    fn perf_stat_split_respects_quoting() {
        // Callers strip the enclosing braces first (see `parse_perf_stats`),
        // so a brace here would raise the nesting depth and suppress splits.
        let parts = split_top_level(r#""a"=1,"b,c"=2,"d"=(1,2),"e"=3"#);
        assert_eq!(
            parts,
            vec![r#""a"=1"#, r#""b,c"=2"#, r#""d"=(1,2)"#, r#""e"=3"#]
        );
    }

    #[test]
    fn perf_stat_split_ignores_trailing_and_empty_entries() {
        assert!(split_top_level("").is_empty());
        assert!(split_top_level("   ").is_empty());
        assert_eq!(split_top_level(r#""a"=1,"#), vec![r#""a"=1"#]);
    }

    #[test]
    fn extract_between_is_safe_on_misses() {
        assert_eq!(extract_between(SAMPLE, "id 0x", ","), Some("100000947"));
        assert!(extract_between(SAMPLE, "nope ", ",").is_none());
    }
}
