// Tauri IPC commands for Auralis.
use crate::AppState;
use auralis_core::config::Settings;
use std::path::PathBuf;
use tauri::State;

// --- Settings ---
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.read().clone()
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    if let Ok(store) = storage::SettingsStore::new() {
        let _ = store.save(&settings).map_err(|e| e.to_string());
    }
    *state.settings.write() = settings;
    Ok(())
}

// --- Audio devices ---
#[tauri::command]
pub fn list_audio_devices() -> Vec<serde_json::Value> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|mut devs| {
            devs.filter_map(|d| {
                d.name().ok().map(|name| {
                    serde_json::json!({
                        "name": name,
                        "isDefault": false
                    })
                })
            }).collect()
        })
        .unwrap_or_default()
}

// --- Recording ---
#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>, device: Option<String>) -> Result<(), String> {
    let cfg = state.settings.read().audio.clone();
    let device = device.or_else(|| {
        let d = &state.settings.read().audio.device;
        if d.is_empty() { None } else { Some(d.clone()) }
    });
    let mut rec = state.recorder.write();
    *rec = audio::RecorderHandle::start(
        state.0.clone(),
        device,
        cfg.sample_rate,
        cfg.silence_threshold,
    );
    Ok(())
}

#[tauri::command]
pub fn pause_recording(state: State<AppState>) -> Result<(), String> {
    state.recorder.write().pause();
    Ok(())
}

#[tauri::command]
pub fn resume_recording(state: State<AppState>) -> Result<(), String> {
    state.recorder.write().resume();
    Ok(())
}

#[tauri::command]
pub fn stop_recording(state: State<AppState>) -> Result<(), String> {
    state.recorder.write().stop();
    Ok(())
}

#[tauri::command]
pub fn cancel_recording(state: State<AppState>) -> Result<(), String> {
    state.recorder.write().cancel();
    Ok(())
}

// --- Recordings list ---
#[derive(serde::Serialize)]
pub struct RecordingEntry {
    pub id: String,
    pub path: String,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub created_at: String,
}

#[tauri::command]
pub fn list_recordings() -> Vec<RecordingEntry> {
    let dir = dirs::data_local_dir()
        .map(|d| d.join("Auralis").join("recordings"))
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::read_dir(&dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("wav"))
                .filter_map(|e| {
                    let path = e.path();
                    let name = path.file_stem()?.to_str()?;
                    let meta = e.metadata().ok()?;
                    let modified = meta.modified().ok()?;
                    let duration = auralis_core::audio::wav_sample_rate(&std::fs::read(&path).ok()?)
                        .map(|sr| {
                            let data_len = std::fs::read(&path).ok().map(|b| b.len()).unwrap_or(0).saturating_sub(44);
                            (data_len as u64 / (sr as u64 * 2)) * 1000
                        })
                        .unwrap_or(0);
                    Some(RecordingEntry {
                        id: name.to_string(),
                        path: path.to_string_lossy().to_string(),
                        duration_ms: duration,
                        sample_rate: 16000,
                        created_at: chrono::DateTime::from(modified)
                            .format("%Y-%m-%dT%H:%M:%S").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

mod storage;