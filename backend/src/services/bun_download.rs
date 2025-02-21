use std::io::Write;

use anyhow::Result;
use cmd_lib::{run_cmd, run_fun};
use tempfile::{NamedTempFile, TempDir};

use crate::utils::get_panorama_state_dir;

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

  let panorama_state_dir = get_panorama_state_dir();

  let bin_dir = panorama_state_dir.join("bin");
  std::fs::create_dir_all(&bin_dir)?;
  let bun_path = bin_dir.join("bun");
  debug!(bun_path = ?bun_path.display(), "Checking for bun at path");

  if !bun_path.exists() {
    let url = format!("https://github.com/oven-sh/bun/releases/latest/download/bun-{target}.zip");
    debug!(url = url, "Bun not found, downloading bun from url");

    let res = reqwest::get(url).await?;
    let mut tempfile = NamedTempFile::new()?;
    tempfile.write_all(&res.bytes().await?)?;
    let out_dir = TempDir::new()?;
    let out_dir_path = out_dir.path();
    zip_extract::extract(tempfile, out_dir_path, false)?;
    let output_path = out_dir_path.join(format!("bun-{target}")).join("bun");
    std::fs::rename(output_path, bun_path)?;
    info!("Done.");
  } else {
    debug!("Bun already downloaded!.")
  }

  Ok(())
}
