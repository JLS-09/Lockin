//! Where Lockin keeps its files.
//!
//! Both the app and the daemon call these functions, so they always open
//! the same database, whatever the Tauri identifier is.
//! On Linux this gives ~/.config/lockin/ and ~/.local/share/lockin/.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("io.github", "jls_09", "lockin").context("could not find the home directory")
}

/// ~/.config/lockin/
pub fn config_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().to_path_buf())
}

/// ~/.local/share/lockin/
pub fn data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

/// $XDG_RUNTIME_DIR/lockin/ (e.g. /run/user/1000/lockin/), for the daemon's socket.
/// None when the system has no runtime folder.
pub fn runtime_dir() -> Option<PathBuf> {
    project_dirs()
        .ok()?
        .runtime_dir()
        .map(|folder| folder.to_path_buf())
}

/// ~/.local/share/lockin/lockin.db
pub fn database_file() -> Result<PathBuf> {
    Ok(data_dir()?.join("lockin.db"))
}
