//! Serves the production frontend build embedded in the server binary.
//!
//! When compiled with `frontend_embedded` (i.e. `frontend/dist/` exists at
//! build time), this module serves the built SPA.  Otherwise the server runs
//! API-only — the frontend is served elsewhere (nginx, Vite dev, etc.).

use axum::{
    body::Body,
    http::{header, StatusCode},
    response::Response,
};

/// rust-embed struct — the folder is relative to the crate root
/// (`crates/panorama-server/`), so `../../frontend/dist` resolves to the
/// Vite build output directory.
#[cfg(frontend_embedded)]
#[derive(rust_embed::RustEmbed)]
#[folder = "../../frontend/dist"]
struct FrontendAssets;

/// Try to serve a static frontend asset.  Returns `None` when the frontend
/// isn't embedded or the requested file isn't found — the caller should
/// fall back to `index.html` (SPA routing).
pub fn try_serve(path: &str) -> Option<Response> {
    #[cfg(frontend_embedded)]
    {
        if let Some(data) = FrontendAssets::get(path) {
            let mime = mime_for(path);
            return Some(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime)
                    .body(Body::from(data.data))
                    .unwrap(),
            );
        }
    }
    None
}

/// SPA fallback — serve `index.html` for any non-file path so client-side
/// routing works.  Returns `None` when the frontend isn't embedded.
pub fn serve_index() -> Option<Response> {
    #[cfg(frontend_embedded)]
    {
        if let Some(data) = FrontendAssets::get("index.html") {
            return Some(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(data.data))
                    .unwrap(),
            );
        }
    }
    None
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "js" | "mjs" => "application/javascript",
        "css" => "text/css",
        "html" => "text/html",
        "json" => "application/json",
        "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
