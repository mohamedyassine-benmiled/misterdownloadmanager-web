//! Experimental qBittorrent Web API boundary. Authentication is opt-in; torrent operations
//! are not exposed until their semantics can be implemented without losing data.

#[cfg(test)]
use std::collections::BTreeMap;
use std::net::IpAddr;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Form, FromRequest, Multipart, Query, Request};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
#[cfg(test)]
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::http::{Peer, S, State};

/// Invalid or empty configuration denies every address. Universal networks are never accepted.
#[derive(Clone, Debug, Default)]
pub struct Allowlist(Vec<ipnet::IpNet>);

impl Allowlist {
    pub fn parse(input: &str) -> Self {
        if input.len() > 4096 {
            return Self::default();
        }
        let mut nets = Vec::new();
        for part in input.split(',') {
            let part = part.trim();
            let parsed = part
                .parse::<ipnet::IpNet>()
                .ok()
                .or_else(|| part.parse::<IpAddr>().ok().map(ipnet::IpNet::from));
            let Some(net) = parsed else {
                return Self::default();
            };
            if net.prefix_len() == 0
                || nets.len() >= 64
                || matches!(net, ipnet::IpNet::V6(v) if v.addr().to_ipv4_mapped().is_some())
            {
                return Self::default();
            }
            nets.push(net.trunc());
        }
        Self(nets)
    }

    pub fn allows(&self, ip: IpAddr) -> bool {
        let ip = match ip {
            IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
            _ => ip,
        };
        self.0.iter().any(|net| net.contains(&ip))
    }
}

pub(crate) fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/v2/auth/login", post(login))
        .route("/api/v2/auth/logout", post(logout))
        .route("/api/v2/app/webapiVersion", get(api_version))
        .route("/api/v2/app/version", get(app_version))
        .route("/api/v2/app/preferences", get(preferences))
        .route("/api/v2/torrents/categories", get(categories))
        .route("/api/v2/torrents/createCategory", post(create_category))
        .route("/api/v2/torrents/setCategory", post(set_category))
        .route("/api/v2/torrents/setShareLimits", post(set_share_limits))
        .route("/api/v2/torrents/pause", post(pause))
        .route("/api/v2/torrents/resume", post(resume))
        .route("/api/v2/torrents/stop", post(pause))
        .route("/api/v2/torrents/start", post(resume))
        .route("/api/v2/torrents/delete", post(delete))
        .route("/api/v2/torrents/files", get(files))
        .route("/api/v2/torrents/info", get(info))
        .route("/api/v2/torrents/properties", get(properties))
        .route(
            "/api/v2/torrents/add",
            post(add_torrent).layer(DefaultBodyLimit::max((10 << 20) + (128 << 10))),
        )
}

async fn api_version(s: S, ConnectInfo(peer): ConnectInfo<Peer>, headers: HeaderMap) -> Response {
    match authenticated(&s, peer, headers).await {
        Ok(_) => (StatusCode::OK, "2.8.0").into_response(),
        Err(status) => status.into_response(),
    }
}

async fn app_version(s: S, ConnectInfo(peer): ConnectInfo<Peer>, headers: HeaderMap) -> Response {
    match authenticated(&s, peer, headers).await {
        Ok(_) => (StatusCode::OK, "v4.3.0").into_response(),
        Err(status) => status.into_response(),
    }
}

