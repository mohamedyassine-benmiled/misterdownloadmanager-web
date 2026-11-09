//! Owner-only file/torrent operations; no desktop dialogs or arbitrary filesystem paths.
use super::*;
use std::path::Path;

pub(super) fn relative_folder(root: &Path, folder: &Path) -> Option<String> {
    let root = root.canonicalize().ok()?;
    let folder = folder.canonicalize().ok()?;
    folder
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

fn plain(item: &Item) -> bool {
    item.torrent.is_none()
        && item.video.is_none()
        && item.gallery.is_none()
        && item.remote.is_none()
}

fn same_location(a: &Path, b: &Path) -> bool {
    let parents = a
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .zip(b.parent().and_then(|p| p.canonicalize().ok()));
    let Some((left, right)) = parents else {
        return a == b;
    };
    if left != right {
        return false;
    }
    match (a.file_name(), b.file_name()) {
        (Some(left), Some(right)) if cfg!(any(windows, target_os = "macos")) => {
            left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
        }
        (left, right) => left == right,
    }
}

fn held_file(root: &Path, path: &Path) -> Result<PathBuf, &'static str> {
    let root = root
        .canonicalize()
        .map_err(|_| "downloads-folder-unavailable")?;
    let file = path.canonicalize().map_err(|_| "file-unavailable")?;
    if !file.starts_with(root) {
        return Err("folder-outside-downloads");
    }
    if !file.is_file() {
        return Err("not-a-file");
    }
    Ok(file)
}

fn source_url(value: &str) -> Result<url::Url, &'static str> {
    let url = url::Url::parse(value.trim()).map_err(|_| "bad-address")?;
    if value.len() > 8192
        || !matches!(url.scheme(), "http" | "https" | "ftp" | "ftps")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("bad-address");
    }
    Ok(url)
}

/// Never overwrite a destination, including a file created after validation.
fn move_exclusive(from: &Path, to: &Path, url: &str) -> Result<(), String> {
    if std::fs::hard_link(from, to).is_err() {
        let mut input = std::fs::File::open(from).map_err(|e| e.to_string())?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut input, &mut output).and_then(|_| output.sync_all());
        drop(output);
        if let Err(e) = copied {
            let _ = std::fs::remove_file(to);
            return Err(e.to_string());
        }
    }
    #[cfg(windows)]
    crate::win32::mark_downloaded(to, url, None);
    #[cfg(unix)]
    crate::unix::mark_downloaded(to, url, None);
    std::fs::remove_file(from).map_err(|e| {
        let _ = std::fs::remove_file(to);
        e.to_string()
    })
}

fn metadata(spec: &crate::torrents::TorrentSpec) -> Result<mdm_torrent::Meta, &'static str> {
    let file = crate::torrents::torrent_file(&spec.hash).ok_or("torrent-metadata-unavailable")?;
    let bytes = mdm_torrent::read_file(&file).map_err(|_| "torrent-metadata-unavailable")?;
    let meta = mdm_torrent::inspect(&bytes).map_err(|_| "torrent-metadata-unavailable")?;
    if meta.info_hash != spec.hash {
        return Err("torrent-metadata-unavailable");
    }
    Ok(meta)
}

