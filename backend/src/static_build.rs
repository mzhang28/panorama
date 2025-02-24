use axum::{
  http::{StatusCode, Uri, header},
  response::{Html, IntoResponse, Response},
  routing::{Router, get},
};
use rust_embed::Embed;

#[cfg(feature = "static-build")]
#[derive(Embed)]
#[folder = "$PANORAMA_STATIC_ASSETS"]
struct Assets;

pub fn create_static_router(app: Router) -> Router {
  Router::new()
    .nest("/api", app)
    .route("/", get(index_handler))
    .route("/{*file}", get(static_handler))
}

async fn index_handler() -> impl IntoResponse {
  static_handler("/index.html".parse::<Uri>().unwrap()).await
}

// We use a wildcard matcher ("/dist/*file") to match against everything
// within our defined assets directory. This is the directory on our Asset
// struct below, where folder = "examples/public/".
async fn static_handler(uri: Uri) -> impl IntoResponse {
  let mut path = uri.path().trim_start_matches('/').to_string();

  StaticFile(path)
}

// Finally, we use a fallback route for anything that didn't match.
async fn not_found() -> Html<&'static str> {
  Html("<h1>404</h1><p>Not Found</p>")
}

pub struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
  T: Into<String>,
{
  fn into_response(self) -> Response {
    let path = self.0.into();
    println!("trying ot find path: {:?}", path);
    println!("All paths: {:?}", Assets::iter().collect::<Vec<_>>());

    match Assets::get(path.as_str()) {
      Some(content) => {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
      }
      None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
  }
}
