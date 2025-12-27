use crate::db::DbClient;
use derivative::Derivative;
use mlua::Lua;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct App {
  pub name: String,
  pub version: String,
  pub lua_entrypoint: String,
  pub frontend_server: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct AppManifest {
  name: String,
  version: String,
  lua_entrypoint: String,
  permissions: Option<String>,
  fields: Option<HashMap<String, Field>>,
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
  db: DbClient,
}

impl AppManager {
  pub fn new(db: DbClient) -> AppManager {
    AppManager {
      apps: Vec::new(),
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

    let frontend_server = manifest.dev.and_then(|d| d.frontend_server);

    let app = App {
      name: manifest.name,
      version: manifest.version,
      lua_entrypoint: manifest.lua_entrypoint,
      frontend_server,
    };
    println!("Successfully loaded app: {:?}", app);

    self.apps.push(app);

    Ok(())
  }
}
