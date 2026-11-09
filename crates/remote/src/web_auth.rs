//! HTTP session boundary: HttpOnly cookies, per-session CSRF and password-confirmed changes.
use axum::Router;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header, request::Parts};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::{self, Auth, Client, Error, Identity};
use crate::http::{self, Peer, S, State};

const COOKIE: &str = "__Secure-mdm-session";

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/v1/auth/session", get(session))
        .route("/api/v1/auth/setup", post(setup))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/activity", post(activity))
        .route("/api/v1/auth/settings", get(settings).post(save_settings))
        .route("/api/v1/auth/credentials", post(credentials))
        .route("/api/v1/devices", get(devices))
        .route("/api/v1/devices/pair", post(pair_device))
        .route("/api/v1/devices/revoke", post(revoke_device))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/manage", get(config).post(manage))
}

pub(crate) fn failure(e: Error) -> Response {
    let status = match e {
        Error::Unauthorized | Error::InvalidCredentials => StatusCode::UNAUTHORIZED,
        Error::Forbidden => StatusCode::FORBIDDEN,
        Error::Banned(_) | Error::Busy => StatusCode::TOO_MANY_REQUESTS,
        Error::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        Error::AlreadyConfigured => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    };
    let mut r = http::error(status, e.code());
    if let Error::Banned(seconds) = e {
        r.headers_mut().insert(
            header::RETRY_AFTER,
            HeaderValue::from_str(&seconds.to_string()).unwrap(),
        );
    }
    r
}

pub(crate) async fn call<T: Send + 'static>(
    s: &State,
    f: impl FnOnce(&mut Auth) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let inner = s.inner.clone();
    tokio::task::spawn_blocking(move || f(&mut inner.auth.lock().unwrap()))
        .await
        .map_err(|_| Error::Unavailable)?
}

fn cookie(headers: &HeaderMap) -> Option<String> {
    let all = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .filter(|(name, value)| *name == COOKIE && value.len() == 64)
        .map(|(_, v)| v.to_owned())
        .collect::<Vec<_>>();
    (all.len() == 1).then(|| all[0].clone())
}

fn client(a: &Auth, headers: &HeaderMap, peer: Peer) -> Result<Client, Error> {
    let forwarded = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok());
    let has_forwarding = [
        "x-forwarded-for",
        "forwarded",
        "x-real-ip",
        "x-forwarded-host",
    ]
    .iter()
    .any(|h| headers.contains_key(*h));
    Client::resolve(peer.0.ip(), forwarded, has_forwarding, a.policy()?)
}

fn cookie_header(raw: &str, clear: bool) -> Result<HeaderValue, Error> {
    let path = std::env::var("MDM_REMOTE_URL")
        .ok()
        .and_then(|s| url::Url::parse(&s).ok())
        .map(|u| u.path().to_owned())
        .unwrap_or_else(|| "/".into());
    if !path.starts_with('/')
        || !path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-._~%".contains(&b))
    {
        return Err(Error::InvalidPolicy);
    }
    let path = if path.ends_with('/') {
        path
    } else {
        format!("{path}/")
    };
    HeaderValue::from_str(&format!(
        "{COOKIE}={raw}; Path={path}; Secure; HttpOnly; SameSite=Strict; Max-Age={}",
        if clear { 0 } else { 86400 }
    ))
    .map_err(|_| Error::Unavailable)
}

fn with_cookie(body: serde_json::Value, raw: Option<String>) -> Response {
    let mut r = http::json(body.to_string());
    if let Some(raw) = raw {
        match cookie_header(&raw, false) {
            Ok(h) => {
                r.headers_mut().insert(header::SET_COOKIE, h);
            }
            Err(e) => return failure(e),
        }
    }
    r
}

