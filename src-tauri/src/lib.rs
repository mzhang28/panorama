use panorama_backend::{create_context, create_web_server};
use tauri::tray::TrayIconBuilder;

#[tauri::command]
fn api_fetch() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_http::init())
    .setup(|app| {
      async {
        let context = create_context().await.unwrap();
        let app = create_web_server(context).await.unwrap();
        // let listener = TcpListener::bind("0.0.0.0:10203").await.unwrap();
        // axum::serve(listener, app).await
      };

      let tray = TrayIconBuilder::new().build(app)?;

      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![api_fetch])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
