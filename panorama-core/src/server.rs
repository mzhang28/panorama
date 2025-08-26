use anyhow::Result;
use axum::{Router, routing::get};

pub async fn server_main() -> Result<()> {
    println!("Server main");
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
