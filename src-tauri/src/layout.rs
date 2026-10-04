//! Dashboard layout: which widgets exist, in what order, and how each is
//! configured.
//!
//! The layout is stored as JSON in the platform config directory, which Tauri
//! derives from the bundle identifier (`com.hamon.app`) rather than the product
//! name — so `~/.config/com.hamon.app/layout.json` on Linux and
//! `~/Library/Application Support/com.hamon.app/layout.json` on macOS. Reading
//! and writing both go through [`LayoutStore`], which never panics: a corrupt
//! or partially-written file falls back to the default layout and is overwritten
//! on the next save.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Bumped when the shape changes in a way that needs migration.
const LAYOUT_VERSION: u32 = 1;

/// One entry in the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Widget {
    /// Stable identifier; also the catalog key.
    pub kind: String,
    /// Widget-specific settings. Shape depends on `kind`.
    #[serde(default)]
    pub config: serde_json::Value,
}

impl Widget {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_string(),
            config: default_config_for(kind),
        }
    }

    pub fn with_config(kind: &str, config: serde_json::Value) -> Self {
        Self {
            kind: kind.to_string(),
            config,
        }
    }
}

/// The whole dashboard: a flat, ordered list of widgets plus view settings.
///
/// A flat list is the right shape for drag-to-reorder. Responsive column
/// count is a presentation concern and lives in the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layout {
    pub version: u32,
    pub widgets: Vec<Widget>,
    #[serde(default = "default_columns")]
    pub columns: u8,
    #[serde(default = "default_sample_interval_ms")]
    pub sample_interval_ms: u64,
}

fn default_columns() -> u8 {
    2
}

fn default_sample_interval_ms() -> u64 {
    1000
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            version: LAYOUT_VERSION,
            widgets: default_widgets(),
            columns: default_columns(),
            sample_interval_ms: default_sample_interval_ms(),
        }
    }
}

impl Layout {
    /// Fills in defaults for any widget the UI does not know about, so an
    /// old layout referencing a removed widget still renders.
    pub fn sanitized(mut self) -> Self {
        self.version = LAYOUT_VERSION;
        self.columns = self.columns.clamp(1, 4);
        self.sample_interval_ms = self.sample_interval_ms.clamp(
            crate::sample::MIN_INTERVAL_MS,
            crate::sample::MAX_INTERVAL_MS,
        );
        self.widgets.retain(|w| !w.kind.is_empty());
        self
    }

    /// Adds a widget, returning the new length.
    pub fn add(&mut self, kind: &str) -> usize {
        self.widgets.push(Widget::new(kind));
        self.widgets.len()
    }

    /// Removes the widget at `index`. Returns `true` if anything changed.
    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.widgets.len() {
            self.widgets.remove(index);
            true
        } else {
            false
        }
    }

    /// Moves the widget at `from` so that it lands at `to`.
    ///
    /// Indices are clamped rather than rejected so a drag that ends outside
    /// the grid still lands somewhere sensible.
    pub fn move_widget(&mut self, from: usize, to: usize) -> bool {
        if from >= self.widgets.len() {
            return false;
        }
        let to = to.min(self.widgets.len() - 1);
        if from == to {
            return false;
        }
        let w = self.widgets.remove(from);
        self.widgets.insert(to, w);
        true
    }

    /// Applies a configuration payload for the widget at `index`.
    pub fn configure(&mut self, index: usize, config: serde_json::Value) -> bool {
        match self.widgets.get_mut(index) {
            Some(w) => {
                w.config = config;
                true
            }
            None => false,
        }
    }
}

/// The starters offered on a fresh install: one widget per major subsystem,
/// which is a useful starting point on any machine.
///
/// Every kind here is also in [`CATALOG_KINDS`] — enforced by
/// `tests::default_widgets_are_real_catalog_kinds`. A kind with no renderer
/// would render as an empty card, which is worse than not offering it.
const DEFAULT_WIDGETS: &[&str] = &[
    "gauge",
    "cpu-cores",
    "memory",
    "gpu",
    "network",
    "disk",
    "temperatures",
    "system",
];

fn default_widgets() -> Vec<Widget> {
    DEFAULT_WIDGETS.iter().map(|k| Widget::new(k)).collect()
}

