use panorama_backend::{create_context, create_web_server};
use tokio::net::TcpListener;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      tauri::async_runtime::spawn(async {
        let context = create_context().await.unwrap();
        let app = create_web_server(context).await.unwrap();
        let listener = TcpListener::bind("0.0.0.0:10203").await.unwrap();
        axum::serve(listener, app).await
      });

      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
