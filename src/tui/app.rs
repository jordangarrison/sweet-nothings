//! App state machine

use anyhow::Result;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::audio::{CpalRecorder, Recorder};
use crate::clipboard::{Clipboard, SystemClipboard};
use crate::config::Config;
use crate::transcribe::{Transcriber, WhisperCliTranscriber};

/// App state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Recording audio
    Recording,
    /// Transcribing audio
    Transcribing,
    /// Done, showing result
    Done,
    /// Error occurred
    Error,
}

/// Main application
pub struct App {
    /// Current state
    state: AppState,
    /// Audio recorder
    recorder: CpalRecorder,
    /// Transcriber (stored for later use in thread)
    transcriber_model_path: PathBuf,
    /// Clipboard
    clipboard: SystemClipboard,
    /// Configuration
    config: Config,
    /// Recording start time
    recording_start: Option<Instant>,
    /// Path to recorded audio
    audio_path: Option<PathBuf>,
    /// Channel to receive transcription result
    transcription_rx: Option<Receiver<Result<String>>>,
    /// Transcription result
    transcription: Option<String>,
    /// Error message
    error: Option<String>,
    /// Time when result was shown
    result_shown_at: Option<Instant>,
}

impl App {
    pub fn new(
        recorder: CpalRecorder,
        clipboard: SystemClipboard,
        config: Config,
    ) -> Self {
        Self {
            state: AppState::Recording,
            recorder,
            transcriber_model_path: config.model_path(),
            clipboard,
            config,
            recording_start: None,
            audio_path: None,
            transcription_rx: None,
            transcription: None,
            error: None,
            result_shown_at: None,
        }
    }

    pub fn state(&self) -> AppState {
        self.state
    }

    pub fn is_recording(&self) -> bool {
        self.state == AppState::Recording
    }

    #[allow(dead_code)]
    pub fn is_transcribing(&self) -> bool {
        self.state == AppState::Transcribing
    }

    pub fn is_done(&self) -> bool {
        self.state == AppState::Done
    }

    pub fn is_error(&self) -> bool {
        self.state == AppState::Error
    }

    pub fn recording_duration(&self) -> Duration {
        self.recording_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    pub fn audio_level(&self) -> f32 {
        self.recorder.audio_level()
    }

    pub fn transcription(&self) -> Option<&str> {
        self.transcription.as_deref()
    }

    pub fn transcription_result(&self) -> Option<&str> {
        self.transcription.as_deref()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[allow(dead_code)]
    pub fn model_name(&self) -> &str {
        &self.config.model
    }

    pub fn should_exit(&self) -> bool {
        if self.state == AppState::Done {
            if let Some(shown_at) = self.result_shown_at {
                return shown_at.elapsed() >= self.config.exit_delay;
            }
        }
        false
    }

    pub fn start_recording(&mut self) -> Result<()> {
        self.recorder.start()?;
        self.recording_start = Some(Instant::now());
        self.state = AppState::Recording;
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<()> {
        let audio_path = self.recorder.stop()?;
        self.audio_path = Some(audio_path.clone());
        self.state = AppState::Transcribing;

        // Start transcription in background thread
        let model_path = self.transcriber_model_path.clone();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let result = (|| -> Result<String> {
                let transcriber = WhisperCliTranscriber::auto_detect(model_path)?;
                transcriber.transcribe(&audio_path)
            })();
            let _ = tx.send(result);
        });

        self.transcription_rx = Some(rx);

        Ok(())
    }

    pub fn check_transcription_result(&mut self) {
        if let Some(rx) = &self.transcription_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(text) => {
                        // Copy to clipboard
                        if let Err(e) = self.clipboard.copy(&text) {
                            self.error = Some(format!("Failed to copy to clipboard: {}", e));
                            self.state = AppState::Error;
                            return;
                        }

                        // Auto-paste if enabled
                        if self.config.auto_paste {
                            if let Err(e) = self.clipboard.paste() {
                                // Don't fail on paste error, just continue
                                eprintln!("Warning: Failed to auto-paste: {}", e);
                            }
                        }

                        self.transcription = Some(text);
                        self.state = AppState::Done;
                        self.result_shown_at = Some(Instant::now());
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        self.state = AppState::Error;
                    }
                }

                // Cleanup audio file
                if let Some(path) = &self.audio_path {
                    let _ = std::fs::remove_file(path);
                }

                self.transcription_rx = None;
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Cleanup audio file if still exists
        if let Some(path) = &self.audio_path {
            let _ = std::fs::remove_file(path);
        }
    }
}
