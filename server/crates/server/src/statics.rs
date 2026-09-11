//! Auslieferung der eingebetteten SPA (web/build) mit Fallback auf index.html.

use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../../web/build"]
struct Assets;

pub async fn serve(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path) {
        Some(f) => file_response(path, f.data.into_owned(), path.starts_with("_app/immutable/")),
        None => match Assets::get("index.html") {
            Some(f) => file_response("index.html", f.data.into_owned(), false),
            None => (StatusCode::NOT_FOUND, "not found").into_response(),
        },
    }
}

fn file_response(path: &str, data: Vec<u8>, immutable: bool) -> Response<Body> {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if immutable { "public, max-age=31536000, immutable" } else { "no-cache" };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache)
        .body(Body::from(data))
        .unwrap()
}
