//! Allowlisted web operations, executed on the application's existing single-writer loop.
//! This deliberately does not deserialize arbitrary IPC commands or expose raw Settings.
use super::*;
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
mod transfers;
pub(super) type Checks = HashMap<u64, Arc<std::sync::Mutex<Value>>>;

pub(crate) fn specs() -> Vec<Value> {
    let mut out = Vec::new();
    macro_rules! field {
        ($key:literal, $label:literal, $section:literal, $kind:literal, $min:expr, $max:expr) => {
            out.push(json!({ "key": $key, "label": crate::i18n::mark($label), "section": $section, "kind": $kind, "min": $min, "max": $max }));
        };
    }
    field!(
        "max_connections",
        "Maximum connections per download",
        "Connection",
        "number",
        1,
        128
    );
    field!(
        "many_connections",
        "Allow more than 32 connections",
        "Connection",
        "boolean",
        0,
        0
    );
    field!(
        "global_limit_on",
        "Enable global speed limit",
        "Connection",
        "boolean",
        0,
        0
    );
    field!(
        "global_limit_kb",
        "Global speed limit (KB/sec)",
        "Connection",
        "number",
        1,
        1073741824u64
    );
    field!(
        "speed_limit_on",
        "Enable per-download speed limit",
        "Connection",
        "boolean",
        0,
        0
    );
    field!(
        "speed_limit_kb",
        "Per-download speed limit (KB/sec)",
        "Connection",
        "number",
        1,
        1073741824u64
    );
    field!(
        "limiter_on_startup",
        "Enable speed limiter on startup",
        "Connection",
        "boolean",
        0,
        0
    );
    field!("user_agent", "User-Agent", "Connection", "string", 0, 1024);
    field!(
        "ignore_mtime",
        "Ignore modification time when resuming",
        "Downloads",
        "boolean",
        0,
        0
    );
    field!(
        "set_creation_date",
        "Preserve the server file date",
        "Downloads",
        "boolean",
        0,
        0
    );
    field!(
        "duplicate_action",
        "Duplicate URL policy (0: numbered copy, 1: numbered copy, 2: overwrite, 3: reuse)",
        "Downloads",
        "number",
        0,
        3
    );
    field!(
        "limits_on",
        "Enable transfer quota",
        "Downloads",
        "boolean",
        0,
        0
    );
    field!(
        "limits_mb",
        "Transfer quota (MB)",
        "Downloads",
        "number",
        1,
        1000000000
    );
    field!(
        "limits_hours",
        "Quota interval (hours)",
        "Downloads",
        "number",
        1,
        8760
    );
    field!(
        "ftp_passive",
        "Use passive FTP",
        "Connection",
        "boolean",
        0,
        0
    );
    field!(
        "proxy_mode",
        "Proxy mode (0: none, 1: system, 2: PAC, 3: manual)",
        "Proxy and DNS",
        "number",
        0,
        3
    );
    field!(
        "proxy_host",
        "Proxy host",
        "Proxy and DNS",
        "string",
        0,
        255
    );
    field!("proxy_port", "Proxy port", "Proxy and DNS", "string", 0, 5);
    field!(
        "proxy_user",
        "Proxy username",
        "Proxy and DNS",
        "string",
        0,
        255
    );
    field!(
        "proxy_http",
        "Use proxy for HTTP",
        "Proxy and DNS",
        "boolean",
        0,
        0
    );
    field!(
        "proxy_https",
        "Use proxy for HTTPS",
        "Proxy and DNS",
        "boolean",
        0,
        0
    );
    field!(
        "proxy_ftp",
        "Use proxy for FTP",
        "Proxy and DNS",
        "boolean",
        0,
        0
    );
    field!(
        "dns_mode",
        "DNS mode (0: system, 1: plain, 2: TLS, 3: HTTPS)",
        "Proxy and DNS",
        "number",
        0,
        3
    );
    field!(
        "dns_provider",
        "DNS provider ID",
        "Proxy and DNS",
        "string",
        0,
        128
    );
    field!(
        "dns_servers",
        "Custom DNS server addresses",
        "Proxy and DNS",
        "string",
        0,
        2048
    );
    field!(
        "dns_tls_name",
        "DNS TLS server name",
        "Proxy and DNS",
        "string",
        0,
        255
    );
    field!(
        "dns_doh_url",
        "DNS-over-HTTPS URL",
        "Proxy and DNS",
        "string",
        0,
        2048
    );
    field!(
        "dns_fallback",
        "Allow fallback to system DNS",
        "Proxy and DNS",
        "boolean",
        0,
        0
    );
    field!(
        "net_adapter",
        "Bind to network adapter",
        "Proxy and DNS",
        "string",
        0,
        255
    );
    field!(
        "auto_types",
        "Captured file extensions",
        "Browser capture",
        "string",
        0,
        8192
    );
    field!(
        "excluded_sites",
        "Sites excluded from capture",
        "Browser capture",
        "string",
        0,
        8192
    );
    field!(
        "capture_min_kb",
        "Minimum captured file size (KB)",
        "Browser capture",
        "number",
        0,
        1073741824u64
    );
    field!(
        "capture_opened",
        "Capture types normally opened in the browser",
        "Browser capture",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_keep_sharing",
        "Keep seeding completed torrents",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_incoming",
        "Allow incoming torrent connections",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_port",
        "Torrent listening port (0: automatic)",
        "Torrents",
        "number",
        0,
        65535
    );
    field!(
        "torrent_upnp",
        "Map torrent port with UPnP",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!("torrent_dht", "Enable DHT", "Torrents", "boolean", 0, 0);
    field!(
        "torrent_lsd",
        "Enable local peer discovery",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_proxy",
        "Use SOCKS5 proxy for torrents",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_stop_idle",
        "Stop the torrent engine when idle",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_upload_on",
        "Enable global torrent upload limit",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_upload_kb",
        "Torrent upload limit (KB/sec)",
        "Torrents",
        "number",
        1,
        1073741824u64
    );
    field!(
        "torrent_stop_ratio_on",
        "Stop seeding at ratio limit",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_stop_ratio",
        "Seeding ratio limit (hundredths)",
        "Torrents",
        "number",
        1,
        1000000
    );
    field!(
        "torrent_stop_minutes_on",
        "Stop seeding after time limit",
        "Torrents",
        "boolean",
        0,
        0
    );
    field!(
        "torrent_stop_minutes",
        "Seeding time limit (minutes)",
        "Torrents",
        "number",
        1,
        10000000
    );
    field!(
        "extract_auto",
        "Extract completed archives automatically",
        "Archives",
        "boolean",
        0,
        0
    );
    field!(
        "extract_delete",
        "Trash archives after extraction",
        "Archives",
        "boolean",
        0,
        0
    );
    field!(
        "extract_subfolder",
        "Extraction subfolder (0: smart, 1: always, 2: never)",
        "Archives",
        "number",
        0,
        2
    );
    out
}

fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, &'static str> {
    v.get(key).and_then(Value::as_str).ok_or("missing-field")
}
fn clean_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= 100
        && !name.chars().any(|c| c.is_control() || "/\\".contains(c))
}

