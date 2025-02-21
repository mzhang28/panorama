use std::path::{Path, PathBuf};

use tempfile::{NamedTempFile, TempDir};

pub fn get_panorama_state_dir() -> PathBuf {
  dirs::state_dir()
    .map(|d| d.join("panorama"))
    .or_else(|| dirs::runtime_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::data_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::home_dir().map(|d| d.join(".panorama")))
    .unwrap()
}