async fn add_torrent(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers.clone()).await {
        return status.into_response();
    }
    let mut magnet: Option<String> = None;
    let mut torrent: Option<Vec<u8>> = None;
    let mut category: Option<String> = None;
    let mut seen = std::collections::HashSet::new();
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let mut fields = Vec::new();
    if content_type.starts_with("multipart/form-data") {
        let mut multipart = match Multipart::from_request(request, &s).await {
            Ok(value) => value,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        loop {
            let field = match multipart.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => break,
                Err(_) => return StatusCode::BAD_REQUEST.into_response(),
            };
            let Some(name) = field.name().map(str::to_owned) else {
                return StatusCode::BAD_REQUEST.into_response();
            };
            if !seen.insert(name.clone()) {
                return StatusCode::BAD_REQUEST.into_response();
            }
            if name == "torrents" {
                match field.bytes().await {
                    Ok(bytes) if !bytes.is_empty() && bytes.len() <= (10 << 20) => {
                        torrent = Some(bytes.to_vec())
                    }
                    _ => return StatusCode::BAD_REQUEST.into_response(),
                }
            } else {
                let value = match field.text().await {
                    Ok(value) if value.len() <= 8192 => value,
                    _ => return StatusCode::BAD_REQUEST.into_response(),
                };
                fields.push((name, value));
            }
        }
    } else if content_type.starts_with("application/x-www-form-urlencoded") {
        let body = match axum::body::to_bytes(request.into_body(), 64 << 10).await {
            Ok(body) => body,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        for (name, value) in url::form_urlencoded::parse(&body) {
            if name.len() > 128 || value.len() > 8192 || !seen.insert(name.to_string()) {
                return StatusCode::BAD_REQUEST.into_response();
            }
            fields.push((name.into_owned(), value.into_owned()));
        }
    } else {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    for (name, value) in fields {
        match name.as_str() {
            "urls" => {
                let url = value.trim();
                if !url.starts_with("magnet:?") || url.lines().count() != 1 {
                    return StatusCode::BAD_REQUEST.into_response();
                }
                magnet = Some(url.to_owned());
            }
            "category" => {
                if !value.is_empty() && !valid_category(&value) {
                    return StatusCode::BAD_REQUEST.into_response();
                }
                category = Some(value);
            }
            "autoTMM"
                if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") => {}
            "paused" | "stopped" | "skip_checking" | "sequentialDownload"
            | "firstLastPiecePrio"
                if value.eq_ignore_ascii_case("false") => {}
            "savepath" | "downloadPath" | "rename" | "tags" if value.is_empty() => {}
            // Do not silently discard an option that changes where or how a torrent is added.
            _ => return StatusCode::NOT_IMPLEMENTED.into_response(),
        }
    }
    if magnet.is_some() == torrent.is_some() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || sink.ask(mdm_ipc::Command {
        remote: true, web_owner: true,
        web_request: Some(serde_json::json!({"op": "qbittorrent.add", "magnet": magnet, "torrent_file": torrent, "category": category, "start": true})),
        ..Default::default()
    }, Duration::from_secs(5))).await.ok().flatten()
        .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer).ok());
    match result {
        Some(value)
            if value["pending"] == true
                && value["request_id"]
                    .as_str()
                    .is_some_and(|id| !id.is_empty()) =>
        {
            (StatusCode::OK, "Ok.").into_response()
        }
        Some(value) if value["pending"] == false && value["id"].as_u64().is_some() => {
            (StatusCode::OK, "Ok.").into_response()
        }
        Some(value) if value["code"] == "unknown-category" => StatusCode::CONFLICT.into_response(),
        Some(value) if value["code"] == "invalid-torrent-source" => {
            StatusCode::BAD_REQUEST.into_response()
        }
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

// Legacy experimental records remain on disk for a future explicit migration to MDM categories.
#[cfg(test)]
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Category {
    name: String,
    #[serde(rename = "savePath")]
    save_path: String,
    /// Private folder component; never sent as part of the qBittorrent category response.
    #[serde(default)]
    folder: Option<String>,
}

#[cfg(test)]
pub(crate) struct Categories {
    path: PathBuf,
    entries: Option<BTreeMap<String, Category>>,
}

#[cfg(test)]
impl Categories {
    pub(crate) fn load(path: PathBuf) -> Self {
        let entries = match std::fs::read(&path) {
            Ok(data) if data.len() <= 1024 * 1024 => {
                serde_json::from_slice::<BTreeMap<String, Category>>(&data)
                    .ok()
                    .filter(|map| {
                        map.len() <= 1024
                            && map.iter().all(|(k, v)| {
                                valid_category(k)
                                    && k == &v.name
                                    && v.save_path.is_empty()
                                    && v.folder
                                        .as_ref()
                                        .is_none_or(|f| Some(f.clone()) == category_folder_name(k))
                            })
                    })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(BTreeMap::new()),
            _ => None,
        };
        Self { path, entries }
    }

    #[cfg(test)]
    fn create(&mut self, name: &str, save_path: &str) -> Result<(), StatusCode> {
        if !valid_category(name) || !save_path.is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
        let mut entries = self
            .entries
            .clone()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        if entries.contains_key(name) {
            return Err(StatusCode::CONFLICT);
        }
        if entries.len() >= 1024 {
            return Err(StatusCode::INSUFFICIENT_STORAGE);
        }
        entries.insert(
            name.to_owned(),
            Category {
                name: name.into(),
                save_path: String::new(),
                folder: category_folder_name(name),
            },
        );
        let bytes = serde_json::to_vec(&entries).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let parent = self.path.parent().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        mdm_ipc::private_dir(parent)
            .and_then(|_| mdm_ipc::private_write(&self.path, &bytes))
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.entries = Some(entries);
        Ok(())
    }

    /// Read the stored mapping, deriving it for category records written by earlier builds.
    #[cfg(test)]
    pub(crate) fn folder_for(&self, name: &str) -> Result<Option<String>, StatusCode> {
        let entries = self
            .entries
            .as_ref()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        Ok(entries.get(name).and_then(|category| {
            category
                .folder
                .clone()
                .or_else(|| category_folder_name(name))
        }))
    }
}

pub fn valid_category(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | '|'))
        && name
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

// Only translate the application's actual category configuration, never a parallel registry.
fn category_response(value: serde_json::Value) -> Option<serde_json::Value> {
    let mut result = serde_json::Map::new();
    for category in value.as_array()? {
        let name = category.get("name")?.as_str()?;
        let folder = category.get("folder")?.as_str()?;
        if !valid_category(name) || folder.is_empty() || result.contains_key(name) {
            return None;
        }
        result.insert(
            name.to_owned(),
            serde_json::json!({ "name": name, "savePath": folder }),
        );
    }
    Some(serde_json::Value::Object(result))
}

#[derive(Deserialize)]
struct CreateCategoryForm {
    category: String,
    #[serde(rename = "savePath", default)]
    save_path: String,
}

async fn create_category(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<CreateCategoryForm>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    if !valid_category(&form.category)
        || form.category != form.category.trim()
        || !form.save_path.is_empty()
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || sink.ask(mdm_ipc::Command {
        remote: true, web_owner: true,
        web_request: Some(serde_json::json!({"op": "qbittorrent.create_category", "category": form.category})),
        ..Default::default()
    }, Duration::from_secs(5))).await.ok().flatten()
        .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer).ok());
    match result {
        Some(value)
            if value
                .get("categories")
                .and_then(serde_json::Value::as_array)
                .is_some() =>
        {
            StatusCode::OK.into_response()
        }
        Some(value) if value["code"] == "duplicate-category" => {
            StatusCode::CONFLICT.into_response()
        }
        Some(value) if value["code"] == "invalid-category-name" => {
            StatusCode::BAD_REQUEST.into_response()
        }
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn categories(s: S, ConnectInfo(peer): ConnectInfo<Peer>, headers: HeaderMap) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || {
        sink.ask(
            mdm_ipc::Command {
                remote: true,
                web_owner: true,
                web_request: Some(serde_json::json!({ "op": "qbittorrent.categories" })),
                ..Default::default()
            },
            Duration::from_secs(5),
        )
    })
    .await
    .ok()
    .flatten()
    .and_then(|answer| serde_json::from_str(&answer).ok())
    .and_then(category_response);
    match result {
        Some(value) => axum::Json(value).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn preferences(s: S, ConnectInfo(peer): ConnectInfo<Peer>, headers: HeaderMap) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || {
        sink.ask(
            mdm_ipc::Command {
                remote: true,
                web_owner: true,
                web_request: Some(serde_json::json!({"op": "qbittorrent.preferences"})),
                ..Default::default()
            },
            Duration::from_secs(5),
        )
    })
    .await
    .ok()
    .flatten()
    .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer).ok())
    .and_then(|answer| {
        let path = answer.get("save_path")?.as_str()?;
        (!path.is_empty()).then_some(answer)
    });
    match result {
        Some(value) => axum::Json(value).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(Deserialize)]
