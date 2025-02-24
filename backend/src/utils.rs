use std::{env, path::PathBuf};

pub fn get_panorama_state_dir() -> PathBuf {
  env::var("PANORAMA_DIR")
    .ok()
    .map(|v| PathBuf::from(v))
    .or_else(|| dirs::state_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::runtime_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::data_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::home_dir().map(|d| d.join(".panorama")))
    .unwrap()
}