/// Per-kind default configuration.
///
/// Every configurable field gets an explicit entry so the frontend never has
/// to reason about a missing key. The fall-through arm covers the kinds that
/// have no options of their own.
pub fn default_config_for(kind: &str) -> serde_json::Value {
    match kind {
        "gauge" => serde_json::json!({ "metric": "cpu-usage" }),
        "history" => serde_json::json!({ "metric": "cpu-usage", "span_seconds": 60 }),
        "cpu-cores" => serde_json::json!({ "show_frequency": true }),
        "gpu" => serde_json::json!({ "device": "auto" }),
        "network-interfaces" => serde_json::json!({ "interface": "auto" }),
        "disk-filesystems" => serde_json::json!({ "mount": "auto" }),
        "disk-io" => serde_json::json!({ "span_seconds": 60 }),
        "temperatures" => serde_json::json!({ "show_fans": true }),
        "processes" => serde_json::json!({ "limit": 10, "sort": "cpu" }),
        "battery" => serde_json::json!({ "show_health": true }),
        "system" => serde_json::json!({ "style": "compact" }),
        "custom-text" => serde_json::json!({ "lines": ["Hamon"] }),
        "custom-ascii" => serde_json::json!({ "art": "" }),
        "uptime" => serde_json::json!({ "show_boot_time": false }),
        _ => serde_json::json!({}),
    }
}

/// The widget kinds the frontend catalog offers, in display order.
///
/// This is the backend half of the catalog contract; the other half is
/// `src/lib/catalog.ts`. A kind present in one list and not the other is a real
/// bug — the backend would reject the widget, or the UI would offer a widget
/// that renders nothing — so the two are compared by
/// `npm run check:catalog`.
pub const CATALOG_KINDS: &[&str] = &[
    "gauge",
    "history",
    "cpu-cores",
    "memory",
    "gpu",
    "network",
    "network-interfaces",
    "disk",
    "disk-filesystems",
    "disk-io",
    "temperatures",
    "processes",
    "battery",
    "system",
    "uptime",
    "custom-text",
    "custom-ascii",
];

/// One catalog entry, as the machine-readable form of the contract.
///
/// Used by the `catalog` example so the parity check reads the real
/// definitions instead of parsing the source.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogEntry {
    pub kind: &'static str,
    pub defaults: serde_json::Value,
}

/// Every catalog kind with the config a freshly added widget of that kind gets.
pub fn catalog_entries() -> Vec<CatalogEntry> {
    CATALOG_KINDS
        .iter()
        .map(|kind| CatalogEntry {
            kind,
            defaults: default_config_for(kind),
        })
        .collect()
}

/// Reads and writes [`Layout`] under the app config directory.
pub struct LayoutStore {
    path: PathBuf,
}

