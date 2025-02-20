use std::io::Write;

use anyhow::Result;
use cmd_lib::{run_cmd, run_fun};
use tempfile::{NamedTempFile, TempDir};
use tokio::fs::File;
use tokio_util::io::StreamReader;

pub async fn download_bun() -> Result<()> {
  #[rustfmt::skip]
  let platform = run_fun!(uname -ms)?;
  println!("PLATFORM: {platform:?}");
  let target = match platform.as_str() {
    "Darwin x86_64" => "darwin-x64",
    "Darwin arm64" => "darwin-aarch64",
    "Linux aarch64" | "Linux arm64" => "linux-aarch64",
    "MINGW64" => "windows-x64",
    "Linux x86_64" => "linux-x64",
    _ => panic!("unsupported platform"),
  };

  let url = format!("https://github.com/oven-sh/bun/releases/latest/download/bun-{target}.zip");
  println!("url {url}");

  let panorama_state_dir = dirs::state_dir()
    .map(|d| d.join("panorama"))
    .or_else(|| dirs::runtime_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::data_dir().map(|d| d.join("panorama")))
    .or_else(|| dirs::home_dir().map(|d| d.join(".panorama")))
    .unwrap();

  let bin_dir = panorama_state_dir.join("bin");
  std::fs::create_dir_all(&bin_dir)?;
  let bun_path = bin_dir.join("bun");

  if !bun_path.exists() {
    let res = reqwest::get(url).await?;
    let mut tempfile = NamedTempFile::new()?;
    tempfile.write_all(&res.bytes().await?)?;
    let zip_path = tempfile.path();
    let out_dir = TempDir::new()?;
    let out_dir_path = out_dir.path();
    zip_extract::extract(tempfile, out_dir_path, false)?;
    let output_path = out_dir_path.join(format!("bun-{target}")).join("bun");
    std::fs::rename(output_path, bun_path)?;
  }
  info!("Done.");

  Ok(())
}