pub(crate) async fn identity(
    parts: &Parts,
    s: &Arc<State>,
    password: bool,
) -> Result<Identity, Error> {
    let peer = parts
        .extensions
        .get::<ConnectInfo<Peer>>()
        .map(|p| p.0)
        .ok_or(Error::InvalidClient)?;
    let headers = parts.headers.clone();
    let raw = cookie(&headers);
    let mutation = !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS);
    call(s, move |a| {
        let c = client(a, &headers, peer)?;
        let id = a.authenticate(raw.as_deref(), c, crate::devices::now())?;
        if password && !id.password {
            return Err(Error::Forbidden);
        }
        if mutation
            && !headers
                .get("x-csrf-token")
                .and_then(|h| h.to_str().ok())
                .is_some_and(|v| auth::same(v, &id.csrf))
        {
            return Err(Error::Forbidden);
        }
        Ok(id)
    })
    .await
}

pub struct Owner(pub Identity);
impl FromRequestParts<Arc<State>> for Owner {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        s: &Arc<State>,
    ) -> Result<Self, Self::Rejection> {
        identity(parts, s, true).await.map(Self).map_err(failure)
    }
}
pub struct Browser(pub Identity);
impl FromRequestParts<Arc<State>> for Browser {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        s: &Arc<State>,
    ) -> Result<Self, Self::Rejection> {
        identity(parts, s, false).await.map(Self).map_err(failure)
    }
}

async fn session(s: S, ConnectInfo(peer): ConnectInfo<Peer>, headers: HeaderMap) -> Response {
    let raw = cookie(&headers);
    match call(&s, move |a| {
        let c = client(a, &headers, peer)?;
        a.session(raw.as_deref(), c, crate::devices::now())
    })
    .await
    {
        Ok((view, cookie)) => with_cookie(serde_json::to_value(view).unwrap(), cookie),
        Err(e) => failure(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    username: String,
    password: String,
    #[serde(default)]
    code: String,
}

async fn setup(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    axum::Json(b): axum::Json<Login>,
) -> Response {
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return failure(Error::Busy);
    };
    let inner = s.inner.clone();
    let result = call(&s, move |a| {
        if a.configured()? {
            return Err(Error::AlreadyConfigured);
        }
        let c = client(a, &headers, peer)?;
        a.banned(c, crate::devices::now())?;
        // Invalid form fields must not consume an otherwise valid one-time setup code.
        Auth::validate_credentials(&b.username, &b.password)?;
        if inner.pairing.lock().unwrap().check(&b.code, c.ip) != crate::pairing::Check::Right {
            return Err(Error::InvalidCredentials);
        }
        let result = a.enroll(&b.username, &b.password, c, crate::devices::now())?;
        inner.pairing.lock().unwrap().done("Web UI owner");
        Ok(result)
    })
    .await;
    match result {
        Ok((raw, id)) => with_cookie(
            serde_json::json!({ "csrf": id.csrf, "owner": true }),
            Some(raw),
        ),
        Err(e) => failure(e),
    }
}

async fn login(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    axum::Json(b): axum::Json<Login>,
) -> Response {
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return failure(Error::Busy);
    };
    match call(&s, move |a| {
        let c = client(a, &headers, peer)?;
        a.login(&b.username, &b.password, c, crate::devices::now())
    })
    .await
    {
        Ok((raw, id)) => with_cookie(
            serde_json::json!({ "csrf": id.csrf, "owner": true }),
            Some(raw),
        ),
        Err(e) => failure(e),
    }
}

