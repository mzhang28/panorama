use std::{
  collections::HashMap,
  fs::{self, File},
  io::Read,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::{Context as _, Result};
use serde_yaml::Value;
use wasmtime::Module;

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

    let all_app_data = HashMap::new();
    for path in found {
      let app_data = self.install_app_from_path(path).await;
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
  installer_path: Option<String>,
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
    println!("manifest: {:?}", manifest);

    let register_path = app_path.join("register.rn");

    let register_script = {
      let mut file = File::open(register_path)?;
      let mut string = String::new();
      file.read_to_string(&mut string)?;
      string
    };

    {
      use wasmtime::{Config, Engine};

      let config = Config::new();
      let engine = Engine::new(&config)?;
    }

    // let mut sources = Sources::new();
    // sources
    //   .insert(Source::new("register.rn", register_script).into_diagnostic()?)
    //   .into_diagnostic()?;

    // let mut diagnostics = Diagnostics::new();
    // let register_script_unit = prepare(&mut sources)
    //   .with_diagnostics(&mut diagnostics)
    //   .build();
    // if !diagnostics.is_empty() {
    //   let mut writer = StandardStream::stderr(ColorChoice::Always);
    //   diagnostics.emit(&mut writer, &sources).into_diagnostic()?;
    // }
    // let register_script_unit =
    //   Arc::new(register_script_unit.into_diagnostic()?);

    // let module = Module::new();
    // // let mut ctx = Context::new();
    // let mut ctx = Context::with_default_modules().into_diagnostic()?;
    // ctx.install(module).into_diagnostic()?;

    // let rt_ctx = ctx.runtime().into_diagnostic()?;
    // let ctx_arc = Arc::new(rt_ctx);
    // let mut vm = Vm::new(ctx_arc, register_script_unit);

    // let main = Hash::type_hash(["main"]);
    // let result = vm
    //   .execute(main, ())
    //   .into_diagnostic()?
    //   .complete()
    //   .into_result()
    //   .into_diagnostic()?;
    // println!("Executed. {result:?}");

    Ok(())
  }
}
