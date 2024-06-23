#[macro_use]
pub mod macros;
pub mod internal;

use std::{
  collections::HashMap,
  fs::{self, File},
  io::Read,
  path::{Path, PathBuf},
};

use anyhow::{anyhow, Context as _, Result};
use internal::{WasmtimeInstanceEnv, WasmtimeModule};
use itertools::Itertools;
use wasmtime::{
  AsContext, Caller, Config, Engine, Instance, Linker, Memory, Module, Store,
};
use wasmtime_wasi::WasiCtxBuilder;

use crate::AppState;

pub type AllAppData = HashMap<String, AppData>;

impl AppState {
  pub async fn install_apps_from_search_paths(&self) -> Result<AllAppData> {
    let search_paths = vec![
      PathBuf::from("/Users/michael/Projects/panorama/apps"),
      PathBuf::from("/home/michael/Projects/panorama/apps"),
    ];

    let mut found = Vec::new();

    for path in search_paths {
      if !path.exists() {
        continue;
      }

      let read_dir = fs::read_dir(&path)
        .with_context(|| format!("could not read {}", path.display()))?;

      for dir_entry in read_dir {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();

        let manifest_path = path.join("manifest.yml");
        if manifest_path.exists() {
          found.push(path);
        }
      }
    }

    let mut all_app_data = HashMap::new();
    for path in found {
      let app_data = self.install_app_from_path(&path).await?;
      println!("App data: {:?}", app_data);
      all_app_data.insert(
        path.display().to_string(),
        AppData {
          name: "hello".to_string(),
        },
      );
    }

    Ok(all_app_data)
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppManifest {
  name: String,
  version: Option<String>,
  panorama_version: Option<String>,
  description: Option<String>,
  installer_path: PathBuf,
}

#[derive(Debug)]
pub struct AppData {
  name: String,
}

impl AppState {
  async fn install_app_from_path(&self, path: impl AsRef<Path>) -> Result<()> {
    let app_path = path.as_ref();
    let manifest_path = app_path.join("manifest.yml");
    let manifest: AppManifest = {
      let file = File::open(manifest_path)?;
      serde_yaml::from_reader(file)?
    };
    println!("Manifest: {:?}", manifest);

    let installer_path = app_path.join(manifest.installer_path);

    let installer_program = {
      let mut file = File::open(&installer_path).with_context(|| {
        format!(
          "Could not open installer from path: {}",
          installer_path.display()
        )
      })?;
      let mut buf = Vec::new();
      file.read_to_end(&mut buf)?;
      buf
    };

    println!("Installer program: {} bytes", installer_program.len());

    let config = Config::new();
    let engine = Engine::new(&config)?;
    let module = Module::new(&engine, &installer_program)?;

    let mut linker = Linker::new(&engine);
    WasmtimeModule::link_imports(&mut linker)?;
    let module = linker.instantiate_pre(&module)?;
    let module = WasmtimeModule { module };

    let mut state = WasmtimeInstanceEnv { mem: None };
    let mut store = Store::new(&engine, state);
    println!(
      "Required imports: {:?}",
      module
        .module
        .module()
        .imports()
        .map(|s| s.name())
        .collect_vec()
    );
    let instance = module
      .module
      .instantiate(&mut store)
      .context("Could not instantiate")?;
    let mem = instance
      .get_memory(&mut store, "memory")
      .ok_or_else(|| anyhow!("Fuck!"))?;
    store.data_mut().mem = Some(mem);

    instance.exports(&mut store).for_each(|export| {
      println!("Export: {}", export.name());
    });

    let hello = instance
      .get_typed_func::<(), i32>(&mut store, "install")
      .context("Could not get typed function")?;
    hello.call(&mut store, ()).context("Could not call")?;

    Ok(())
  }
}

fn read_utf_8string<C>(
  c: C,
  mem: &Memory,
  len: usize,
  offset: usize,
) -> Result<String>
where
  C: AsContext,
{
  let mut buffer = vec![0; len];
  mem.read(c, offset, &mut buffer)?;
  let string = String::from_utf8(buffer)?;
  Ok(string)
}