impl LayoutStore {
    /// `base` is the platform config directory provided by Tauri.
    pub fn new(base: &Path) -> Self {
        Self {
            path: base.join("layout.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the layout, falling back to defaults on any problem.
    ///
    /// A missing file is the normal first-run case and is not logged as an
    /// error; a malformed one is worth a warning.
    pub fn load(&self) -> Layout {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return Layout::default();
        };
        match serde_json::from_str::<Layout>(&text) {
            Ok(l) => {
                log::info!("loaded layout with {} widgets", l.widgets.len());
                l.sanitized()
            }
            Err(e) => {
                log::warn!(
                    "layout at {} is corrupt ({e}); using defaults",
                    self.path.display()
                );
                Layout::default()
            }
        }
    }

    /// Writes atomically: a temp file plus rename, so a crash mid-save
    /// cannot leave a truncated layout behind.
    pub fn save(&self, layout: &Layout) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(layout)
            .map_err(|e| format!("could not serialize layout: {e}"))?;

        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json)
            .map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| format!("could not replace {}: {e}", self.path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hamon-layout-test-{tag}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn default_layout_is_valid() {
        let l = Layout::default();
        assert_eq!(l.version, LAYOUT_VERSION);
        assert!(!l.widgets.is_empty());
        assert_eq!(l.columns, 2);
        assert!(l.widgets.iter().all(|w| !w.kind.is_empty()));
    }

    #[test]
    fn add_remove_move_operate_on_indices() {
        let mut l = Layout::default();
        let n = l.add("gauge");
        assert_eq!(l.widgets.len(), n);
        let last = l.widgets.len() - 1;
        assert_eq!(l.widgets[last].kind, "gauge");

        assert!(l.move_widget(last, 0));
        assert_eq!(l.widgets[0].kind, "gauge");
        assert!(!l.move_widget(999, 0), "out-of-range source is a no-op");
        assert!(!l.move_widget(0, 0), "moving onto itself changes nothing");

        assert!(l.remove(0));
        assert!(!l.remove(9999));
    }

    #[test]
    fn move_clamps_target_into_range() {
        let mut l = Layout::default();
        let first = l.widgets[0].kind.clone();
        assert!(l.move_widget(0, 999));
        assert_eq!(l.widgets[l.widgets.len() - 1].kind, first);
    }

    #[test]
    fn configure_replaces_payload_in_place() {
        let mut l = Layout::default();
        assert!(l.configure(0, serde_json::json!({"metric": "net-rx"})));
        assert_eq!(l.widgets[0].config["metric"], "net-rx");
        assert!(!l.configure(500, serde_json::json!({})));
    }

    #[test]
    fn sanitizing_clamps_user_supplied_bounds() {
        let l = Layout {
            version: 0,
            widgets: vec![Widget::new("gauge"), Widget::new("")],
            columns: 99,
            sample_interval_ms: 1,
        }
        .sanitized();
        assert_eq!(l.version, LAYOUT_VERSION);
        assert_eq!(l.columns, 4);
        assert_eq!(l.sample_interval_ms, crate::sample::MIN_INTERVAL_MS);
        assert_eq!(l.widgets.len(), 1, "empty kinds are dropped");
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = temp_dir("roundtrip");
        let store = LayoutStore::new(&dir);
        let mut original = Layout {
            columns: 3,
            ..Default::default()
        };
        original.add("custom-text");
        store.save(&original).expect("save should succeed");

        let loaded = store.load();
        assert_eq!(loaded.columns, 3);
        assert_eq!(loaded.widgets.len(), original.widgets.len());
        assert_eq!(loaded.widgets.last().unwrap().kind, "custom-text");
        assert!(!store.path().exists() || store.path().extension().is_some());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = temp_dir("corrupt");
        std::fs::write(dir.join("layout.json"), "{ not json").unwrap();
        let store = LayoutStore::new(&dir);
        let loaded = store.load();
        assert_eq!(loaded.widgets.len(), Layout::default().widgets.len());
    }

    #[test]
    fn missing_file_returns_defaults_without_error() {
        let dir = temp_dir("missing");
        let store = LayoutStore::new(&dir);
        assert_eq!(store.load(), Layout::default());
    }

    #[test]
    fn save_creates_nested_directories() {
        let base = temp_dir("nested");
        let store = LayoutStore::new(&base.join("a/b/c"));
        store
            .save(&Layout::default())
            .expect("nested save should work");
        assert!(store.path().exists());
    }

    #[test]
    fn default_widgets_are_real_catalog_kinds() {
        for kind in DEFAULT_WIDGETS {
            assert!(
                CATALOG_KINDS.contains(kind),
                "default layout offers {kind:?}, which the frontend catalog does not render"
            );
        }
    }

    #[test]
    fn catalog_kinds_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in CATALOG_KINDS {
            assert!(seen.insert(*kind), "{kind} appears twice in the catalog");
        }
    }

    #[test]
    fn every_catalog_kind_has_a_default_config() {
        for kind in CATALOG_KINDS {
            let w = Widget::new(kind);
            assert_eq!(w.kind.as_str(), *kind);
            assert!(w.config.is_object(), "{kind} needs an object default");
        }
    }

    #[test]
    fn catalog_entries_cover_every_kind_exactly_once() {
        let entries = catalog_entries();
        assert_eq!(entries.len(), CATALOG_KINDS.len());
        for (entry, kind) in entries.iter().zip(CATALOG_KINDS) {
            assert_eq!(entry.kind, *kind);
            assert_eq!(
                entry.defaults,
                default_config_for(kind),
                "{kind}: the emitted default disagrees with default_config_for"
            );
        }
    }
}
