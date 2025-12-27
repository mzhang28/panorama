use crate::db::DbClient;
use anyhow::{Result, bail};
use derivative::Derivative;
use mlua::{Lua, LuaSerdeExt, Value as LuaValue};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

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

    // Inject DB
    let db_client = self.db.clone();
    let db_query =
      lua.create_async_function(move |lua, (sql, vars_lua): (String, mlua::Value)| {
        let vars: HashMap<String, serde_json::Value> = lua.from_value(vars_lua).unwrap();
        let db = db_client.clone();
        async move {
          let result = db
            .query(&sql, vars)
            .await
            .map_err(|e| mlua::Error::ExternalError(Arc::new(e)))
            .unwrap();
          let result = serde_json::to_value(result).unwrap();
          let result = json_to_lua(&lua, result).unwrap();
          Ok(result)
        }
      })?;

    let globals = lua.globals();
    let db_table = lua.create_table()?;
    db_table.set("query", db_query)?;
    globals.set("db", db_table)?;

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

  pub async fn call_app_function(
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
        return Err(
          format!(
            "Function '{}' is not exposed by app '{}'",
            func_name, app_name
          )
          .into(),
        );
      }

      let globals = lua.globals();
      let func: mlua::Function = globals.get(func_name)?;
      let req_lua = lua.to_value(&req)?;

      // Use call_async to support async functions in Lua (which might call our async DB)
      let res_lua: mlua::Value = func.call_async(req_lua).await?;

      let res_json: serde_json::Value = lua.from_value(res_lua)?;
      Ok(res_json)
    } else {
      Err(format!("App '{}' runtime not found", app_name).into())
    }
  }
}

fn json_to_lua(lua: &Lua, json: JsonValue) -> Result<LuaValue> {
  match json {
    JsonValue::Null => Ok(LuaValue::Nil),
    JsonValue::Bool(b) => Ok(LuaValue::Boolean(b)),
    JsonValue::Number(n) => {
      if let Some(i) = n.as_i64() {
        Ok(LuaValue::Integer(i))
      } else if let Some(f) = n.as_f64() {
        Ok(LuaValue::Number(f))
      } else {
        bail!("failed to convert")
        // Err(bailmlua::Error::FromLuaConversionError {
        //   from: "number",
        //   to: "mlua::Value",
        //   message: Some("invalid number".to_string()),
        // })
      }
    }
    JsonValue::String(s) => Ok(LuaValue::String(lua.create_string(&s)?)),
    JsonValue::Array(arr) => {
      let table = lua.create_table()?;
      for (i, v) in arr.into_iter().enumerate() {
        table.set(i + 1, json_to_lua(lua, v)?)?;
      }
      Ok(LuaValue::Table(table))
    }
    JsonValue::Object(obj) => {
      let table = lua.create_table()?;
      for (k, v) in obj {
        table.set(k, json_to_lua(lua, v)?)?;
      }
      Ok(LuaValue::Table(table))
    }
  }
}
