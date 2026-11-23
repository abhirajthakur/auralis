// Auralis desktop shell — wires the auralis-core engine to the OS.
//
// Responsibilities (Milestone 1):
// - app state (settings + recorder)
// - tray icon with Show/Quit
// - Tauri command registration
// - crash logging (panics land in %LOCALAPPDATA%\Auralis\logs on Windows)
//
// Global shortcuts, floating recorder window, and text insertion arrive in
// Milestone 4.

mod audio;
mod commands;
mod storage;

use auralis_core::config::Settings;
use parking_lot::RwLock;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

/// Shared application state managed by Tauri.
pub struct AppState {
    pub settings: RwLock<Settings>,
    pub recorder: RwLock<audio::RecorderHandle>,
}

/// Install a panic hook that persists crashes to disk. Without this, a GUI
/// process on Windows dies silently — exactly the kind of bug that is
/// impossible to diagnose from an installer.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("Auralis panic: {info}\n");
        eprintln!("{msg}");
        if let Some(dir) = dirs::data_local_dir().map(|d| d.join("Auralis").join("logs")) {
            let _ = std::fs::create_dir_all(&dir);
            let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let _ = std::fs::write(dir.join(format!("crash-{ts}.log")), &msg);
        }
    }));
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Tray with Show/Quit. Failure here is non-fatal: the app remains usable
/// from its window even if the desktop environment has no tray host.
fn build_tray(app: &tauri::App) -> Result<(), tauri::Error> {
    let show = MenuItem::with_id(app, "show", "Show Auralis", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id("auralis-tray")
        .icon(icon)
        .menu(&menu)
        .tooltip("Auralis — Your voice, refined.")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();

    // Settings: non-fatal. A missing/corrupt DB falls back to defaults and is
    // rewritten on the next save.
    let settings = storage::SettingsStore::new()
        .and_then(|store| store.load())
        .unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second launch: focus the existing window instead of opening a
            // second copy.
            show_main(&app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: RwLock::new(settings),
            recorder: RwLock::new(audio::RecorderHandle::idle()),
        })
        .setup(|app| {
            if let Err(e) = build_tray(app) {
                eprintln!("tray unavailable: {e}");
            }
            tracing::info!("Auralis v{} ready", auralis_core::VERSION);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::list_audio_devices,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::stop_recording,
            commands::cancel_recording,
            commands::list_recordings,
        ])
        .run(tauri::generate_context!())
        .expect("Auralis failed to start");
}
