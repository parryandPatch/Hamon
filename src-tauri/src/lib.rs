//! Hamon — a cross-platform hardware monitor.
//!
//! The architecture is deliberately small:
//!
//! ```text
//!   [ sampler thread ]  ──emit(Snapshot)──▶  [ webview ]
//!         │                                      ▲
//!         ▼                                      │ commands
//!   [ collectors ]  ◀── layout / interval ──  [ commands.rs ]
//! ```
//!
//! Collectors are synchronous and hold all their own state; the sampler is the
//! only place that decides cadence; the frontend only ever sees one flat
//! `Snapshot` per tick plus layout edits.

pub mod collect;
pub mod commands;
pub mod layout;
pub mod model;
pub mod platform;
pub mod sample;

use commands::AppState;

/// Entry point for desktop builds.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // Keep the log file bounded: the sampler is chatty at debug
                // level and a runaway log would grow without limit.
                .max_file_size(2 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: None },
                ))
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::get_layout,
            commands::set_layout,
            commands::add_widget,
            commands::remove_widget,
            commands::move_widget,
            commands::configure_widget,
            commands::set_sample_interval,
            commands::reset_layout,
            commands::get_status,
            commands::refresh,
            commands::pause_sampling,
            commands::resume_sampling,
            commands::interval_bounds,
        ])
        .setup(|app| {
            // `Manager` supplies `get_webview_window` and `state`.
            use tauri::Manager;

            // Stop sampling when the window goes away so the process can exit
            // instead of lingering on a background thread.
            if let Some(window) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    use tauri::WindowEvent;
                    if matches!(event, WindowEvent::Destroyed) {
                        handle.state::<AppState>().sampler.stop();
                    }
                });
            }
            commands::setup(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Hamon")
        .run(|_app, event| {
            use tauri::RunEvent;
            // Belt and braces: `WindowEvent::Destroyed` only fires for the
            // main window, whereas `ExitRequested` covers every exit path.
            if let RunEvent::ExitRequested { .. } = event {
                log::info!("exit requested");
            }
        });
}
