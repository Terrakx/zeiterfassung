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

/// SHA-256-Hashes (base64) aller Inline-Skripte der index.html für die Content Security Policy.
pub fn inline_script_hashes() -> Vec<String> {
    use sha2::{Digest, Sha256};
    let Some(f) = Assets::get("index.html") else { return Vec::new() };
    let html = String::from_utf8_lossy(&f.data);
    let mut out = Vec::new();
    let mut rest = &html[..];
    while let Some(start) = rest.find("<script") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else { break };
        let tag = &after[..tag_end];
        let body_start = tag_end + 1;
        let Some(close) = after[body_start..].find("</script>") else { break };
        let body = &after[body_start..body_start + close];
        if !tag.contains("src=") && !body.trim().is_empty() {
            out.push(base64_encode(&Sha256::digest(body.as_bytes())));
        }
        rest = &after[body_start + close + 9..];
    }
    out
}

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}
