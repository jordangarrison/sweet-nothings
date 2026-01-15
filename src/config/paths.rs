//! XDG-compliant path resolution

use directories::ProjectDirs;
use std::path::{Path, PathBuf};

const APP_NAME: &str = "sweet-nothings";

/// Get the project directories for XDG paths
fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", APP_NAME)
}

/// Get the configuration file path
/// Returns: $XDG_CONFIG_HOME/sweet-nothings/config.toml
pub fn config_path() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config/sweet-nothings/config.toml")
        })
}

/// Get the data directory
/// Returns: $XDG_DATA_HOME/sweet-nothings
pub fn data_dir() -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".local/share/sweet-nothings")
        })
}

/// Get the models directory
/// Returns: $XDG_DATA_HOME/sweet-nothings/models
pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

/// Get the path to a specific model file
pub fn model_path(model_name: &str, custom_dir: Option<&Path>) -> PathBuf {
    let dir = custom_dir
        .map(PathBuf::from)
        .unwrap_or_else(models_dir);

    // Add ggml- prefix and .bin suffix if needed
    let filename = if model_name.starts_with("ggml-") {
        model_name.to_string()
    } else {
        format!("ggml-{}", model_name)
    };

    let filename = if filename.ends_with(".bin") {
        filename
    } else {
        format!("{}.bin", filename)
    };

    dir.join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path_normalization() {
        let path = model_path("base.en", None);
        assert!(path.to_string_lossy().contains("ggml-base.en.bin"));

        let path = model_path("ggml-base.en", None);
        assert!(path.to_string_lossy().contains("ggml-base.en.bin"));

        let path = model_path("ggml-base.en.bin", None);
        assert!(path.to_string_lossy().contains("ggml-base.en.bin"));
    }
}
