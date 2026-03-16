use crate::db::DbClient;
use anyhow::{Result, bail};
use derivative::Derivative;
use mlua::{Lua, LuaSerdeExt, Value as LuaValue};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use surrealdb_types::Value as SurrealValue;

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
        let mut field_type = field.type_.clone();
        if !field_type.starts_with("option<") {
            field_type = format!("option<{}>", field_type);
        }
        if field.type_ == "any" {
            field_type = format!("{} FLEXIBLE", field_type);
        }
        println!("Ensuring field: {} of type {}", field_name, field_type);
        self
          .db
          .ensure_field("nodes", field_name, &field_type)
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
          let result = db.query(&sql, vars).await.unwrap();
          // let result = serde_json::to_value(result).unwrap();
          // let result = json_to_lua(&lua, result).unwrap();
          let mut results = Vec::new();
          for val in result {
            results.push(surreal_to_lua(&lua, val).unwrap());
          }
          let result = lua.create_sequence_from(results);
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

fn surreal_to_lua(lua: &Lua, value: SurrealValue) -> Result<LuaValue> {
  Ok(match value {
    SurrealValue::None => LuaValue::Nil,
    SurrealValue::Null => LuaValue::Nil,
    SurrealValue::Bool(b) => LuaValue::Boolean(b),
    SurrealValue::Number(number) => match number {
      surrealdb_types::Number::Int(n) => LuaValue::Integer(n),
      surrealdb_types::Number::Float(n) => LuaValue::Number(n),
      surrealdb_types::Number::Decimal(decimal) => todo!(),
    },
    SurrealValue::String(s) => lua.create_string(s).map(|s| LuaValue::String(s))?,
    SurrealValue::Bytes(s) => lua.create_string(s.as_ref()).map(|s| LuaValue::String(s))?,
    // SurrealValue::Duration(duration) => LuaValue::String(duration.to_string()),
    // SurrealValue::Datetime(datetime) => LuaValue::String(datetime.to_string()),
    SurrealValue::Uuid(uuid) => lua
      .create_string(uuid.to_string())
      .map(|s| LuaValue::String(s))?,
    // SurrealValue::Geometry(geometry) => LuaValue::String(geometry.to_string()),
    // SurrealValue::Table(table) => LuaValue::String(table.to_string()),
    SurrealValue::RecordId(record_id) => lua
      .create_string(format!("<record:{:?}>", record_id))
      .map(|s| LuaValue::String(s))?,
    // SurrealValue::File(file) => LuaValue::String(file.to_string()),
    // SurrealValue::Range(range) => LuaValue::String(range.to_string()),
    // SurrealValue::Regex(regex) => LuaValue::String(regex.to_string()),
    // SurrealValue::Array(array) => {
    //   LuaValue::Array(array.into_iter().map(|v| surreal_to_lua(lua, v)).collect())
    // }
    SurrealValue::Object(object) => {
      let table = lua.create_table_with_capacity(object.len(), 0)?;
      for (k, v) in object.into_iter() {
        let v2 = surreal_to_lua(lua, v)?;
        table.set(k, v2)?;
      }
      LuaValue::Table(table)
    } // SurrealValue::Set(set) => {
    //   LuaValue::Array(set.into_iter().map(|v| surreal_to_lua(lua, v)).collect())
    // }
    _ => todo!("lol"),
  })
}
