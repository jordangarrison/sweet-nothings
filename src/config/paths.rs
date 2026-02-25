//! XDG-compliant path resolution

use directories::ProjectDirs;
use std::path::PathBuf;

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
