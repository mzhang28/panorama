use axum::{
    body::Body,
    extract::MatchedPath,
    http::Request,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Attaches a `request_id` tag and a "request.start" breadcrumb to the Sentry
/// scope for every incoming HTTP request.  The request id is generated here so
/// every error / breadcrumb produced during the request lifetime carries it.
pub async fn sentry_request_id(mut req: Request<Body>, next: Next) -> Result<Response, Response> {
    let request_id = Uuid::new_v4().to_string();
    let method = req.method().to_string();

    // Prefer the matched-path (e.g. `/api/nodes/{id}`) over the raw URI so
    // transactions are grouped correctly in Sentry's performance view.
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|mp| mp.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    // Insert the request id into the extensions map so handlers can retrieve it
    // for logging / correlation purposes without depending on Sentry directly.
    req.extensions_mut().insert(RequestId(request_id.clone()));

    sentry::configure_scope(|scope| {
        scope.set_tag("request_id", request_id.clone());
        scope.set_transaction(Some(&path));
    });

    sentry::add_breadcrumb(sentry::Breadcrumb {
        ty: "http".into(),
        category: Some("request".into()),
        message: Some(format!("{} {}", method, path)),
        level: sentry::Level::Info,
        data: {
            let mut m = std::collections::BTreeMap::new();
            m.insert("request_id".into(), serde_json::Value::String(request_id));
            m.insert("method".into(), serde_json::Value::String(method));
            m.insert("path".into(), serde_json::Value::String(path));
            m
        },
        ..Default::default()
    });

    let response = next.run(req).await;

    Ok(response)
}

// -- RequestId extractor ----------------------------------------------------

/// Wraps the per-request UUID so handlers can extract it directly.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Allow handlers to extract the request id via `RequestId` directly.
impl<S> axum::extract::FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = (axum::http::StatusCode, String);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .ok_or_else(|| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "RequestId missing from extensions".into(),
                )
            })
    }
}
