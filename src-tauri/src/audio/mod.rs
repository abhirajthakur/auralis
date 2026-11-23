// Audio capture using cpal. Records to WAV files.
//
// Architecture:
// - cpal callback → SyncSender<Vec<f32>> (chunked, non-blocking on the
//   realtime thread)
// - level thread: drains chunks, emits `recording//level` events every 50 ms
// - command loop: handles Stop/Cancel/Pause/Resume, on Stop drains remaining
//   chunks, encodes WAV, saves to %LOCALAPPDATA%\Auralis\recordings
//
// The sample receiver is shared (Arc<Mutex<Receiver>>) between the level
// thread and the stop-drain so no chunk is lost at either end.

use auralis_core::audio::{encode_wav, resample_linear, rms, to_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;

#[derive(Debug, Clone, Default)]
pub struct RecordingSession {
    pub status: String,
    pub duration_ms: u64,
    pub rms: f32,
    pub peak: f32,
    pub speaking: bool,
}

#[derive(Debug)]
pub struct RecorderHandle {
    pub session: RecordingSession,
    pub tx: Option<Sender<RecorderCmd>>,
    #[allow(dead_code)]
    _jh: Option<std::thread::JoinHandle<()>>,
}

#[derive(Debug)]
pub enum RecorderCmd {
    Stop,
    Pause,
    Resume,
    Cancel,
}

type SharedSamples = Arc<Mutex<Receiver<Vec<f32>>>>;

/// Lock the receiver and pull every pending chunk without blocking.
fn drain_pending(rx: &SharedSamples, into: &mut Vec<f32>) {
    if let Ok(rx) = rx.lock() {
        while let Ok(chunk) = rx.try_recv() {
            into.extend(chunk);
        }
    }
}

impl RecorderHandle {
    pub fn idle() -> Self {
        Self { session: RecordingSession::default(), tx: None, _jh: None }
    }

    pub fn start(
        app: tauri::AppHandle,
        device_name: Option<String>,
        target_sr: u32,
        silence_threshold: f32,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        // Move a clone into the recorder thread; the original stays with the
        // handle for the initial state emit.
        let app_thread = app.clone();

        let handle = std::thread::Builder::new()
            .name("auralis-recorder".into())
            .spawn(move || recorder_thread(app_thread, cmd_rx, device_name, target_sr, silence_threshold))
            .ok();

        let _ = app.emit(
            "recording//state",
            serde_json::json!({ "status": "recording", "durationMs": 0u64 }),
        );

        Self {
            session: RecordingSession { status: "recording".into(), ..Default::default() },
            tx: Some(cmd_tx),
            _jh: handle,
        }
    }

    pub fn pause(&mut self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(RecorderCmd::Pause);
        }
        self.session.status = "paused".into();
    }

    pub fn resume(&mut self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(RecorderCmd::Resume);
        }
        self.session.status = "recording".into();
    }

    pub fn stop(&mut self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(RecorderCmd::Stop);
        }
        self.session.status = "idle".into();
    }

    pub fn cancel(&mut self) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(RecorderCmd::Cancel);
        }
        self.session.status = "idle".into();
    }
}

struct Shared {
    samples: Vec<f32>,
    paused: bool,
    started: std::time::Instant,
    peak: f32,
}