struct InfoQuery {
    hashes: Option<String>,
    category: Option<String>,
    filter: Option<String>,
    sort: Option<String>,
    reverse: Option<bool>,
    limit: Option<usize>,
    offset: Option<usize>,
}

fn info_response(value: serde_json::Value) -> Option<serde_json::Value> {
    let mut result = Vec::new();
    for item in value.as_array()? {
        let size = item.get("size")?.as_u64()?;
        let downloaded = item.get("downloaded")?.as_u64()?;
        if downloaded > size {
            return None;
        }
        let uploaded = item.get("uploaded")?.as_u64()?;
        let speed = item.get("download_speed")?.as_u64()?;
        let live = item.get("live")?.as_bool()?;
        let seeding = item.get("seeding")?.as_bool()?;
        let status = item.get("status")?.as_str()?;
        let state = match (status, live) {
            ("Queued", _) => "queuedDL",
            ("Downloading", true) => "downloading",
            ("Downloading", false) | ("Stopped", _) => "pausedDL",
            ("Complete", _) if seeding => "uploading",
            ("Complete", _) => "pausedUP",
            ("Error", _) => "error",
            _ => return None,
        };
        let paths = item.get("paths")?;
        let left = size - downloaded;
        let eta = if speed == 0 {
            8_640_000
        } else {
            (left / speed).min(8_640_000)
        };
        result.push(serde_json::json!({
            "hash": item.get("hash")?.as_str()?, "name": item.get("name")?.as_str()?,
            "category": item.get("category_key")?.as_str()?, "state": state,
            "size": size, "total_size": size, "downloaded": downloaded, "uploaded": uploaded,
            "amount_left": left, "progress": if size == 0 { 1.0 } else { downloaded as f64 / size as f64 },
            "ratio": if downloaded == 0 { 0.0 } else { uploaded as f64 / downloaded as f64 },
            "ratio_limit": item.get("ratio_limit")?.as_f64()?,
            "seeding_time_limit": item.get("seeding_time_limit")?.as_i64()?,
            "inactive_seeding_time_limit": -1,
            "dlspeed": speed, "upspeed": item.get("upload_speed")?.as_u64()?, "eta": eta,
            "seeding_time": item.get("seeded_seconds")?.as_u64()?,
            "save_path": paths.get("save_path")?.as_str()?,
            "root_path": paths.get("root_path")?.as_str()?,
            "content_path": paths.get("content_path")?.as_str()?
        }));
    }
    Some(serde_json::Value::Array(result))
}

