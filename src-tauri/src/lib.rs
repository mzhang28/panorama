mod schema;

use cozo::{DbInstance, ScriptMutability};
use schema::ensure_schema;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
  format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let dir = dirs::data_dir().unwrap().join("panorama");
  let db_path = dir.join("cozo-db");

  let db = DbInstance::new("rocksdb", db_path, Default::default()).unwrap();
  if let Err(err) = ensure_schema(&db) {
    println!("WTF? {:?}", err);
  }

  tauri::Builder::default()
    .plugin(tauri_plugin_window::init())
    .plugin(tauri_plugin_shell::init())
    .invoke_handler(tauri::generate_handler![greet])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
