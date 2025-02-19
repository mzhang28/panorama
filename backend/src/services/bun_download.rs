use anyhow::Result;
use cmd_lib::{run_cmd, run_fun};
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

  let res = reqwest::get(url).await?;
  let mut body = res.bytes_stream();

  // TODO:

  Ok(())
}