impl App {
    pub(super) fn web_torrent(&self, spec: &crate::torrents::TorrentSpec) -> Value {
        let live = self
            .torrent_host
            .get()
            .and_then(|t| crate::torrents::details_of(&t, &spec.hash));
        let meta = metadata(spec).ok();
        let chosen = spec.files.as_ref().map(|files| {
            files
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
        });
        let progress = live.as_ref().map(|d| {
            d.files
                .iter()
                .map(|(index, _, _, done)| (*index, *done))
                .collect::<HashMap<_, _>>()
        });
        let files = meta.as_ref().map(|m| m.files.iter().enumerate().filter(|(_, f)| !f.padding).take(10000).map(|(index, f)| {
            let done = progress.as_ref().and_then(|p| p.get(&index).copied());
            json!({ "index": index, "path": f.path, "size": f.size, "selected": chosen.as_ref().is_none_or(|files| files.contains(&index)), "downloaded": done })
        }).collect::<Vec<_>>()).unwrap_or_default();
        let peers = live.as_ref().map(|d| d.peers.iter().take(500).map(|p| json!({ "address": p.addr, "client": p.client, "downloaded": p.fetched, "uploaded": p.uploaded, "transport": p.kind })).collect::<Vec<_>>()).unwrap_or_default();
        let trackers = meta
            .as_ref()
            .map(|m| mdm_torrent::trackers::listed(&m.torrent))
            .unwrap_or_default();
        json!({ "hash": spec.hash, "files": files, "metadata_available": meta.is_some(), "files_truncated": meta.as_ref().is_some_and(|m| m.files.len() > 10000),
            "uploaded": spec.uploaded, "seeded_seconds": spec.seeded_secs, "upload_limit_kb": spec.upload_limit.map(|n| n / 1024).unwrap_or(0),
            "in_order": spec.in_order, "ends_first": spec.ends_first, "busy": self.torrent_host.busy(&spec.hash), "peers": peers, "trackers": trackers,
            "pieces_complete": live.as_ref().map(|d| d.pieces.iter().filter(|p| **p).count()), "pieces_total": live.as_ref().map(|d| d.pieces.len()) })
    }

