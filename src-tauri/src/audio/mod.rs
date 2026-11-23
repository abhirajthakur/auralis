// Audio capture using cpal. Records to WAV files.
use auralis_core::audio::{encode_wav, resample_linear, rms, to_mono};
use std::sync::mpsc::{self, Sender};
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

#[derive(Debug, Clone)]
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
        let handle = std::thread::spawn(move || {
            let host = cpal::default_host();
            let device = device_name
                .and_then(|n| {
                    host.input_devices()
                        .ok()
                        .and_then(|mut devs| devs.find(|d| d.name().map(|nm| nm == n).unwrap_or(false)))
                })
                .unwrap_or_else(|| host.default_input_device().expect("no default input device"));

            let config = device.default_input_config().expect("no input config").config();
            let in_sr = config.sample_rate.0;
            let in_channels = config.channels;

            let (sample_tx, sample_rx) = mpsc::sync_channel::<Vec<f32>>(1024);
            let st = std::sync::Arc::new(std::sync::Mutex::new((
                Vec::<f32>::new(),
                false,        // paused
                std::time::Instant::now(),
                0u64,         // duration_ms
                0f32,         // peak
            )));
            let st2 = st.clone();

            let stream = device
                .build_input_stream(
                    &config,
                    move |samples: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut s = st2.lock().unwrap();
                        if s.1 {
                            return;
                        }
                        let mono = to_mono(samples, in_channels).unwrap_or_else(|_| samples.to_vec());
                        let resampled = resample_linear(&mono, in_sr, target_sr);
                        s.3 = s.2.elapsed().as_millis() as u64;
                        let r = rms(&resampled);
                        s.4 = s.4.max(r);
                        let _ = sample_tx.send(resampled);
                    },
                    |e| tracing::error!("cpal error: {e}"),
                    None,
                )
                .expect("stream build");
            let _stream = stream.play();

            let app2 = app.clone();
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let mut last_emit = std::time::Instant::now();
                loop {
                    std::thread::sleep(Duration::from_millis(20));
                    while let Ok(s) = sample_rx.try_recv() {
                        buf.extend(s);
                    }
                    if last_emit.elapsed() > Duration::from_millis(50) {
                        let r = rms(&buf);
                        let _ = app2.emit("recording//level", serde_json::json!({
                            "rms": r,
                            "peak": 0f32,
                            "speaking": r > silence_threshold
                        }));
                        buf.clear();
                        last_emit = std::time::Instant::now();
                    }
                }
            });

            loop {
                match cmd_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(RecorderCmd::Stop) | Err(_) => {
                        drop(_stream);
                        let mut all = st.lock().unwrap().0.clone();
                        while let Ok(s) = sample_rx.try_recv() {
                            all.extend(s);
                        }
                        if !all.is_empty() {
                            let wav = encode_wav(&all, target_sr);
                            if let Some(dir) =
                                dirs::data_local_dir().map(|d| d.join("Auralis").join("recordings"))
                            {
                                std::fs::create_dir_all(&dir).ok();
                                let path =
                                    dir.join(format!("{}.wav", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
                                if std::fs::write(&path, &wav).is_ok() {
                                    let _ = app.emit("recording//saved", serde_json::json!({
                                        "path": path.to_string_lossy(),
                                        "durationMs": st.lock().unwrap().3
                                    }));
                                }
                            }
                        }
                        let _ = app.emit("recording//state", serde_json::json!({
                            "status": "idle",
                            "durationMs": 0u64
                        }));
                        break;
                    }
                    Ok(RecorderCmd::Cancel) => {
                        drop(_stream);
                        let _ = app.emit("recording//state", serde_json::json!({
                            "status": "idle",
                            "durationMs": 0u64
                        }));
                        break;
                    }
                    Ok(RecorderCmd::Pause) => {
                        st.lock().unwrap().1 = true;
                        let _ = app.emit("recording//state", serde_json::json!({"status": "paused"}));
                    }
                    Ok(RecorderCmd::Resume) => {
                        st.lock().unwrap().1 = false;
                        st.lock().unwrap().2 = std::time::Instant::now();
                        let _ = app.emit("recording//state", serde_json::json!({"status": "recording"}));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });

        let mut h = Self {
            session: RecordingSession { status: "recording".into(), ..Default::default() },
            tx: Some(cmd_tx),
            _jh: Some(handle),
        };
        let _ = app.emit("recording//state", serde_json::json!({
            "status": "recording",
            "durationMs": 0u64
        }));
        h
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