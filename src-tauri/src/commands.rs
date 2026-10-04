//! The Tauri command surface.
//!
//! Everything the frontend can invoke lives here. Sampling is not a command:
//! it starts once at setup and pushes [`crate::model::Snapshot`] values on
//! the [`SNAPSHOT_EVENT`] event, so the only polling the UI does is for
//! layout and interval changes.

use crate::layout::{Layout, LayoutStore};
use crate::sample::{MAX_INTERVAL_MS, MIN_INTERVAL_MS, Sampler};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Manager, State};

/// Event name the frontend subscribes to for live data.
pub const SNAPSHOT_EVENT: &str = "hamon://snapshot";

/// Shared, mutable application state.
pub struct AppState {
    pub sampler: Sampler,
    pub layout: Mutex<Layout>,
    pub store: LayoutStore,
}

impl AppState {
    fn layout_snapshot(&self) -> Layout {
        self.layout.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Runs `f` under the layout lock and persists the result.
    ///
    /// Every mutating command follows the same shape: mutate in memory, save
    /// to disk, return the new layout. Saving first would mean a failure
    /// leaves the UI and the file disagreeing; saving second means a failure
    /// leaves them agreeing on the new value and only the file stale, which
    /// the next successful save repairs.
    fn update_layout(
        &self,
        f: impl FnOnce(&mut Layout) -> Result<(), String>,
    ) -> Result<Layout, String> {
        let updated = {
            let mut guard = self
                .layout
                .lock()
                .map_err(|_| "layout lock was poisoned by a previous panic".to_string())?;
            f(&mut guard)?;
            guard.clone()
        };
        self.store.save(&updated)?;
        Ok(updated)
    }
}

/// Start/stop status, so the UI can show a paused state.
///
/// No `rename_all`: every other type on the wire is snake_case, and a command
/// response that renamed its own fields would be the one place the frontend has
/// to remember a different convention.
/// `tests::responses_use_the_same_field_naming_as_the_snapshots` pins this.
#[derive(Debug, Serialize)]
pub struct RuntimeStatus {
    pub running: bool,
    pub interval_ms: u64,
}

/// Current status of the sampler.
#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> RuntimeStatus {
    RuntimeStatus {
        running: state.sampler.is_running(),
        interval_ms: state.sampler.interval_ms(),
    }
}

/// Forces the next sample to happen immediately.
///
/// The frontend calls this on window focus so the numbers match the moment
/// the user is looking at them, rather than trailing by up to one interval.
#[tauri::command]
pub fn refresh(state: State<'_, AppState>) {
    state.sampler.notify();
}

/// Pauses sampling. The UI keeps the last snapshot on screen, dimmed.
#[tauri::command]
pub fn pause_sampling(state: State<'_, AppState>) {
    state.sampler.stop();
}

/// Resumes sampling after a [`pause_sampling`].
///
/// The sampler keeps the emit callback it was given at startup, so this is
/// just a flag flip on the backend's side.
#[tauri::command]
pub fn resume_sampling(state: State<'_, AppState>) {
    state.sampler.resume();
}

/// The saved layout, or defaults on first run.
#[tauri::command]
pub fn get_layout(state: State<'_, AppState>) -> Layout {
    state.layout_snapshot()
}

/// Replaces the whole layout and persists it.
#[tauri::command]
pub fn set_layout(state: State<'_, AppState>, layout: Layout) -> Result<Layout, String> {
    state.update_layout(|current| {
        *current = layout.clone();
        Ok(())
    })
}

/// Adds a widget of the given kind and persists the result.
#[tauri::command]
pub fn add_widget(state: State<'_, AppState>, kind: String) -> Result<Layout, String> {
    let kind = kind.trim().to_string();
    if kind.is_empty() {
        return Err("widget kind must not be empty".into());
    }
    state.update_layout(|l| {
        l.add(&kind);
        Ok(())
    })
}

/// Removes the widget at `index`.
#[tauri::command]
pub fn remove_widget(state: State<'_, AppState>, index: usize) -> Result<Layout, String> {
    state.update_layout(|l| {
        if l.remove(index) {
            Ok(())
        } else {
            Err(format!("no widget at index {index}"))
        }
    })
}

/// Moves the widget at `from` to `to`.
#[tauri::command]
pub fn move_widget(state: State<'_, AppState>, from: usize, to: usize) -> Result<Layout, String> {
    state.update_layout(|l| {
        if l.move_widget(from, to) {
            Ok(())
        } else {
            Err(format!("cannot move widget from index {from} to {to}"))
        }
    })
}

/// Updates one widget's configuration payload.
#[tauri::command]
pub fn configure_widget(
    state: State<'_, AppState>,
    index: usize,
    config: serde_json::Value,
) -> Result<Layout, String> {
    state.update_layout(|l| {
        if l.configure(index, config) {
            Ok(())
        } else {
            Err(format!("no widget at index {index}"))
        }
    })
}