fn recorder_thread(
    app: tauri::AppHandle,
    cmd_rx: mpsc::Receiver<RecorderCmd>,
    device_name: Option<String>,
    target_sr: u32,
    silence_threshold: f32,
) {
    let host = cpal::default_host();
    let device = match device_name
        .and_then(|n| {
            host.input_devices()
                .ok()
                .and_then(|mut devs| devs.find(|d| d.name().map(|nm| nm == n).unwrap_or(false)))
        }) {
        Some(d) => Some(d),
        None => host.default_input_device(),
    };
    let Some(device) = device else {
        let _ = app.emit(
            "recording//error",
            serde_json::json!({ "message": "No microphone found. Check that a recording device is connected and enabled." }),
        );
        let _ = app.emit(
            "recording//state",
            serde_json::json!({ "status": "idle", "durationMs": 0u64 }),
        );
        return;
    };

    let Ok(config) = device.default_input_config().map(|c| c.config()) else {
        let _ = app.emit(
            "recording//error",
            serde_json::json!({ "message": "Could not read the microphone configuration." }),
        );
        let _ = app.emit(
            "recording//state",
            serde_json::json!({ "status": "idle", "durationMs": 0u64 }),
        );
        return;
    };
    let in_sr = config.sample_rate.0;
    let in_channels = config.channels;

    let (sample_tx, sample_rx): (SyncSender<Vec<f32>>, Receiver<Vec<f32>>) = mpsc::sync_channel(256);
    let sample_rx = Arc::new(Mutex::new(sample_rx));
    let shared = Arc::new(Mutex::new(Shared {
        samples: Vec::new(),
        paused: false,
        started: std::time::Instant::now(),
        peak: 0.0,
    }));

    // Realtime capture callback: downmix → resample → authoritative buffer,
    // plus a non-blocking copy to the metering channel (dropped if full —
    // captured audio is never lost, only the level meter can skip frames).
    let shared_cb = Arc::clone(&shared);
    let stream = device.build_input_stream(
        &config,
        move |samples: &[f32], _: &cpal::InputCallbackInfo| {
            let mut s = shared_cb.lock().unwrap();
            if s.paused {
                return;
            }
            let mono = to_mono(samples, in_channels).unwrap_or_else(|_| samples.to_vec());
            let resampled = resample_linear(&mono, in_sr, target_sr);
            s.peak = s.peak.max(rms(&resampled));
            s.samples.extend_from_slice(&resampled);
            let _ = sample_tx.try_send(resampled);
        },
        move |e| tracing::error!("cpal stream error: {e}"),
        None,
    );

    let stream = match stream {
        Ok(s) => match s.play() {
            Ok(()) => Some(s),
            Err(e) => {
                tracing::error!("cpal play failed: {e}");
                None
            }
        },
        Err(e) => {
            tracing::error!("cpal stream build failed: {e}");
            None
        }
    };

    // Level thread: emits RMS metering events.
    let app_level = app.clone();
    let level_rx = Arc::clone(&sample_rx);
    let level_handle = std::thread::Builder::new()
        .name("auralis-level".into())
        .spawn(move || {
            let mut last_emit = std::time::Instant::now();
            let mut pending: Vec<f32> = Vec::new();
            loop {
                if last_emit.elapsed() >= Duration::from_millis(50) {
                    drain_pending(&level_rx, &mut pending);
                    let r = rms(&pending);
                    let _ = app_level.emit(
                        "recording//level",
                        serde_json::json!({
                            "rms": r,
                            "peak": 0f32,
                            "speaking": r > silence_threshold,
                        }),
                    );
                    pending.clear();
                    last_emit = std::time::Instant::now();
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        })
        .ok();

    // Command loop.
    let mut all_samples: Vec<f32> = Vec::new();
    'run: loop {
        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(RecorderCmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                drop(stream); // stop the callback first, then drain
                drain_pending(&sample_rx, &mut all_samples);
                if let Ok(mut s) = shared.lock() {
                    all_samples.extend(s.samples.drain(..));
                }
                save_recording(&app, &all_samples, target_sr);
                let _ = app.emit(
                    "recording//state",
                    serde_json::json!({ "status": "idle", "durationMs": 0u64 }),
                );
                break 'run;
            }
            Ok(RecorderCmd::Cancel) => {
                drop(stream);
                if let Ok(mut s) = shared.lock() {
                    s.samples.clear();
                }
                let _ = app.emit(
                    "recording//state",
                    serde_json::json!({ "status": "idle", "durationMs": 0u64 }),
                );
                break 'run;
            }
            Ok(RecorderCmd::Pause) => {
                if let Ok(mut s) = shared.lock() {
                    s.paused = true;
                }
                let _ = app.emit("recording//state", serde_json::json!({ "status": "paused" }));
            }
            Ok(RecorderCmd::Resume) => {
                if let Ok(mut s) = shared.lock() {
                    s.paused = false;
                    s.started = std::time::Instant::now();
                }
                let _ = app.emit("recording//state", serde_json::json!({ "status": "recording" }));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    // Level thread runs until process exit; it will block forever on the
    // drained receiver, which is fine — it's a daemon. Explicitly detached.
    drop(level_handle);
}

fn save_recording(app: &tauri::AppHandle, samples: &[f32], target_sr: u32) {
    if samples.is_empty() {
        return;
    }
    let duration_ms = (samples.len() as u64 * 1000) / target_sr as u64;
    let wav = encode_wav(samples, target_sr);
    let Some(dir) = dirs::data_local_dir().map(|d| d.join("Auralis").join("recordings")) else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("{}.wav", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    match std::fs::write(&path, &wav) {
        Ok(()) => {
            let _ = app.emit(
                "recording//saved",
                serde_json::json!({
                    "id": path.file_stem().and_then(|s| s.to_str()).unwrap_or_default(),
                    "path": path.to_string_lossy(),
                    "durationMs": duration_ms,
                    "sampleRate": target_sr,
                }),
            );
        }
        Err(e) => {
            tracing::error!("failed to save recording: {e}");
            let _ = app.emit(
                "recording//error",
                serde_json::json!({ "message": format!("Could not save the recording: {e}") }),
            );
        }
    }
}
