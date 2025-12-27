use crate::db::DbClient;
use derivative::Derivative;
use mlua::{Lua, LuaSerdeExt};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct App {
  pub name: String,
  pub version: String,
  pub lua_entrypoint: String,
  pub frontend_server: Option<String>,
  pub functions: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct AppManifest {
  name: String,
  version: String,
  lua_entrypoint: String,
  permissions: Option<String>,
  fields: Option<HashMap<String, Field>>,
  functions: Option<Vec<String>>,
  dev: Option<DevBlock>,
}

#[derive(Debug, serde::Deserialize)]
struct Field {
  #[serde(rename = "type")]
  type_: String,
}

#[derive(Debug, serde::Deserialize)]
struct DevBlock {
  frontend_server: Option<String>,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct AppManager {
  apps: Vec<App>,
  #[derivative(Debug = "ignore")]
  runtimes: HashMap<String, Lua>,
  #[derivative(Debug = "ignore")]
  db: DbClient,
}

impl AppManager {
  pub fn new(db: DbClient) -> AppManager {
    AppManager {
      apps: Vec::new(),
      runtimes: HashMap::new(),
      db,
    }
  }

  pub fn get_app(&self, name: &str) -> Option<&App> {
    self.apps.iter().find(|app| app.name == name)
  }

  pub async fn load_app(&mut self, app_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = app_dir.join("manifest.yml");
    let manifest_content = fs::read_to_string(&manifest_path)?;

    let manifest: AppManifest = serde_saphyr::from_str(&manifest_content)?;

    // Handle fields
    if let Some(fields) = &manifest.fields {
      for (field_name, field) in fields {
        let field_type = &field.type_;
        println!("Ensuring field: {} of type {}", field_name, field_type);
        self
          .db
          .ensure_field("nodes", field_name, field_type)
          .await?;
      }
    }

    let lua_path = app_dir.join(&manifest.lua_entrypoint);
    let lua_code = fs::read_to_string(&lua_path)?;

    let lua = Lua::new();
    lua.load(&lua_code).exec()?;
    
    // Store runtime
    self.runtimes.insert(manifest.name.clone(), lua);

    let frontend_server = manifest.dev.and_then(|d| d.frontend_server);
    let functions = manifest.functions.unwrap_or_default();

    let app = App {
      name: manifest.name,
      version: manifest.version,
      lua_entrypoint: manifest.lua_entrypoint,
      frontend_server,
      functions,
    };
    println!("Successfully loaded app: {:?}", app);

    self.apps.push(app);

    Ok(())
  }

  pub fn call_app_function(
    &self,
    app_name: &str,
    func_name: &str,
    req: serde_json::Value,
  ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if let Some(lua) = self.runtimes.get(app_name) {
       // Check if function is exposed? The caller (protocol handler) might want to check app.functions, but we can also just call it.
       // For security, strictly we should check if func_name is in app.functions.
       // Let's assume the caller or this method checks.
       // But wait, the manifest defines what is exposed.
       let app = self.get_app(app_name).ok_or("App metadata not found")?;
       if !app.functions.contains(&func_name.to_string()) {
           return Err(format!("Function '{}' is not exposed by app '{}'", func_name, app_name).into());
       }

       let globals = lua.globals();
       let func: mlua::Function = globals.get(func_name)?;
       let req_lua = lua.to_value(&req)?;
       let res_lua: mlua::Value = func.call(req_lua)?;
       let res_json: serde_json::Value = lua.from_value(res_lua)?;
       Ok(res_json)
    } else {
       Err(format!("App '{}' runtime not found", app_name).into())
    }
  }
}