fn apply_info_query(value: &mut serde_json::Value, query: &InfoQuery) -> Result<(), StatusCode> {
    let items = value
        .as_array_mut()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    match query.filter.as_deref().unwrap_or("all") {
        "all" => {}
        "downloading" => items.retain(|item| item["progress"].as_f64().is_some_and(|p| p < 1.0)),
        "completed" => items.retain(|item| item["progress"].as_f64().is_some_and(|p| p >= 1.0)),
        "paused" | "stopped" => items.retain(|item| {
            item["state"]
                .as_str()
                .is_some_and(|s| s.starts_with("paused"))
        }),
        "resumed" | "running" => items.retain(|item| {
            item["state"]
                .as_str()
                .is_some_and(|s| !s.starts_with("paused"))
        }),
        "active" => items.retain(|item| {
            item["dlspeed"].as_u64().unwrap_or(0) > 0 || item["upspeed"].as_u64().unwrap_or(0) > 0
        }),
        "inactive" => items.retain(|item| {
            item["dlspeed"].as_u64().unwrap_or(0) == 0 && item["upspeed"].as_u64().unwrap_or(0) == 0
        }),
        "errored" => items.retain(|item| item["state"] == "error"),
        _ => return Err(StatusCode::NOT_IMPLEMENTED),
    }
    if let Some(sort) = query.sort.as_deref() {
        if !matches!(
            sort,
            "name"
                | "hash"
                | "category"
                | "state"
                | "size"
                | "progress"
                | "ratio"
                | "dlspeed"
                | "upspeed"
                | "eta"
        ) {
            return Err(StatusCode::NOT_IMPLEMENTED);
        }
        items.sort_by(|a, b| {
            let (left, right) = (&a[sort], &b[sort]);
            if let (Some(left), Some(right)) = (left.as_str(), right.as_str()) {
                left.cmp(right)
            } else {
                left.as_f64()
                    .and_then(|left| right.as_f64().and_then(|right| left.partial_cmp(&right)))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    }
    if query.reverse.unwrap_or(false) {
        items.reverse();
    }
    let offset = query.offset.unwrap_or(0).min(items.len());
    items.drain(..offset);
    if let Some(limit) = query.limit.filter(|limit| *limit > 0) {
        items.truncate(limit);
    }
    Ok(())
}

async fn info(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Query(query): Query<InfoQuery>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    let hashes = query.hashes.clone().unwrap_or_else(|| "all".to_owned());
    if parse_hashes(&hashes).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let category = query.category.clone();
    let result = tokio::task::spawn_blocking(move || sink.ask(mdm_ipc::Command {
        remote: true, web_owner: true,
        web_request: Some(serde_json::json!({"op": "qbittorrent.info", "hashes": hashes, "category": category})),
        ..Default::default()
    }, Duration::from_secs(5))).await.ok().flatten()
        .and_then(|answer| serde_json::from_str(&answer).ok()).and_then(info_response);
    match result {
        Some(mut value) => match apply_info_query(&mut value, &query) {
            Ok(()) => axum::Json(value).into_response(),
            Err(status) => status.into_response(),
        },
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

// The properties endpoint is derived from the same MDM facts as the torrent list; in particular,
// its save path is the torrent's real path, not its current category's configured folder.
fn properties_response(value: serde_json::Value) -> Option<serde_json::Value> {
    let mut entries = info_response(value)?.as_array()?.clone();
    if entries.len() != 1 {
        return None;
    }
    let item = entries.pop()?;
    Some(serde_json::json!({
        "save_path": item.get("save_path")?.as_str()?,
        "total_size": item.get("total_size")?.as_u64()?,
        "total_downloaded": item.get("downloaded")?.as_u64()?,
        "total_uploaded": item.get("uploaded")?.as_u64()?,
        "dl_speed": item.get("dlspeed")?.as_u64()?,
        "up_speed": item.get("upspeed")?.as_u64()?,
        "seeding_time": item.get("seeding_time")?.as_u64()?,
        "share_ratio": item.get("ratio")?.as_f64()?
    }))
}

async fn properties(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Query(query): Query<FileQuery>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    if !matches!(parse_hashes(&query.hash), Ok(TorrentSelection::Hashes(ref hashes)) if hashes.len() == 1)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || {
        sink.ask(
            mdm_ipc::Command {
                remote: true,
                web_owner: true,
                web_request: Some(
                    serde_json::json!({"op": "qbittorrent.info", "hashes": query.hash}),
                ),
                ..Default::default()
            },
            Duration::from_secs(5),
        )
    })
    .await
    .ok()
    .flatten()
    .and_then(|answer| serde_json::from_str(&answer).ok())
    .and_then(properties_response);
    match result {
        Some(value) => axum::Json(value).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(Deserialize)]
struct FileQuery {
    hash: String,
}

/// Convert existing MDM torrent details to qBittorrent's file objects. A missing live counter
/// falls back to zero, or to complete for a selected file in a completed torrent.
fn files_response(value: serde_json::Value) -> Option<serde_json::Value> {
    let details = value.get("details")?;
    if details.get("metadata_available")?.as_bool()? != true
        || details.get("files_truncated")?.as_bool()?
    {
        return None;
    }
    let root = value.get("paths")?.get("root_path")?.as_str()?;
    let prefix = if root.is_empty() {
        None
    } else {
        Some(std::path::Path::new(root).file_name()?.to_str()?)
    };
    let complete = value
        .get("complete")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut output = Vec::new();
    for file in details.get("files")?.as_array()? {
        let index = file.get("index")?.as_u64()?;
        let size = file.get("size")?.as_u64()?;
        let selected = file.get("selected")?.as_bool()?;
        // The qBittorrent schema requires a number even when MDM has no per-file counter.
        let done = file
            .get("downloaded")?
            .as_u64()
            .unwrap_or(if complete && selected { size } else { 0 });
        let path = file.get("path")?.as_str()?;
        let path = std::path::Path::new(path);
        let mut parts = Vec::new();
        if let Some(prefix) = prefix {
            if prefix.contains(['/', '\\']) {
                return None;
            }
            parts.push(prefix.to_owned());
        }
        for component in path.components() {
            let std::path::Component::Normal(part) = component else {
                return None;
            };
            let part = part.to_str()?;
            if part.contains(['/', '\\']) {
                return None;
            }
            parts.push(part.to_owned());
        }
        if parts.is_empty() || done > size {
            return None;
        }
        let progress = if size == 0 {
            1.0
        } else {
            done as f64 / size as f64
        };
        output.push(
            serde_json::json!({ "index": index, "name": parts.join("/"), "size": size,
            "progress": progress, "priority": if selected { 1 } else { 0 } }),
        );
    }
    Some(serde_json::Value::Array(output))
}

async fn files(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Query(query): Query<FileQuery>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    if !matches!(parse_hashes(&query.hash), Ok(TorrentSelection::Hashes(ref hashes)) if hashes.len() == 1)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || {
        sink.ask(
            mdm_ipc::Command {
                remote: true,
                web_owner: true,
                web_request: Some(
                    serde_json::json!({ "op": "qbittorrent.files", "hash": query.hash }),
                ),
                ..Default::default()
            },
            Duration::from_secs(5),
        )
    })
    .await
    .ok()
    .flatten()
    .and_then(|answer| serde_json::from_str(&answer).ok())
    .and_then(files_response);
    match result {
        Some(value) => axum::Json(value).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(Deserialize)]
struct ControlForm {
    hashes: String,
}

#[derive(Deserialize)]
struct SetCategoryForm {
    hashes: String,
    category: String,
}

#[derive(Deserialize)]
struct ShareLimitsForm {
    hashes: String,
    #[serde(rename = "ratioLimit")]
    ratio: f64,
    #[serde(rename = "seedingTimeLimit")]
    minutes: i64,
    #[serde(rename = "inactiveSeedingTimeLimit", default = "global_limit")]
    inactive_minutes: i64,
}

fn global_limit() -> i64 {
    -2
}

async fn set_share_limits(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<ShareLimitsForm>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    if parse_hashes(&form.hashes).is_err()
        || !form.ratio.is_finite()
        || !(form.ratio == -2.0 || form.ratio == -1.0 || (0.0..=1_000_000.0).contains(&form.ratio))
        || !(-2..=10_000_000).contains(&form.minutes)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    // MDM has no inactive-seeding timer; accepting a specific value would silently ignore it.
    if form.inactive_minutes != -2 {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || sink.ask(mdm_ipc::Command {
        remote: true, web_owner: true,
        web_request: Some(serde_json::json!({"op": "qbittorrent.share_limits", "hashes": form.hashes, "ratio": form.ratio, "minutes": form.minutes})),
        ..Default::default()
    }, Duration::from_secs(5))).await.ok().flatten()
        .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer).ok());
    match result {
        Some(value) if value["ok"] == true => StatusCode::OK.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn set_category(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<SetCategoryForm>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    if parse_hashes(&form.hashes).is_err() || !valid_category(&form.category) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || sink.ask(mdm_ipc::Command {
        remote: true, web_owner: true,
        web_request: Some(serde_json::json!({ "op": "qbittorrent.set_category", "hashes": form.hashes, "category": form.category })),
        ..Default::default()
    }, Duration::from_secs(5))).await.ok().flatten()
        .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer).ok());
    match result {
        Some(value) if value["ok"] == true => StatusCode::OK.into_response(),
        Some(value) if value["code"] == "unknown-category" => StatusCode::CONFLICT.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn control(
    s: S,
    peer: Peer,
    headers: HeaderMap,
    form: ControlForm,
    action: &'static str,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers).await {
        return status.into_response();
    }
    if parse_hashes(&form.hashes).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let sink = s.inner.sink.clone();
    let result = tokio::task::spawn_blocking(move || {
        sink.ask(mdm_ipc::Command {
            remote: true,
            web_owner: true,
            web_request: Some(serde_json::json!({ "op": "qbittorrent.control", "action": action, "hashes": form.hashes })),
            ..Default::default()
        }, Duration::from_secs(5))
    }).await.ok().flatten()
        .and_then(|answer| serde_json::from_str::<mdm_ipc::api::ActAnswer>(&answer).ok());
    match result {
        Some(result)
            if result.skipped.iter().all(|skip| {
                matches!(
                    (action, skip.why.as_str()),
                    ("pause", "not running") | ("resume", "already running")
                )
            }) =>
        {
            StatusCode::OK.into_response()
        }
        Some(_) => StatusCode::CONFLICT.into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

async fn pause(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<ControlForm>,
) -> Response {
    control(s, peer, headers, form, "pause").await
}

async fn resume(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<ControlForm>,
) -> Response {
    control(s, peer, headers, form, "resume").await
}

async fn delete(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<DeleteForm>,
) -> Response {
    if let Err(status) = authenticated(&s, peer, headers.clone()).await {
        return status.into_response();
    }
    if parse_hashes(&form.hashes).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    // MDM stops asynchronously and only removes stopped torrents. Wait for its normal stop
    // operation before asking it to remove the entries; never bypass its running-item guard.
    let stopped = control(
        s.clone(),
        peer,
        headers.clone(),
        ControlForm {
            hashes: form.hashes.clone(),
        },
        "pause",
    )
    .await;
    if stopped.status() != StatusCode::OK {
        return stopped;
    }
    let mut ready = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    for _ in 0..40 {
        if std::time::Instant::now() >= deadline {
            break;
        }
        let sink = s.inner.sink.clone();
        let hashes = form.hashes.clone();
        let state = tokio::task::spawn_blocking(move || {
            sink.ask(
                mdm_ipc::Command {
                    remote: true,
                    web_owner: true,
                    web_request: Some(
                        serde_json::json!({ "op": "qbittorrent.removal_status", "hashes": hashes }),
                    ),
                    ..Default::default()
                },
                Duration::from_secs(5),
            )
        })
        .await
        .ok()
        .flatten()
        .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer).ok());
        match state.and_then(|value| value.get("ready").and_then(serde_json::Value::as_bool)) {
            Some(true) => {
                ready = true;
                break;
            }
            Some(false) => tokio::time::sleep(Duration::from_millis(125)).await,
            None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    }
    if !ready {
        return StatusCode::CONFLICT.into_response();
    }
    let action = if form.delete_files {
        "delete"
    } else {
        "remove"
    };
    control(
        s,
        peer,
        headers,
        ControlForm {
            hashes: form.hashes,
        },
        action,
    )
    .await
}

/// Deterministic, portable folder component for a category label. The label itself is never
/// interpreted as a path: slashes in a nested category remain part of the identity. The digest
/// distinguishes case, punctuation, Unicode and labels with the same readable prefix, including
/// on case-insensitive filesystems. The caller must still enforce the configured download root
/// and reject symlinks while creating and using the directory.
pub fn category_folder_name(category: &str) -> Option<String> {
    if !valid_category(category) {
        return None;
    }
    let readable: String = category
        .bytes()
        .filter_map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' => {
                Some((b as char).to_ascii_lowercase())
            }
            b'/' | b' ' => Some('-'),
            _ => None,
        })
        .take(48)
        .collect();
    let readable = readable.trim_matches('-');
    let readable = if readable.is_empty() {
        "category"
    } else {
        readable
    };
    Some(format!(
        "cat-{readable}-{}",
        hex::encode(Sha256::digest(category.as_bytes()))
    ))
}

fn sid(headers: &HeaderMap) -> Option<String> {
    let values: Vec<_> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .filter(|(name, _)| *name == "SID")
        .map(|(_, value)| value)
        .collect();
    if values.len() == 1
        && values[0].len() == 64
        && values[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        Some(values[0].to_owned())
    } else {
        None
    }
}

async fn authenticated(
    s: &State,
    peer: Peer,
    headers: HeaderMap,
) -> Result<Option<crate::auth::Identity>, StatusCode> {
    if !s.qbittorrent {
        return Err(StatusCode::NOT_FOUND);
    }
    let raw = match sid(&headers) {
        Some(raw) => raw,
        None if s.qbittorrent_auth_bypass && !headers.contains_key(header::COOKIE) => {
            return Ok(None);
        }
        None => return Err(StatusCode::FORBIDDEN),
    };
    crate::web_auth::call(s, move |auth| {
        let client = crate::web_auth::client(auth, &headers, peer)?;
        let now = crate::devices::now();
        let identity = auth.authenticate(Some(&raw), client, now)?;
        if !identity.password {
            return Err(crate::auth::Error::Forbidden);
        }
        auth.activity(&identity, now);
        Ok(Some(identity))
    })
    .await
    .map_err(|error| match error {
        crate::auth::Error::Unauthorized | crate::auth::Error::Forbidden => StatusCode::FORBIDDEN,
        crate::auth::Error::Banned(_) | crate::auth::Error::Busy => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    })
}

async fn logout(s: S, ConnectInfo(peer): ConnectInfo<Peer>, headers: HeaderMap) -> Response {
    let identity = match authenticated(&s, peer, headers).await {
        Ok(identity) => identity,
        Err(status) => return status.into_response(),
    };
    if let Some(identity) = identity {
        if crate::web_auth::call(&s, move |auth| {
            auth.logout(&identity);
            Ok(())
        })
        .await
        .is_err()
        {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    }
    let secure = if s.secure { "; Secure" } else { "" };
    (
        StatusCode::OK,
        [(
            header::SET_COOKIE,
            format!("SID=; Path=/api/v2/; HttpOnly; SameSite=Strict; Max-Age=0{secure}"),
        )],
    )
        .into_response()
}

#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}

/// Use the existing owner's password verifier, ban policy and bounded session store, but put the
/// resulting token in a separate qBittorrent cookie. No authentication bypass is allowed here.
async fn login(
    s: S,
    ConnectInfo(peer): ConnectInfo<Peer>,
    headers: HeaderMap,
    Form(form): Form<Login>,
) -> Response {
    if !s.qbittorrent {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(_permit) = s.inner.auth_jobs.clone().try_acquire_owned() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let result = crate::web_auth::call(&s, move |auth| {
        let client = crate::web_auth::client(auth, &headers, peer)?;
        auth.login(
            &form.username,
            &form.password,
            client,
            crate::devices::now(),
        )
    })
    .await;
    match result {
        Ok((token, _)) => {
            let secure = if s.secure { "; Secure" } else { "" };
            let cookie = format!("SID={token}; Path=/api/v2/; HttpOnly; SameSite=Strict{secure}");
            (StatusCode::OK, [(header::SET_COOKIE, cookie)], "Ok.").into_response()
        }
        Err(crate::auth::Error::Banned(_)) | Err(crate::auth::Error::Busy) => {
            StatusCode::TOO_MANY_REQUESTS.into_response()
        }
        Err(crate::auth::Error::InvalidCredentials) | Err(crate::auth::Error::Unauthorized) => {
            (StatusCode::OK, "Fails.").into_response()
        }
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// qBittorrent identifies torrents by their v1 info hashes, separated by `|`, or by `all`.
#[derive(Debug, PartialEq, Eq)]
pub enum TorrentSelection {
    All,
    Hashes(Vec<String>),
}

/// Bound the amount of work a single request can ask the application to perform.
pub const MAX_HASHES: usize = 1000;

pub fn parse_hashes(value: &str) -> Result<TorrentSelection, &'static str> {
    if value == "all" {
        return Ok(TorrentSelection::All);
    }
    let mut hashes = Vec::new();
    for hash in value.split('|') {
        if hashes.len() >= MAX_HASHES
            || hash.len() != 40
            || !hash.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("invalid-hashes");
        }
        hashes.push(hash.to_ascii_lowercase());
    }
    Ok(TorrentSelection::Hashes(hashes))
}

/// Translate MDM's existing asynchronous admission answers without treating a queued metadata
/// lookup or a request for permission to reuse existing files as a successfully added torrent.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum Admission {
    Pending(String),
    Added,
    ConfirmationRequired,
    Failed,
}

#[cfg(test)]
fn admission_started(value: &serde_json::Value) -> Admission {
    if value.get("error").is_some() || value.get("code").is_some() {
        return Admission::Failed;
    }
    match value.get("pending").and_then(serde_json::Value::as_bool) {
        Some(true) => value
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .map(|id| Admission::Pending(id.to_owned()))
            .unwrap_or(Admission::Failed),
        Some(false)
            if value
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .is_some() =>
        {
            Admission::Added
        }
        _ => Admission::Failed,
    }
}

#[cfg(test)]
fn admission_result(value: &serde_json::Value) -> Admission {
    match value.get("state").and_then(serde_json::Value::as_str) {
        Some("pending") => Admission::Pending(String::new()),
        Some("added")
            if value
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .is_some() =>
        {
            Admission::Added
        }
        Some("confirmation") => Admission::ConfirmationRequired,
        _ => Admission::Failed,
    }
}

/// Form fields used by `/api/v2/torrents/delete`. Missing `deleteFiles` keeps data.
#[derive(Debug, Deserialize)]
pub struct DeleteForm {
    pub hashes: String,
    #[serde(rename = "deleteFiles", default)]
    pub delete_files: bool,
}

impl DeleteForm {
    pub fn selection(&self) -> Result<TorrentSelection, &'static str> {
        parse_hashes(&self.hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torrent_info_filter_sort_and_pagination() {
        let mut items = serde_json::json!([
            {"name":"z", "progress":0.5, "state":"downloading", "dlspeed":10, "upspeed":0},
            {"name":"a", "progress":1.0, "state":"pausedUP", "dlspeed":0, "upspeed":0},
            {"name":"b", "progress":1.0, "state":"uploading", "dlspeed":0, "upspeed":2}
        ]);
        let query = InfoQuery {
            hashes: None,
            category: None,
            filter: Some("completed".into()),
            sort: Some("name".into()),
            reverse: Some(true),
            limit: Some(1),
            offset: Some(0),
        };
        assert_eq!(apply_info_query(&mut items, &query), Ok(()));
        assert_eq!(items.as_array().unwrap().len(), 1);
        assert_eq!(items[0]["name"], "b");
    }

    #[test]
    fn admission_does_not_report_pending_or_confirmation_as_success() {
        assert_eq!(
            admission_started(&serde_json::json!({"pending": true, "request_id": "abc"})),
            Admission::Pending("abc".into())
        );
        assert_eq!(
            admission_started(&serde_json::json!({"pending": false, "id": 42, "duplicate": true})),
            Admission::Added
        );
        assert_eq!(
            admission_started(&serde_json::json!({"pending": true})),
            Admission::Failed
        );
        assert_eq!(
            admission_result(&serde_json::json!({"state": "pending"})),
            Admission::Pending(String::new())
        );
        assert_eq!(
            admission_result(&serde_json::json!({"state": "confirmation"})),
            Admission::ConfirmationRequired
        );
        assert_eq!(
            admission_result(&serde_json::json!({"state": "added", "id": 42})),
            Admission::Added
        );
        assert_eq!(
            admission_result(&serde_json::json!({"state": "error", "code": "torrent-read-failed"})),
            Admission::Failed
        );
        assert_eq!(
            admission_result(&serde_json::json!({"state": "added"})),
            Admission::Failed
        );
    }

    #[test]
    fn torrent_info_uses_engine_counters_and_real_paths() {
        let mut input = serde_json::json!([{ "hash": "0123456789abcdef0123456789abcdef01234567",
            "name": "episode.mkv", "category_key": "tv", "status": "Downloading", "live": true, "seeding": false,
            "size": 100, "downloaded": 50, "uploaded": 20, "download_speed": 10,
            "upload_speed": 2, "seeded_seconds": 0, "ratio_limit": -2.0, "seeding_time_limit": -2,
            "paths": {"save_path": "/downloads/tv", "root_path": "", "content_path": "/downloads/tv/episode.mkv"} }]);
        let result = info_response(input.clone()).unwrap();
        assert_eq!(result[0]["progress"], 0.5);
        assert_eq!(result[0]["eta"], 5);
        assert_eq!(result[0]["state"], "downloading");
        assert_eq!(result[0]["content_path"], "/downloads/tv/episode.mkv");
        input[0]["status"] = serde_json::json!("Complete");
        input[0]["live"] = serde_json::json!(false);
        input[0]["seeding"] = serde_json::json!(true);
        assert_eq!(
            info_response(input.clone()).unwrap()[0]["state"],
            "uploading"
        );
        let properties = properties_response(input.clone()).unwrap();
        assert_eq!(properties["save_path"], "/downloads/tv");
        assert_eq!(properties["total_downloaded"], 50);
        assert!(properties_response(serde_json::json!([])).is_none());
        input[0]["size"] = serde_json::Value::Null;
        assert!(info_response(input).is_none());
    }

    #[test]
    fn file_responses_use_mdm_metadata_and_require_known_progress() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Show");
        let mut input = serde_json::json!({
            "paths": {"root_path": folder},
            "details": {"metadata_available": true, "files_truncated": false,
                "files": [{"index": 0, "path": "episode.mkv", "size": 100, "downloaded": 25, "selected": true}]}
        });
        let files = files_response(input.clone()).unwrap();
        assert_eq!(files[0]["name"], "Show/episode.mkv");
        assert_eq!(files[0]["progress"], 0.25);
        input["details"]["files"][0]["downloaded"] = serde_json::Value::Null;
        assert_eq!(files_response(input.clone()).unwrap()[0]["progress"], 0.0);
        input["complete"] = serde_json::json!(true);
        assert_eq!(files_response(input.clone()).unwrap()[0]["progress"], 1.0);
        input["details"]["files"][0]["selected"] = serde_json::json!(false);
        assert_eq!(files_response(input.clone()).unwrap()[0]["progress"], 0.0);
        input["details"]["files"][0]["selected"] = serde_json::json!(true);
        input["details"]["files"][0]["downloaded"] = serde_json::json!(101);
        assert!(files_response(input.clone()).is_none());
        input["details"]["files_truncated"] = serde_json::json!(true);
        assert!(files_response(input).is_none());
    }

    #[test]
    fn categories_translate_real_mdm_configuration_and_reject_ambiguous_data() {
        let result = category_response(serde_json::json!([
            { "name": "tv-sonarr", "folder": "/downloads/tv" },
            { "name": "radarr", "folder": "/downloads/movies" }
        ]))
        .unwrap();
        assert_eq!(result["tv-sonarr"]["savePath"], "/downloads/tv");
        assert_eq!(result["radarr"]["name"], "radarr");
        assert!(
            category_response(serde_json::json!([
                { "name": "tv", "folder": "/downloads/a" },
                { "name": "tv", "folder": "/downloads/b" }
            ]))
            .is_none()
        );
        assert!(category_response(serde_json::json!({"error": "not-ready"})).is_none());
        assert!(category_response(serde_json::json!([{ "name": "tv", "folder": "" }])).is_none());
    }

    #[test]
    fn folder_names_are_portable_deterministic_and_distinguish_labels() {
        let name = category_folder_name("tv-sonarr").unwrap();
        assert!(name.starts_with("cat-tv-sonarr-"));
        assert_eq!(category_folder_name("tv-sonarr"), Some(name.clone()));
        for other in [
            "TV-sonarr",
            "tv/sonarr",
            "tv sonarr",
            "tv_sonarr",
            "CON",
            "视频",
        ] {
            let folder = category_folder_name(other).unwrap();
            assert_ne!(folder, name);
            assert!(folder.len() <= 117);
            assert!(
                folder.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'-'
                    || b == b'_')
            );
        }
        assert_ne!(
            category_folder_name("tv/sonarr"),
            category_folder_name("tv sonarr")
        );
        assert!(category_folder_name("../outside").is_none());
    }

    #[test]
    fn categories_persist_and_refuse_unsafe_paths_or_corrupt_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("categories.json");
        let mut store = Categories::load(path.clone());
        assert_eq!(store.create("tv-sonarr", ""), Ok(()));
        assert_eq!(store.create("radarr", ""), Ok(()));
        assert_eq!(store.create("projects/linux", ""), Ok(()));
        assert_eq!(store.create("tv-sonarr", ""), Err(StatusCode::CONFLICT));
        assert_eq!(store.create("../escape", ""), Err(StatusCode::BAD_REQUEST));
        assert_eq!(
            store.create("projects//linux", ""),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            store.create("projects|linux", ""),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            store.create("custom", "/outside"),
            Err(StatusCode::BAD_REQUEST)
        );
        let reloaded = Categories::load(path.clone());
        assert_eq!(
            reloaded.folder_for("tv-sonarr"),
            Ok(category_folder_name("tv-sonarr"))
        );
        assert_eq!(reloaded.folder_for("missing"), Ok(None));
        assert_eq!(
            reloaded
                .entries
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["projects/linux", "radarr", "tv-sonarr"]
        );
        std::fs::write(&path, b"broken").unwrap();
        let mut corrupt = Categories::load(path.clone());
        assert_eq!(
            corrupt.create("new", ""),
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }

    #[test]
    fn older_category_records_derive_folder_and_tampering_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("categories.json");
        std::fs::write(&path, br#"{"legacy":{"name":"legacy","savePath":""}}"#).unwrap();
        assert_eq!(
            Categories::load(path.clone()).folder_for("legacy"),
            Ok(category_folder_name("legacy"))
        );
        std::fs::write(
            &path,
            br#"{"legacy":{"name":"legacy","savePath":"","folder":"../outside"}}"#,
        )
        .unwrap();
        assert_eq!(
            Categories::load(path).folder_for("legacy"),
            Err(StatusCode::SERVICE_UNAVAILABLE)
        );
    }

    #[test]
    fn subnet_allowlist_fails_closed_and_normalizes_mapped_peers() {
        let list = Allowlist::parse("127.0.0.1/32, 172.20.0.0/24, ::1/128");
        assert!(list.allows("172.20.0.5".parse().unwrap()));
        assert!(list.allows("::ffff:172.20.0.5".parse().unwrap()));
        assert!(!list.allows("172.20.1.5".parse().unwrap()));
        assert!(!Allowlist::parse("127.0.0.1,bad").allows("127.0.0.1".parse().unwrap()));
        assert!(!Allowlist::parse("0.0.0.0/0").allows("127.0.0.1".parse().unwrap()));
        assert!(!Allowlist::parse("").allows("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn normalizes_hashes_and_supports_all() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(parse_hashes("all"), Ok(TorrentSelection::All));
        assert_eq!(
            parse_hashes(&format!("{}|{hash}", hash.to_uppercase())),
            Ok(TorrentSelection::Hashes(vec![hash.into(), hash.into()]))
        );
    }

    #[test]
    fn rejects_ambiguous_or_invalid_selections() {
        for value in [
            "",
            "all|all",
            "ALL",
            "01234",
            "0123456789abcdef0123456789abcdef0123456z",
            "0123456789abcdef0123456789abcdef01234567|",
        ] {
            assert_eq!(parse_hashes(value), Err("invalid-hashes"), "{value}");
        }
        let too_many = vec!["0".repeat(40); MAX_HASHES + 1].join("|");
        assert_eq!(parse_hashes(&too_many), Err("invalid-hashes"));
    }

    #[test]
    fn delete_keeps_files_by_default() {
        let form: DeleteForm = serde_json::from_str(r#"{"hashes":"all"}"#).unwrap();
        assert!(!form.delete_files);
        assert_eq!(form.selection(), Ok(TorrentSelection::All));
    }
}