    pub(super) fn web_transfer(&mut self, value: &Value) -> Result<Value, &'static str> {
        let op = text(value, "op")?;
        let id = value["id"].as_u64().ok_or("invalid-id")?;
        let item = self
            .items
            .iter()
            .find(|i| i.id == id)
            .ok_or("unknown-download")?;
        if self.moving.iter().any(|(moving, _, _)| *moving == id) {
            return Err("file-busy");
        }
        match op {
            "download.source" => {
                if !plain(item) {
                    return Err("plain-file-required");
                }
                if item.live.is_some() || item.status == Status::Complete {
                    return Err("pause-unfinished-first");
                }
                let url = source_url(text(value, "url")?)?;
                let state = mdm_engine::state::state_path(&item.dest);
                if state.try_exists().map_err(|_| "resume-state-unavailable")? {
                    held_file(&self.downloads, &state)?;
                    let mut saved = mdm_engine::state::ResumeState::load(&state)
                        .map_err(|_| "resume-state-unavailable")?;
                    saved.url = url.clone();
                    saved.final_url = url.clone();
                    saved.final_get = false;
                    saved
                        .write(&state)
                        .map_err(|_| "resume-state-unavailable")?;
                } else if item.downloaded > 0 {
                    return Err("resume-state-unavailable");
                }
                let item = self.item(id).unwrap();
                item.note(crate::i18n::fmt(
                    crate::i18n::t("Address changed to {host}"),
                    &[("host", url.host_str().unwrap_or("?"))],
                ));
                item.url = url;
                item.login = None;
                item.context = ipc::Command {
                    private: item.context.private,
                    ..Default::default()
                };
                item.error = None;
                if item.status == Status::Error {
                    item.status = Status::Stopped;
                }
                self.kept_answers.remove(&id);
                self.dirty = true;
                Ok(json!({ "ok": true }))
            }
            "download.move" => {
                if !plain(item) {
                    return Err("plain-file-required");
                }
                if item.live.is_some() || (item.status != Status::Complete && item.downloaded != 0)
                {
                    return Err("move-not-ready");
                }
                let name = text(value, "name")?;
                if name.is_empty()
                    || name.chars().count() > 255
                    || mdm_engine::naming::sanitize(name) != name
                {
                    return Err("invalid-file-name");
                }
                let folder = safe_folder(&self.downloads, text(value, "folder")?)?;
                if mdm_engine::naming::fit_name(&folder, name).as_deref() != Some(name) {
                    return Err("invalid-file-name");
                }
                let target = folder.join(name);
                let old = item.dest.clone();
                if target == old || old.canonicalize().ok().as_ref() == Some(&target) {
                    return Ok(json!({ "ok": true, "pending": false }));
                }
                if target.symlink_metadata().is_ok()
                    || mdm_engine::naming::part_path(&target).exists()
                    || mdm_engine::state::state_path(&target).exists()
                    || self
                        .items
                        .iter()
                        .any(|i| i.id != id && same_location(&i.dest, &target))
                {
                    return Err("destination-exists");
                }
                if item.status == Status::Complete {
                    let source = held_file(&self.downloads, &old)?;
                    let mut origin = item.url.clone();
                    let _ = origin.set_password(None);
                    let _ = origin.set_username("");
                    let url = origin.to_string();
                    let reserve =
                        mdm_engine::naming::reserve(&target).ok_or("destination-exists")?;
                    let (tx, rx) = std::sync::mpsc::channel();
                    let to = target.clone();
                    self.item(id)
                        .unwrap()
                        .note(crate::i18n::t("Moving the file..."));
                    std::thread::spawn(move || {
                        let _reserve = reserve;
                        let _ = tx.send(move_exclusive(&source, &to, &url));
                    });
                    self.moving.push((id, target, rx));
                    Ok(json!({ "ok": true, "pending": true }))
                } else {
                    take_place(self.item(id).unwrap(), target);
                    self.kept_answers.remove(&id);
                    self.dirty = true;
                    Ok(json!({ "ok": true, "pending": false }))
                }
            }
            "download.checksum" => {
                if !plain(item) {
                    return Err("plain-file-required");
                }
                if item.live.is_some() {
                    return Err("pause-unfinished-first");
                }
                let expected = text(value, "expected")?;
                if expected.len() > 256 {
                    return Err("invalid-checksum");
                }
                let hash = if expected.trim().is_empty() {
                    None
                } else {
                    Some(mdm_engine::verify::Hash::parse(expected).ok_or("invalid-checksum")?)
                };
                if self
                    .web_checks
                    .values()
                    .filter(|c| c.lock().unwrap()["state"] == "checking")
                    .count()
                    >= 2
                    || self
                        .web_checks
                        .get(&id)
                        .is_some_and(|c| c.lock().unwrap()["state"] == "checking")
                {
                    return Err("file-busy");
                }
                let file = if item.status == Status::Complete && hash.is_some() {
                    Some(held_file(&self.downloads, &item.dest)?)
                } else {
                    None
                };
                let state = mdm_engine::state::state_path(&item.dest);
                if state.exists() {
                    held_file(&self.downloads, &state)?;
                    let mut saved = mdm_engine::state::ResumeState::load(&state)
                        .map_err(|_| "resume-state-unavailable")?;
                    saved.checksum = hash.clone();
                    saved
                        .write(&state)
                        .map_err(|_| "resume-state-unavailable")?;
                }
                self.item(id).unwrap().checksum = hash.clone();
                self.dirty = true;
                self.web_checks
                    .retain(|id, _| self.items.iter().any(|i| i.id == *id));
                self.web_checks.remove(&id);
                if self.web_checks.len() >= 64
                    && let Some(old) = self
                        .web_checks
                        .iter()
                        .find(|(_, c)| c.lock().unwrap()["state"] != "checking")
                        .map(|(id, _)| *id)
                {
                    self.web_checks.remove(&old);
                }
                if let (Some(file), Some(hash)) = (file, hash) {
                    let result = Arc::new(std::sync::Mutex::new(json!({ "state": "checking" })));
                    self.web_checks.insert(id, result.clone());
                    std::thread::spawn(move || {
                        let value = match mdm_engine::verify::digest(&file, hash.algo) {
                            Ok(actual) => {
                                json!({ "state": if actual == hash.value { "match" } else { "mismatch" }, "actual": mdm_engine::verify::Hash { algo: hash.algo, value: actual }.display() })
                            }
                            Err(_) => json!({ "state": "error", "code": "file-unavailable" }),
                        };
                        *result.lock().unwrap() = value;
                    });
                    Ok(json!({ "ok": true, "pending": true }))
                } else {
                    Ok(json!({ "ok": true, "pending": false }))
                }
            }
            "torrent.update" | "torrent.action" => {
                let spec = item.torrent.clone().ok_or("torrent-required")?;
                if self.torrent_host.busy(&spec.hash) {
                    return Err("torrent-busy");
                }
                if op == "torrent.action" {
                    let act = match text(value, "action")? {
                        "recheck" => crate::torrents::Act::Recheck,
                        "reannounce" => crate::torrents::Act::Reannounce,
                        "add-trackers" => {
                            let lines = text(value, "trackers")?;
                            if lines.len() > 16384 {
                                return Err("invalid-trackers");
                            }
                            let trackers = lines
                                .lines()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(|s| {
                                    let url = url::Url::parse(s).map_err(|_| "invalid-trackers")?;
                                    if !matches!(url.scheme(), "http" | "https" | "udp")
                                        || url.host_str().is_none()
                                        || !url.username().is_empty()
                                        || url.password().is_some()
                                        || !mdm_torrent::trackers::allowed(
                                            s,
                                            self.settings.torrent_proxy,
                                        )
                                    {
                                        return Err("invalid-trackers");
                                    }
                                    Ok(s.to_owned())
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            if trackers.is_empty() || trackers.len() > 100 {
                                return Err("invalid-trackers");
                            }
                            crate::torrents::Act::AddTrackers(trackers)
                        }
                        _ => return Err("unsupported-torrent-action"),
                    };
                    self.torrent_act(id, act);
                } else {
                    let meta = metadata(&spec)?;
                    let files = value["files"]
                        .as_array()
                        .filter(|f| !f.is_empty() && f.len() <= 10000)
                        .ok_or("invalid-torrent-files")?;
                    let files = files
                        .iter()
                        .map(|n| {
                            n.as_u64()
                                .and_then(|i| usize::try_from(i).ok())
                                .filter(|i| meta.files.get(*i).is_some_and(|f| !f.padding))
                                .ok_or("invalid-torrent-files")
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if files.iter().collect::<std::collections::HashSet<_>>().len() != files.len() {
                        return Err("invalid-torrent-files");
                    }
                    let limit = value["upload_limit_kb"]
                        .as_u64()
                        .filter(|n| *n <= 1073741824)
                        .ok_or("invalid-limit")?;
                    let order = value["in_order"]
                        .as_bool()
                        .ok_or("invalid-torrent-options")?;
                    let ends = value["ends_first"]
                        .as_bool()
                        .ok_or("invalid-torrent-options")?;
                    let size = files.iter().map(|i| meta.files[*i].size).sum();
                    let all = files.len() == meta.files.iter().filter(|f| !f.padding).count();
                    self.choose_files(id, files, all, size);
                    let item = self.item(id).unwrap();
                    let spec = item.torrent.as_mut().unwrap();
                    spec.upload_limit = (limit > 0).then_some(limit * 1024);
                    spec.in_order = order;
                    spec.ends_first = ends;
                    if let Some(model::Job::Torrent(run)) = item.live.as_ref().map(|l| &l.dl) {
                        run.set_upload_limit(spec.upload_limit);
                        run.set_order(spec.order());
                    }
                    self.dirty = true;
                }
                Ok(json!({ "ok": true, "pending": self.torrent_host.busy(&spec.hash) }))
            }
            _ => Err("unsupported-operation"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sources_and_moves_are_explicit_and_non_destructive() {
        for url in [
            "file:///etc/passwd",
            "https://user:pass@example.com/x",
            "javascript:alert(1)",
            "data:text/plain,x",
        ] {
            assert!(source_url(url).is_err());
        }
        assert!(source_url("https://example.com/new.zip").is_ok());
        let dir = tempfile::tempdir().unwrap();
        assert!(same_location(
            &dir.path().join("future.zip"),
            &dir.path().canonicalize().unwrap().join("future.zip")
        ));
        if cfg!(any(windows, target_os = "macos")) {
            assert!(same_location(
                &dir.path().join("future.zip"),
                &dir.path().join("FUTURE.ZIP")
            ));
        }
        let from = dir.path().join("old");
        let to = dir.path().join("new");
        std::fs::write(&from, "original").unwrap();
        std::fs::write(&to, "do not replace").unwrap();
        assert!(move_exclusive(&from, &to, "https://example.com/x").is_err());
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "do not replace");
        assert_eq!(std::fs::read_to_string(&from).unwrap(), "original");
        std::fs::remove_file(&to).unwrap();
        move_exclusive(&from, &to, "https://example.com/x").unwrap();
        assert!(!from.exists());
        assert_eq!(std::fs::read_to_string(to).unwrap(), "original");
    }
}
