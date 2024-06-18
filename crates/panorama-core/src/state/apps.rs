use std::{
  fs::{self, File},
  io::Read,
  path::{Path, PathBuf},
  sync::Arc,
};

use miette::{IntoDiagnostic, Result};
use rune::{
  prepare,
  termcolor::{ColorChoice, StandardStream},
  Context, Diagnostics, Hash, Module, Source, Sources, Vm,
};

use crate::AppState;

impl AppState {
  pub async fn install_apps_from_search_paths(&self) -> Result<()> {
    let search_paths =
      vec![PathBuf::from("/Users/michael/Projects/panorama/apps")];

    let mut found = Vec::new();

    for path in search_paths {
      let read_dir = fs::read_dir(path).into_diagnostic()?;

      for dir_entry in read_dir {
        let dir_entry = dir_entry.into_diagnostic()?;
        let path = dir_entry.path();

        let manifest_path = path.join("manifest.yml");
        if manifest_path.exists() {
          found.push(path);
        }
      }
    }

    for path in found {
      self.install_app_from_path(path).await;
    }

    Ok(())
  }

  async fn install_app_from_path(&self, path: impl AsRef<Path>) -> Result<()> {
    let app_path = path.as_ref();
    let manifest_path = app_path.join("manifest.yml");
    let register_path = app_path.join("register.rn");

    let register_script = {
      let mut file = File::open(register_path).into_diagnostic()?;
      let mut string = String::new();
      file.read_to_string(&mut string).into_diagnostic()?;
      string
    };

    let mut sources = Sources::new();
    sources
      .insert(Source::new("register.rn", register_script).into_diagnostic()?)
      .into_diagnostic()?;

    let mut diagnostics = Diagnostics::new();
    let register_script_unit = prepare(&mut sources)
      .with_diagnostics(&mut diagnostics)
      .build();
    if !diagnostics.is_empty() {
      let mut writer = StandardStream::stderr(ColorChoice::Always);
      diagnostics.emit(&mut writer, &sources).into_diagnostic()?;
    }
    let register_script_unit =
      Arc::new(register_script_unit.into_diagnostic()?);

    let module = Module::new();
    // let mut ctx = Context::new();
    let mut ctx = Context::with_default_modules().into_diagnostic()?;
    ctx.install(module).into_diagnostic()?;

    let rt_ctx = ctx.runtime().into_diagnostic()?;
    let ctx_arc = Arc::new(rt_ctx);
    let mut vm = Vm::new(ctx_arc, register_script_unit);

    let main = Hash::type_hash(["main"]);
    let result = vm
      .execute(main, ())
      .into_diagnostic()?
      .complete()
      .into_result()
      .into_diagnostic()?;
    println!("Executed. {result:?}");

    Ok(())
  }
}