fn safe_folder(root: &std::path::Path, relative: &str) -> Result<PathBuf, &'static str> {
    use std::path::Component;
    let rel = std::path::Path::new(relative);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("folder-outside-downloads");
    }
    let root = root
        .canonicalize()
        .map_err(|_| "downloads-folder-unavailable")?;
    let dest = root.join(rel);
    let existing = dest
        .ancestors()
        .find(|p| p.exists())
        .ok_or("invalid-folder")?
        .canonicalize()
        .map_err(|_| "invalid-folder")?;
    if !existing.starts_with(&root) {
        return Err("folder-outside-downloads");
    }
    std::fs::create_dir_all(&dest).map_err(|_| "folder-not-writable")?;
    let dest = dest.canonicalize().map_err(|_| "invalid-folder")?;
    if !dest.starts_with(root) {
        return Err("folder-outside-downloads");
    }
    Ok(dest)
}

impl App {
    fn web_config(&self) -> Value {
        let full = serde_json::to_value(&self.settings).unwrap_or(Value::Null);
        let fields = specs();
        let values = fields
            .iter()
            .filter_map(|f| {
                let key = f["key"].as_str()?;
                Some((key.to_owned(), full[key].clone()))
            })
            .collect::<serde_json::Map<_, _>>();
        let queues = self.queues.iter().map(|q| json!({ "name": q.name, "running": q.running, "files_at_once": q.files_at_once, "sync": q.sync, "ids": q.items,
            "schedule": { "start_at": q.schedule.start_at, "stop_at": q.schedule.stop_at, "days": q.schedule.days, "retries": q.schedule.retries, "once": q.schedule.once, "start_on_startup": q.schedule.start_on_startup } })).collect::<Vec<_>>();
        let categories = Category::all().into_iter().map(|c| {
            let folder = self.settings.category_dir(c, &self.downloads);
            json!({ "key": c.key(), "name": c.label(), "custom_id": if let Category::Custom(id) = c { Some(id) } else { None },
                 "folder": transfers::relative_folder(&self.downloads, &folder), "types": if let Category::Custom(id) = c { model::custom(id).map(|c| c.types).unwrap_or_default() } else { String::new() } })
        }).collect::<Vec<_>>();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // Runtime queue state isn't part of the configuration revision.
        serde_json::to_string(&json!([values, self.queues, categories]))
            .unwrap_or_default()
            .hash(&mut h);
        json!({ "revision": format!("{:016x}", h.finish()), "values": values, "fields": fields, "queues": queues, "categories": categories,
            "server": { "version": crate::help::VERSION, "downloads_root": self.downloads, "timezone": std::env::var("TZ").unwrap_or_else(|_| "System timezone".into()) } })
    }

