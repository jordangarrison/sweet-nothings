//! Model management module
//!
//! Handles downloading and managing whisper models.

mod download;

pub use download::{download_model, prompt_download, AVAILABLE_MODELS};