/// Changes the sampling interval, clamped to the supported range.
#[tauri::command]
pub fn set_sample_interval(state: State<'_, AppState>, interval_ms: u64) -> Result<Layout, String> {
    let clamped = state.sampler.set_interval_ms(interval_ms);
    state.update_layout(|l| {
        l.sample_interval_ms = clamped;
        Ok(())
    })
}

/// Human-readable interval bounds, so the UI can render a valid slider
/// without hardcoding the same numbers in two places.
#[derive(Serialize)]
pub struct IntervalBounds {
    pub min_ms: u64,
    pub max_ms: u64,
}

#[tauri::command]
pub fn interval_bounds() -> IntervalBounds {
    IntervalBounds {
        min_ms: MIN_INTERVAL_MS,
        max_ms: MAX_INTERVAL_MS,
    }
}

/// Everything the frontend needs in a single round trip at startup.
///
/// Bundling these avoids a visible "empty then filled" flash on launch: the
/// first paint already has the layout and the privilege hint.
#[derive(Serialize)]
pub struct Bootstrap {
    pub layout: Layout,
    pub status: RuntimeStatus,
    pub bounds: IntervalBounds,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    Bootstrap {
        layout: state.layout_snapshot(),
        status: RuntimeStatus {
            running: state.sampler.is_running(),
            interval_ms: state.sampler.interval_ms(),
        },
        bounds: IntervalBounds {
            min_ms: MIN_INTERVAL_MS,
            max_ms: MAX_INTERVAL_MS,
        },
    }
}

/// Resets the dashboard back to the default widget set.
#[tauri::command]
pub fn reset_layout(state: State<'_, AppState>) -> Result<Layout, String> {
    state.update_layout(|l| {
        *l = Layout::default();
        Ok(())
    })
}

/// Builds the sampler, starts it, and registers the command surface.
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("no config directory available: {e}"))?;
    let store = LayoutStore::new(&config_dir);
    let layout = store.load();
    let sampler = Sampler::new(layout.sample_interval_ms);

    let handle = app.handle().clone();
    sampler.spawn(move |snapshot| {
        use tauri::Emitter;
        // A failed emit means the window is gone; the sampler exits on its own
        // shortly after, so there is nothing useful to do here.
        if let Err(e) = handle.emit(SNAPSHOT_EVENT, snapshot) {
            log::debug!("snapshot emit failed: {e}");
        }
    });

    app.manage(AppState {
        sampler,
        layout: Mutex::new(layout),
        store,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_match_sampler_limits() {
        let b = interval_bounds();
        assert_eq!(b.min_ms, MIN_INTERVAL_MS);
        assert_eq!(b.max_ms, MAX_INTERVAL_MS);
        assert!(b.min_ms < b.max_ms);
    }

    #[test]
    fn responses_use_the_same_field_naming_as_the_snapshots() {
        // The frontend's `types.ts` mirrors every wire type by hand, so a rename
        // on one side silently becomes `undefined` on the other. Pinning the
        // spelling here means the mismatch fails a Rust test rather than
        // producing a dashboard full of em dashes.
        let json = serde_json::to_string(&RuntimeStatus {
            running: true,
            interval_ms: 1000,
        })
        .unwrap();
        assert!(json.contains("\"running\":true"), "{json}");
        assert!(json.contains("\"interval_ms\":1000"), "{json}");
        assert!(!json.contains("intervalMs"), "{json}");

        let bounds = serde_json::to_value(interval_bounds()).unwrap();
        assert_eq!(bounds["min_ms"], MIN_INTERVAL_MS);
        assert_eq!(bounds["max_ms"], MAX_INTERVAL_MS);

        let snapshot_key = crate::model::Snapshot::default();
        let snapshot_json = serde_json::to_value(&snapshot_key).unwrap();
        let object = snapshot_json.as_object().unwrap();
        assert!(
            object.keys().all(|k| !k.chars().any(char::is_uppercase)),
            "a snapshot field is not snake_case: {object:?}"
        );
    }

    #[test]
    fn bootstrap_carries_layout_status_and_bounds() {
        let json = serde_json::to_value(Bootstrap {
            layout: Layout::default(),
            status: RuntimeStatus {
                running: true,
                interval_ms: 500,
            },
            bounds: interval_bounds(),
        })
        .unwrap();
        assert!(
            json["layout"]["widgets"]
                .as_array()
                .is_some_and(|w| !w.is_empty())
        );
        assert_eq!(json["status"]["interval_ms"], 500);
        assert_eq!(json["bounds"]["min_ms"], MIN_INTERVAL_MS);
    }
}