    pub(super) fn web_request(&mut self, v: &Value) -> Result<Value, &'static str> {
        let op = text(v, "op")?;
        if matches!(
            op,
            "download.move"
                | "download.source"
                | "download.checksum"
                | "torrent.update"
                | "torrent.action"
        ) {
            return self.web_transfer(v);
        }
        if op == "config" {
            return Ok(self.web_config());
        }
        if op == "details" {
            let id = v["id"].as_u64().ok_or("invalid-id")?;
            let i = self
                .items
                .iter()
                .find(|i| i.id == id)
                .ok_or("unknown-download")?;
            let mut url = i.url.clone();
            let _ = url.set_password(None);
            let _ = url.set_username("");
            let connections = i.snap.as_ref().map(|s| s.connections.iter().map(|c| json!({ "id": c.id, "downloaded": c.downloaded, "speed": c.speed, "status": format!("{:?}", c.status) })).collect::<Vec<_>>()).unwrap_or_default();
            let segments = i
                .snap
                .as_ref()
                .map(|s| {
                    s.segments
                        .iter()
                        .map(|p| json!({ "start": p.start, "cursor": p.cursor, "end": p.end }))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            return Ok(
                json!({ "id": i.id, "file": i.file_name, "url": url, "path": i.dest, "description": i.description, "resumable": i.resumable,
                "limit_kb": i.own_limit.map(|n| n / 1024).unwrap_or(0), "category": i.category.key(), "log": i.log, "connections": connections, "segments": segments,
                "moving": self.moving.iter().any(|(id, _, _)| *id == i.id),
                "relative_folder": i.dest.parent().and_then(|p| transfers::relative_folder(&self.downloads, p)),
                "expected_checksum": i.checksum.as_ref().map(mdm_engine::verify::Hash::display),
                "checksum_result": self.web_checks.get(&i.id).map(|c| c.lock().unwrap().clone()),
                "plain_file": i.torrent.is_none() && i.video.is_none() && i.gallery.is_none() && i.remote.is_none(),
                "torrent": i.torrent.as_ref().map(|t| self.web_torrent(t)) }),
            );
        }
        if op == "download.update" {
            let id = v["id"].as_u64().ok_or("invalid-id")?;
            let category = v["category"]
                .as_str()
                .map(|key| {
                    Category::all()
                        .into_iter()
                        .find(|c| c.key() == key)
                        .ok_or("unknown-category")
                })
                .transpose()?;
            let limit = v["limit_kb"]
                .as_u64()
                .filter(|n| *n <= 1073741824)
                .ok_or("invalid-limit")?;
            let description = v["description"]
                .as_str()
                .filter(|s| s.len() <= 4096)
                .ok_or("invalid-description")?
                .to_owned();
            let i = self.item(id).ok_or("unknown-download")?;
            i.own_limit = (limit > 0).then_some(limit * 1024);
            i.description = description;
            if let Some(c) = category {
                i.category = c;
            }
            self.apply_speed_limit();
            self.dirty = true;
            return Ok(json!({ "ok": true }));
        }
        if text(v, "revision")? != self.web_config()["revision"].as_str().unwrap_or("") {
            return Err("configuration-changed-reload");
        }
        match op {
            "settings.save" => {
                let changes = v["values"].as_object().ok_or("invalid-settings")?;
                let fields = specs();
                let mut merged =
                    serde_json::to_value(&self.settings).map_err(|_| "settings-unavailable")?;
                for (key, value) in changes {
                    let f = fields
                        .iter()
                        .find(|f| f["key"] == key.as_str())
                        .ok_or("unsupported-setting")?;
                    let valid = match f["kind"].as_str() {
                        Some("boolean") => value.is_boolean(),
                        Some("number") => value.as_u64().is_some_and(|n| {
                            n >= f["min"].as_u64().unwrap() && n <= f["max"].as_u64().unwrap()
                        }),
                        Some("string") => value.as_str().is_some_and(|s| {
                            s.len() <= f["max"].as_u64().unwrap() as usize
                                && !s.chars().any(char::is_control)
                        }),
                        _ => false,
                    };
                    if !valid {
                        return Err("invalid-setting-value");
                    }
                    merged[key] = value.clone();
                }
                let new: Settings =
                    serde_json::from_value(merged).map_err(|_| "invalid-settings")?;
                if new.max_connections > 32 && !new.many_connections {
                    return Err("enable-more-connections-first");
                }
                if !new.proxy_port.is_empty()
                    && new
                        .proxy_port
                        .parse::<u16>()
                        .ok()
                        .filter(|n| *n > 0)
                        .is_none()
                {
                    return Err("invalid-proxy-port");
                }
                self.apply_options(new);
            }
            "queue.save" => {
                let name = text(v, "name")?.trim();
                if !clean_name(name) {
                    return Err("invalid-queue-name");
                }
                let old = v["old_name"].as_str();
                if v.get("old_name")
                    .is_some_and(|old| !old.is_null() && !old.is_string())
                {
                    return Err("invalid-queue-name");
                }
                let index = old.and_then(|n| self.queues.iter().position(|q| q.name == n));
                if old.is_some() && index.is_none() {
                    return Err("unknown-queue");
                }
                if self
                    .queues
                    .iter()
                    .enumerate()
                    .any(|(i, q)| q.name == name && Some(i) != index)
                {
                    return Err("duplicate-queue");
                }
                if index.is_some_and(|i| i < 2 && self.queues[i].name != name) {
                    return Err("built-in-queue");
                }
                let count = v["files_at_once"]
                    .as_u64()
                    .filter(|n| (1..=128).contains(n))
                    .ok_or("invalid-concurrency")?;
                let mut q = index
                    .map(|i| self.queues[i].clone())
                    .unwrap_or_else(|| Queue::new(name, false));
                q.name = name.into();
                q.files_at_once = count as u32;
                if v.get("ids").is_some_and(|ids| !ids.is_array()) {
                    return Err("invalid-download-list");
                }
                if let Some(ids) = v["ids"].as_array() {
                    if ids.len() > 10000 {
                        return Err("too-many-downloads");
                    }
                    let ids = ids
                        .iter()
                        .map(|n| {
                            n.as_u64()
                                .filter(|id| self.items.iter().any(|i| i.id == *id))
                                .ok_or("unknown-download")
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let unique = ids.iter().collect::<std::collections::HashSet<_>>();
                    if unique.len() != ids.len() {
                        return Err("duplicate-download");
                    }
                    q.items = ids;
                }
                if let Some(schedule) = v.get("schedule") {
                    let input = schedule.as_object().ok_or("invalid-schedule")?;
                    let mut allowed =
                        serde_json::to_value(&q.schedule).map_err(|_| "invalid-schedule")?;
                    for (key, value) in input {
                        if ![
                            "start_at",
                            "stop_at",
                            "days",
                            "retries",
                            "once",
                            "start_on_startup",
                        ]
                        .contains(&key.as_str())
                        {
                            return Err("unsupported-schedule-action");
                        }
                        allowed[key] = value.clone();
                    }
                    let s: queue::Schedule =
                        serde_json::from_value(allowed).map_err(|_| "invalid-schedule")?;
                    if [s.start_at, s.stop_at]
                        .iter()
                        .flatten()
                        .any(|(h, m, s)| *h > 23 || *m > 59 || *s > 59)
                        || s.retries.is_some_and(|n| n > 100000)
                    {
                        return Err("invalid-schedule");
                    }
                    if let Some((y, m, d)) = s.once
                        && chrono::NaiveDate::from_ymd_opt(y, m, d).is_none()
                    {
                        return Err("invalid-schedule");
                    }
                    q.schedule = s;
                }
                if let Some(i) = index {
                    self.queues[i] = q;
                } else {
                    self.queues.push(q);
                }
            }
            "queue.delete" => {
                let name = text(v, "name")?;
                let at = self
                    .queues
                    .iter()
                    .position(|q| q.name == name)
                    .ok_or("unknown-queue")?;
                if at < 2 {
                    return Err("built-in-queue");
                }
                self.delete_queue(at);
            }
            "category.save" => {
                let name = text(v, "name")?.trim();
                if !clean_name(name) {
                    return Err("invalid-category-name");
                }
                let id = v["id"]
                    .as_u64()
                    .map(|n| u32::try_from(n).map_err(|_| "invalid-category"))
                    .transpose()?;
                if v.get("id")
                    .is_some_and(|id| !id.is_null() && id.as_u64().is_none())
                {
                    return Err("invalid-category");
                }
                let old_name = id
                    .map(|id| {
                        self.settings
                            .custom_categories
                            .iter()
                            .find(|c| c.id == id)
                            .map(|c| c.name.clone())
                            .ok_or("unknown-category")
                    })
                    .transpose()?;
                if Category::all().iter().any(|c| {
                    c.label().eq_ignore_ascii_case(name)
                        && !matches!(c, Category::Custom(cid) if Some(*cid) == id)
                }) {
                    return Err("duplicate-category");
                }
                let types = text(v, "types")?;
                if types.len() > 8192 || types.chars().any(char::is_control) {
                    return Err("invalid-file-types");
                }
                let folder = safe_folder(&self.downloads, text(v, "folder")?)?;
                let mut new = self.settings.clone();
                let id = match id {
                    Some(id) => {
                        let c = new
                            .custom_categories
                            .iter_mut()
                            .find(|c| c.id == id)
                            .ok_or("unknown-category")?;
                        c.name = name.into();
                        c.types = types.into();
                        c.folder = Some(folder.to_string_lossy().into_owned());
                        id
                    }
                    None => {
                        let id = new
                            .custom_categories
                            .iter()
                            .map(|c| c.id)
                            .max()
                            .unwrap_or(0)
                            .checked_add(1)
                            .ok_or("too-many-categories")?;
                        new.custom_categories.push(model::CustomCategory {
                            id,
                            name: name.into(),
                            types: types.into(),
                            folder: Some(folder.to_string_lossy().into_owned()),
                        });
                        id
                    }
                };
                new.publish_categories();
                new.category_dirs
                    .retain(|(key, _)| key != name && old_name.as_ref() != Some(key));
                new.category_dirs
                    .push((name.into(), folder.to_string_lossy().into_owned()));
                let _ = id;
                self.apply_options(new);
            }
            "category.delete" => {
                let id = v["id"]
                    .as_u64()
                    .and_then(|id| u32::try_from(id).ok())
                    .ok_or("invalid-category")?;
                if !self.settings.custom_categories.iter().any(|c| c.id == id) {
                    return Err("unknown-category");
                }
                let old_name = model::custom(id).map(|c| c.name).unwrap_or_default();
                let mut new = self.settings.clone();
                new.custom_categories.retain(|c| c.id != id);
                new.category_dirs.retain(|(key, _)| key != &old_name);
                for i in &mut self.items {
                    if i.category == Category::Custom(id) {
                        i.category = Category::General;
                    }
                }
                self.apply_options(new);
            }
            _ => return Err("unsupported-operation"),
        }
        self.dirty = true;
        Ok(self.web_config())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_roots_and_schema_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        assert!(safe_folder(dir.path(), "../escape").is_err());
        assert!(safe_folder(dir.path(), "/escape").is_err());
        assert!(
            safe_folder(dir.path(), "nested/downloads")
                .unwrap()
                .starts_with(dir.path().canonicalize().unwrap())
        );
        for f in specs() {
            let key = f["key"].as_str().unwrap();
            assert!(
                !key.ends_with("_pass")
                    && !["password", "secret", "token", "logins", "accounts"]
                        .iter()
                        .any(|s| key.contains(s))
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn category_folders_cannot_escape_through_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        assert_eq!(
            safe_folder(root.path(), "escape/nested"),
            Err("folder-outside-downloads")
        );
        assert!(!outside.path().join("nested").exists());
    }
}
