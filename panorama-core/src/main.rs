use panorama_core::apps::AppManager;
use panorama_core::db::DbClient;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = DbClient::new().await?;
    db.init().await?;
    println!("Connected to database and ensured 'nodes' table.");

    let mut manager = AppManager::new(db);
    let app_path = Path::new("apps/journal");

    if let Err(e) = manager.load_app(app_path).await {
        eprintln!("Error loading app from {:?}: {}", app_path, e);
        // Fallback for when running from within panorama-core directory
        let app_path_fallback = Path::new("../apps/journal");
        if let Err(e2) = manager.load_app(app_path_fallback).await {
             eprintln!("Error loading app from {:?}: {}", app_path_fallback, e2);
        }
    }
    
    Ok(())
}