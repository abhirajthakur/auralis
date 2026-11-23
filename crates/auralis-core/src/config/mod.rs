//! Auralis settings model.
//!
//! Defines the full settings schema with safe defaults and validation.
//! Persistence (SQLite in the shell) is handled by `src-tauri/src/storage`;
//! this module stays pure so the schema is unit-testable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod keys {
    /// Well-known credential slot ids, one per provider id.
    pub const GROQ: &str = "groq";
    pub const OPENAI: &str = "openai";
    pub const GEMINI: &str = "gemini";
    pub const ANTHROPIC: &str = "anthropic";
    pub const OPENROUTER: &str = "openrouter";
    pub const CUSTOM: &str = "custom";
}

/// Which refinement pipeline runs between capture and insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TranscriptionMode {
    /// Insert the provider's transcription verbatim. Lowest latency.
    #[default]
    Raw,
    /// Apply the active refinement style first.
    Refined,
}

/// A built-in or user-defined refinement style.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefinementStyle {
    pub id: String,
    pub name: String,
    /// System instruction sent to the refinement provider.
    pub system_prompt: String,
    /// True when shipped with the app and not deletable.
    pub builtin: bool,
}

impl RefinementStyle {
    pub fn builtin_styles() -> Vec<RefinementStyle> {
        vec![
            RefinementStyle {
                id: "raw".into(),
                name: "Raw transcription".into(),
                system_prompt: String::new(),
                builtin: true,
            },
            RefinementStyle {
                id: "smart".into(),
                name: "Smart formatting".into(),
                system_prompt: crate::refinement::prompts::SMART_FORMATTING.into(),
                builtin: true,
            },
            RefinementStyle {
                id: "professional".into(),
                name: "Professional".into(),
                system_prompt: crate::refinement::prompts::PROFESSIONAL.into(),
                builtin: true,
            },
            RefinementStyle {
                id: "casual".into(),
                name: "Casual".into(),
                system_prompt: crate::refinement::prompts::CASUAL.into(),
                builtin: true,
            },
            RefinementStyle {
                id: "concise".into(),
                name: "Concise".into(),
                system_prompt: crate::refinement::prompts::CONCISE.into(),
                builtin: true,
            },
            RefinementStyle {
                id: "email".into(),
                name: "Email".into(),
                system_prompt: crate::refinement::prompts::EMAIL.into(),
                builtin: true,
            },
            RefinementStyle {
                id: "chat".into(),
                name: "Chat message".into(),
                system_prompt: crate::refinement::prompts::CHAT.into(),
                builtin: true,
            },
            RefinementStyle {
                id: "technical".into(),
                name: "Developer / technical".into(),
                system_prompt: crate::refinement::prompts::TECHNICAL.into(),
                builtin: true,
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutConfig {
    /// Start/stop dictation. Parsed platform-side into accelerator strings.
    pub dictation: String,
    /// Cancel current recording without transcribing.
    pub cancel: String,
    /// Open the main window. Empty string disables it.
    pub open_app: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            dictation: "Ctrl+Shift+Space".into(),
            cancel: "Escape".into(),
            open_app: "Ctrl+Shift+A".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AudioConfig {
    /// Preferred input device name. Empty = system default.
    pub device: String,
    /// Capture sample rate in Hz. 16_000 is what every STT provider wants.
    pub sample_rate: u32,
    /// Silence threshold in RMS (0.0..1.0) used by the VAD gate.
    pub silence_threshold: f32,
    /// Pre-roll milliseconds kept before VAD detects speech onset.
    pub preroll_ms: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            sample_rate: 16_000,
            silence_threshold: 0.010,
            preroll_ms: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryConfig {
    pub enabled: bool,
    /// Maximum retained entries; oldest are pruned.
    pub max_entries: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { enabled: true, max_entries: 500 }
    }
}

/// Root settings object persisted by the shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    // --- Providers -------------------------------------------------------
    /// Speech-to-text provider id: groq | openai | gemini | local-whisper
    pub speech_provider: String,
    /// Refinement provider id: groq | openai | openrouter | gemini | anthropic | openai-compatible | none
    pub refinement_provider: String,
    /// Model id per speech provider (e.g. "whisper-large-v3" for Groq).
    pub speech_models: HashMap<String, String>,
    /// Model id per refinement provider.
    pub refinement_models: HashMap<String, String>,
    /// Base URL for the `openai-compatible` refinement provider.
    pub custom_endpoint: Option<String>,
    /// Model id for the custom endpoint.
    pub custom_model: Option<String>,
    // --- Dictation -------------------------------------------------------
    pub mode: TranscriptionMode,
    /// BCP-47 language hint sent to the provider; "auto" = omit.
    pub language: String,
    /// Extra vocabulary biasing, one phrase per entry.
    pub vocabulary: Vec<String>,
    // --- Refinement ------------------------------------------------------
    pub refinement_enabled: bool,
    /// Active refinement style id.
    pub style_id: String,
    /// User-defined styles.
    pub custom_styles: Vec<RefinementStyle>,
    // --- Behaviour -------------------------------------------------------
    pub shortcuts: ShortcutConfig,
    pub audio: AudioConfig,
    pub history: HistoryConfig,
    pub launch_at_startup: bool,
    /// "dark" is the only theme in v0.1; light is reserved.
    pub theme: String,
    /// After insertion, return focus/clipboard exactly as found.
    pub preserve_clipboard: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            speech_provider: "groq".into(),
            refinement_provider: "groq".into(),
            speech_models: HashMap::from([
                ("groq".to_string(), "whisper-large-v3".to_string()),
                ("openai".to_string(), "gpt-4o-mini-transcribe".to_string()),
                ("gemini".to_string(), "gemini-2.0-flash".to_string()),
                ("local-whisper".to_string(), "base.en".to_string()),
            ]),
            refinement_models: HashMap::from([
                ("groq".to_string(), "llama-3.3-70b-versatile".to_string()),
                ("openai".to_string(), "gpt-4o-mini".to_string()),
                ("gemini".to_string(), "gemini-2.0-flash".to_string()),
                ("openrouter".to_string(), "openai/gpt-4o-mini".to_string()),
                ("anthropic".to_string(), "claude-3-5-haiku-latest".to_string()),
                ("openai-compatible".to_string(), String::new()),
            ]),
            custom_endpoint: None,
            custom_model: None,
            mode: TranscriptionMode::Raw,
            language: "auto".into(),
            vocabulary: Vec::new(),
            refinement_enabled: false,
            style_id: "smart".into(),
            custom_styles: Vec::new(),
            shortcuts: ShortcutConfig::default(),
            audio: AudioConfig::default(),
            history: HistoryConfig::default(),
            launch_at_startup: false,
            theme: "dark".into(),
            preserve_clipboard: true,
        }
    }
}

impl Settings {
    /// All styles: built-ins plus user-defined, with defaults first.
    pub fn all_styles(&self) -> Vec<RefinementStyle> {
        let mut all = RefinementStyle::builtin_styles();
        all.extend(self.custom_styles.iter().cloned());
        all
    }

    pub fn style(&self, id: &str) -> Option<RefinementStyle> {
        self.all_styles().into_iter().find(|s| s.id == id)
    }

    pub fn speech_model(&self, provider: &str) -> &str {
        self.speech_models
            .get(provider)
            .map(|s| s.as_str())
            .unwrap_or_default()
    }

    pub fn refinement_model(&self, provider: &str) -> &str {
        self.refinement_models
            .get(provider)
            .map(|s| s.as_str())
            .unwrap_or_default()
    }

    /// Validate settings, coercing impossible values to defaults.
    /// Returns the list of problems found (already fixed in the returned copy).
    pub fn sanitized(mut self) -> (Self, Vec<String>) {
        let mut problems = Vec::new();

        let speech_ids = ["groq", "openai", "gemini", "local-whisper"];
        if !speech_ids.contains(&self.speech_provider.as_str()) {
            problems.push(format!("unknown speech provider '{}'", self.speech_provider));
            self.speech_provider = "groq".into();
        }

        let refinement_ids = [
            "groq", "openai", "openrouter", "gemini", "anthropic", "openai-compatible", "none",
        ];
        if !refinement_ids.contains(&self.refinement_provider.as_str()) {
            problems.push(format!(
                "unknown refinement provider '{}'",
                self.refinement_provider
            ));
            self.refinement_provider = "groq".into();
        }

        if self.custom_endpoint.as_deref().map(str::trim) == Some("") {
            self.custom_endpoint = None;
        }

        if self.mode == TranscriptionMode::Refined
            && self.refinement_enabled
            && self.refinement_provider == "none"
        {
            self.refinement_enabled = false;
            problems.push("refinement enabled but provider is 'none'".into());
        }

        if !self.style(&self.style_id).is_some() {
            problems.push(format!("missing style '{}'", self.style_id));
            self.style_id = "smart".into();
        }

        let sr = self.audio.sample_rate;
        if !(8_000..=192_000).contains(&sr) {
            problems.push(format!("sample rate {sr} out of range"));
            self.audio.sample_rate = 16_000;
        }
        if self.audio.silence_threshold < 0.0 || self.audio.silence_threshold > 1.0 {
            self.audio.silence_threshold = 0.010;
        }
        if self.audio.preroll_ms > 5_000 {
            self.audio.preroll_ms = 300;
        }

        if self.history.max_entries == 0 || self.history.max_entries > 10_000 {
            self.history.max_entries = 500;
        }

        (self, problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        let (s, problems) = Settings::default().sanitized();
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(s.speech_provider, "groq");
        assert_eq!(s.speech_model("groq"), "whisper-large-v3");
        assert_eq!(s.shortcuts.dictation, "Ctrl+Shift+Space");
    }

    #[test]
    fn sanitizer_fixes_unknown_providers() {
        let mut s = Settings::default();
        s.speech_provider = "skynet".into();
        s.refinement_provider = "hal9000".into();
        let (s, problems) = s.sanitized();
        assert_eq!(problems.len(), 2);
        assert_eq!(s.speech_provider, "groq");
    }

    #[test]
    fn refinement_requires_provider() {
        let mut s = Settings::default();
        s.mode = TranscriptionMode::Refined;
        s.refinement_enabled = true;
        s.refinement_provider = "none".into();
        let (s, problems) = s.sanitized();
        assert!(!s.refinement_enabled);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn builtin_styles_present() {
        let s = Settings::default();
        let styles = s.all_styles();
        assert!(styles.len() >= 8);
        assert!(s.style("raw").is_some());
        assert!(s.style("technical").is_some());
        assert!(s.style("nope").is_none());
        assert!(s.custom_styles.is_empty());
    }

    #[test]
    fn custom_styles_are_appended() {
        let mut s = Settings::default();
        s.custom_styles.push(RefinementStyle {
            id: "legal".into(),
            name: "Legal brief".into(),
            system_prompt: "Formal legal register.".into(),
            builtin: false,
        });
        assert!(s.style("legal").is_some());
        assert!(!s.style("legal").unwrap().builtin);
    }

    #[test]
    fn json_round_trip_preserves_settings() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
