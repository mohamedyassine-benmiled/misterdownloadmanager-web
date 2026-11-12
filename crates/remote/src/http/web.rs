//! Fork routes and authorization policy. Upstream HTTP parsing, transport and TLS stay shared.
use super::*;

pub(super) fn routes() -> Router<Arc<State>> {
    Router::new()
        .merge(crate::web_auth::routes())
        .route("/desktop-icons/{name}", get(desktop_icon))
        .route("/api/status", get(capture_status))
        .route("/api/v1/downloads", get(list).post(add))
        .route("/api/v1/actions", post(act))
        .route("/api/v1/queues/action", post(queue))
        .route(
            "/api/v1/torrent",
            post(torrent).layer(DefaultBodyLimit::max(MAX_TORRENT)),
        )
}

pub(super) fn browser_origin_allowed(
    path: &str,
    headers: &HeaderMap,
    host: &str,
    secure: bool,
) -> bool {
    // Extension origins may capture through the legacy API, never administer browser sessions.
    !(path.starts_with("/api/v1/")
        && headers
            .get(header::ORIGIN)
            .and_then(|o| o.to_str().ok())
            .is_some_and(|o| {
                !o.eq_ignore_ascii_case(&format!(
                    "{}://{host}",
                    if secure { "https" } else { "http" }
                ))
            }))
}

pub(super) async fn paired(parts: &mut Parts, state: &Arc<State>) -> Result<String, Box<Response>> {
    let token = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty() && t.len() <= 128);
    let device = token.and_then(|t| state.inner.devices.lock().unwrap().check(t));
    match device {
        Some(d) => {
            if d.scope == crate::devices::Scope::Capture
                && !matches!(
                    parts.uri.path(),
                    "/api/status" | "/api/add" | "/api/torrent" | "/api/forget"
                )
            {
                return Err(Box::new(error(StatusCode::FORBIDDEN, "insufficient-scope")));
            }
            Ok(d.id)
        }
        None if token.is_some() => Err(Box::new(error(StatusCode::UNAUTHORIZED, "not-paired"))),
        None => {
            crate::web_auth::identity(parts, state, false)
                .await
                .map_err(|e| Box::new(crate::web_auth::failure(e)))?;
            Ok(String::new())
        }
    }
}

pub(super) fn capture_scope() -> crate::devices::Scope {
    crate::devices::Scope::Capture
}

async fn capture_status(s: S, _: Paired) -> Response {
    ask(
        &s,
        mdm_ipc::Command {
            query: Some("capture".into()),
            ..Default::default()
        },
    )
    .await
}

// Exact artwork allowlist, never a server filesystem path supplied by the browser.
async fn desktop_icon(axum::extract::Path(name): axum::extract::Path<String>) -> Response {
    macro_rules! icons {
        ($($name:literal),+ $(,)?) => {
            match name.as_str() {
                $(concat!($name, ".png") => Some(include_bytes!(concat!("../../../app/assets/themes/aero/icons/24/", $name, ".png")).as_slice()),)+
                _ => None,
            }
        };
    }
    match icons!(
        "cat-all",
        "cat-folder",
        "cat-compressed",
        "cat-documents",
        "cat-music",
        "cat-programs",
        "cat-video",
        "cat-torrents",
        "cat-unfinished",
        "cat-finished",
        "cat-queues",
        "st-downloading",
        "st-queued",
        "st-paused",
        "st-error",
        "tb-options",
        "tb-scheduler",
        "tb-add-url",
        "tb-add-torrent",
        "tb-resume",
        "tb-stop",
        "tb-delete",
        "tb-delete-completed",
        "tb-start-queue",
        "tb-stop-queue",
    ) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/png")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
