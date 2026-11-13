//! The `vthumb` URI scheme (ARCHITECTURE §7.5): seek-preview thumbnails served to `<img>`
//! asynchronously, so image loading never goes through `invoke`.
//!
//! `vthumb://localhost/<media_key>/<step>/<bucket>.jpg` (Windows: `http://vthumb.localhost/…`).
//! 200 `image/jpeg` (immutable) or 404 when previews do not apply or the request was
//! superseded by a newer one.

use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::AppState;
use crate::media::thumbs::{self, Request as ThumbRequest};

pub const SCHEME: &str = "vthumb";

/// `/<key>/<step>/<bucket>.jpg` → (key, step, bucket).
pub fn parse_path(path: &str) -> Option<(String, u32, u32)> {
    let mut parts = path.trim_start_matches('/').split('/');
    let key = percent_decode(parts.next()?)?;
    let step = parts.next()?.parse().ok()?;
    let bucket = parts.next()?.strip_suffix(".jpg")?.parse().ok()?;
    if parts.next().is_some() || key.is_empty() || !(1..=60).contains(&step) {
        return None;
    }
    Some((key, step, bucket))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn respond(responder: UriSchemeResponder, bytes: Option<Vec<u8>>) {
    let response = match bytes {
        Some(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "public, max-age=31536000, immutable")
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Access-Control-Allow-Origin", "*")
            .body(Vec::new()),
    };
    match response {
        Ok(response) => responder.respond(response),
        Err(err) => tracing::debug!("vthumb: could not build a response: {err}"),
    }
}

pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let state = ctx.app_handle().state::<AppState>();
    let Some((key, step, bucket)) = parse_path(request.uri().path()) else {
        return respond(responder, None);
    };
    if let Some(bytes) = state.thumbs.cached(&key, step, bucket) {
        return respond(responder, Some(bytes));
    }
    let config = state.config.get();
    let media = state.player.current_media().filter(|m| m.key == key && thumbs::eligible(m));
    let Some(media) = media.filter(|_| config.interface.seek_preview) else {
        return respond(responder, None);
    };
    state.thumbs.request(ThumbRequest {
        media,
        step,
        bucket,
        hwdec: config.performance.thumbnail_hwdec.clone(),
        reply: Box::new(move |bytes| respond(responder, bytes)),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(parse_path("/f%3A00ab/5/12.jpg"), Some(("f:00ab".into(), 5, 12)));
        assert_eq!(parse_path("/f:00ab/5/12.jpg"), Some(("f:00ab".into(), 5, 12)));
        assert_eq!(parse_path("/f:00ab/5/12.png"), None);
        assert_eq!(parse_path("/f:00ab/0/12.jpg"), None, "step out of range");
        assert_eq!(parse_path("/f:00ab/5/12.jpg/x"), None);
    }
}
