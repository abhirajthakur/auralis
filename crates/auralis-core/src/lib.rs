//! Auralis core engine.
//!
//! Pure-Rust library containing everything that must be fast, testable, and
//! independent of the desktop shell:
//!
//! - [`audio`]: capture buffer, resampling, WAV encoding, VAD gate
//! - [`transcription`]: speech-to-text provider abstraction + adapters
//! - [`refinement`]: AI text-refinement engine, styles, prompt building
//! - [`config`]: settings model with serde defaults and validation
//! - [`security`]: secret-store abstraction (OS keyring wired in the shell)
//!
//! The Tauri shell (`src-tauri`) depends on this crate and wires it to the
//! desktop: cpal capture, global shortcuts, text insertion, SQLite, tray.

pub mod audio;
pub mod config;
pub mod refinement;
pub mod security;
pub mod transcription;

pub const APP_NAME: &str = "Auralis";
pub const TAGLINE: &str = "Your voice, refined.";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
