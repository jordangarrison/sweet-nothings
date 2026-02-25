//! Backend registry
//!
//! Maps backend names to implementations, gated by feature flags.

use super::TranscriptionBackend;
use super::WhisperCliBackend;
use anyhow::{bail, Result};

/// List all backend names that are compiled in
pub fn available_backend_names() -> Vec<&'static str> {
    #[allow(unused_mut)]
    let mut backends = vec!["whisper"];
    #[cfg(feature = "parakeet")]
    backends.push("parakeet");
    backends
}

#[allow(dead_code)]
/// Create a backend instance by name.
///
/// For whisper: `model_path` is the resolved path to the ggml model file.
/// The whisper binary is auto-detected.
pub fn create_backend(
    backend_name: &str,
    model_path: &std::path::PathBuf,
) -> Result<Box<dyn TranscriptionBackend>> {
    match backend_name {
        "whisper" => {
            let backend = WhisperCliBackend::auto_detect(model_path.clone())?;
            Ok(Box::new(backend))
        }
        #[cfg(feature = "parakeet")]
        "parakeet" => {
            let backend = super::parakeet::ParakeetBackend::new(model_path)?;
            Ok(Box::new(backend))
        }
        _ => {
            let available = available_backend_names().join(", ");
            bail!(
                "Unknown backend: '{}'. Available backends: {}",
                backend_name,
                available
            )
        }
    }
}