async fn logout(s: S, Browser(id): Browser) -> Response {
    let _ = call(&s, move |a| {
        a.logout(&id);
        Ok(())
    })
    .await;
    let mut r = StatusCode::NO_CONTENT.into_response();
    if let Ok(h) = cookie_header("", true) {
        r.headers_mut().insert(header::SET_COOKIE, h);
    }
    r
}
async fn activity(s: S, Browser(id): Browser) -> Response {
    match call(&s, move |a| {
        a.activity(&id, crate::devices::now());
        Ok(())
    })
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => failure(e),
    }
}
async fn settings(s: S, _: Owner) -> Response {
    match call(&s, |a| a.settings()).await {
        Ok(v) => http::json(v.to_string()),
        Err(e) => failure(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyUpdate {
    current_password: String,
    policy: auth::Policy,
    revision: u64,
    #[serde(default)]
    confirm_unsafe: bool,
}

async fn save_settings(
    s: S,
    Owner(id): Owner,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    axum::Json(b): axum::Json<PolicyUpdate>,
) -> Response {
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return failure(Error::Busy);
    };
    match call(&s, move |a| {
        let c = client(a, &headers, peer)?;
        a.confirm(&id, &b.current_password, c, crate::devices::now())?;
        a.save_policy(b.policy, b.confirm_unsafe, b.revision)?;
        a.settings()
    })
    .await
    {
        Ok(v) => http::json(v.to_string()),
        Err(e) => failure(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    current_password: String,
    username: String,
    password: String,
}
async fn credentials(
    s: S,
    Owner(id): Owner,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    axum::Json(b): axum::Json<Credentials>,
) -> Response {
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return failure(Error::Busy);
    };
    match call(&s, move |a| {
        let c = client(a, &headers, peer)?;
        a.confirm(&id, &b.current_password, c, crate::devices::now())?;
        a.credentials(&b.username, &b.password)
    })
    .await
    {
        Ok(()) => {
            let mut r = http::json("{\"ok\":true}".into());
            if let Ok(h) = cookie_header("", true) {
                r.headers_mut().insert(header::SET_COOKIE, h);
            }
            r
        }
        Err(e) => failure(e),
    }
}

async fn devices(s: S, _: Owner) -> Response {
    // Device::Serialize also contains the on-disk token hash. Never send that record directly.
    let devices = s
        .inner
        .devices
        .lock()
        .unwrap()
        .list()
        .into_iter()
        .map(
            |d| serde_json::json!({ "id": d.id, "name": d.name, "added": d.added, "seen": d.seen, "scope": d.scope }),
        )
        .collect::<Vec<_>>();
    http::json(serde_json::json!({ "devices": devices }).to_string())
}
#[derive(Deserialize)]
struct Confirm {
    current_password: String,
    #[serde(default)]
    id: String,
}
async fn pair_device(
    s: S,
    Owner(id): Owner,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    axum::Json(b): axum::Json<Confirm>,
) -> Response {
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return failure(Error::Busy);
    };
    match call(&s, move |a| {
        let c = client(a, &headers, peer)?;
        a.confirm(&id, &b.current_password, c, crate::devices::now())
    })
    .await
    {
        Ok(()) => {
            let code = s.inner.pairing.lock().unwrap().start();
            http::json(
                serde_json::json!({ "code": code, "seconds": crate::pairing::VALID.as_secs() })
                    .to_string(),
            )
        }
        Err(e) => failure(e),
    }
}
async fn revoke_device(
    s: S,
    Owner(id): Owner,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    axum::Json(b): axum::Json<Confirm>,
) -> Response {
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return failure(Error::Busy);
    };
    let device = b.id;
    match call(&s, move |a| {
        let c = client(a, &headers, peer)?;
        a.confirm(&id, &b.current_password, c, crate::devices::now())
    })
    .await
    {
        Ok(()) => {
            s.inner.devices.lock().unwrap().remove(&device);
            http::json("{\"ok\":true}".into())
        }
        Err(e) => failure(e),
    }
}
async fn capabilities(_: Browser) -> Response {
    http::json(serde_json::json!({ "api_version": 1, "downloads": true, "queues": true, "authentication": true, "devices": true, "management": true }).to_string())
}

async fn config(s: S, _: Owner) -> Response {
    http::ask(
        &s,
        mdm_ipc::Command {
            web_owner: true,
            web_request: Some(serde_json::json!({ "op": "config" })),
            ..Default::default()
        },
    )
    .await
}
async fn manage(s: S, _: Owner, axum::Json(value): axum::Json<serde_json::Value>) -> Response {
    http::ask(
        &s,
        mdm_ipc::Command {
            web_owner: true,
            web_request: Some(value),
            ..Default::default()
        },
    )
    .await
}
