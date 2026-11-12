//! Bounded, in-memory results for asynchronous remote torrent admission.
//! This reports validation outcomes, not durable capture jobs or retry deduplication.
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::App;
use crate::model::Category;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) struct PendingTorrent {
    pub meta: mdm_torrent::Meta,
    pub category: Category,
    pub dir: PathBuf,
    pub queue: Option<usize>,
    pub queue_name: Option<String>,
    pub from_magnet: bool,
}

pub(crate) fn source_hash(source: &mdm_torrent::Source) -> Option<String> {
    match source {
        mdm_torrent::Source::File(bytes) => mdm_torrent::inspect(bytes).ok().map(|m| m.info_hash),
        mdm_torrent::Source::Magnet(text) => mdm_torrent::magnet_info_hash(text),
    }
}

#[derive(Default)]
pub(crate) struct Additions {
    entries: HashMap<String, (Instant, Value)>,
    sequence: u64,
    held: HashMap<String, PendingTorrent>,
}

impl Additions {
    pub(crate) fn begin(&mut self) -> Result<String, &'static str> {
        self.entries
            .retain(|_, (at, _)| at.elapsed() < Duration::from_secs(900));
        self.held.retain(|id, _| self.entries.contains_key(id));
        if self.entries.len() >= 128 {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, (_, v))| {
                    !matches!(v["state"].as_str(), Some("pending" | "confirmation"))
                })
                .min_by_key(|(_, (at, _))| *at)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                self.entries.remove(&id);
            } else {
                return Err("torrent-add-busy");
            }
        }
        self.sequence = self.sequence.wrapping_add(1);
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!("{epoch:x}-{:x}", self.sequence);
        self.entries
            .insert(id.clone(), (Instant::now(), json!({ "state": "pending" })));
        Ok(id)
    }

    pub(crate) fn finish(&mut self, request: &str, result: Value) {
        if let Some(entry) = self.entries.get_mut(request) {
            *entry = (Instant::now(), result);
        }
        self.held.remove(request);
    }

    pub(crate) fn status(&self, request: &str) -> Result<Value, &'static str> {
        self.entries
            .get(request)
            .filter(|(at, _)| at.elapsed() < Duration::from_secs(900))
            .map(|(_, result)| result.clone())
            .ok_or("torrent-add-result-unavailable")
    }

    fn hold(&mut self, request: &str, torrent: PendingTorrent, result: Value) {
        // Hold at most 64 MiB of uploaded metadata awaiting explicit owner consent.
        let bytes: usize = self.held.values().map(|t| t.meta.torrent.len()).sum();
        if bytes.saturating_add(torrent.meta.torrent.len()) > 64 * 1024 * 1024 {
            self.finish(
                request,
                json!({ "state": "error", "code": "torrent-add-busy" }),
            );
            return;
        }
        self.finish(request, result);
        self.held.insert(request.to_owned(), torrent);
    }
}

// Existing data may be reused only within the configured downloads root. Recheck
// canonical ancestors at confirmation time; don't follow a symlink out of that root.
fn safe_destination(root: &Path, meta: &mdm_torrent::Meta, dir: &Path) -> Result<(), &'static str> {
    let root = root
        .canonicalize()
        .map_err(|_| "downloads-folder-unavailable")?;
    let base = meta.folder_in(dir);
    for file in meta.files.iter().filter(|f| !f.padding) {
        if file
            .path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("invalid-torrent-files");
        }
        let path = base.join(&file.path);
        for ancestor in path.ancestors() {
            match std::fs::symlink_metadata(ancestor) {
                Ok(info) => {
                    if info.file_type().is_symlink() {
                        return Err("folder-outside-downloads");
                    }
                    let resolved = ancestor.canonicalize().map_err(|_| "file-unavailable")?;
                    if !resolved.starts_with(&root) {
                        return Err("folder-outside-downloads");
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err("file-unavailable"),
            }
        }
        if path.exists() && !path.is_file() {
            return Err("not-a-file");
        }
    }
    Ok(())
}

fn location_key(path: &Path) -> String {
    let resolved = path
        .ancestors()
        .find_map(|ancestor| {
            ancestor
                .canonicalize()
                .ok()
                .map(|base| base.join(path.strip_prefix(ancestor).unwrap_or(Path::new(""))))
        })
        .unwrap_or_else(|| path.to_path_buf());
    let key = resolved.to_string_lossy().into_owned();
    if cfg!(any(windows, target_os = "macos")) {
        key.to_lowercase()
    } else {
        key
    }
}

fn numbered_destination(
    dir: &Path,
    name: &str,
    occupied: &std::collections::HashSet<String>,
) -> PathBuf {
    let fitted = mdm_engine::naming::fit_name(dir, name).unwrap_or_else(|| name.to_owned());
    let extension = fitted
        .rfind('.')
        .filter(|i| *i > 0)
        .map(|i| &fitted[i..])
        .unwrap_or("");
    mdm_engine::naming::stem_destination(dir, name, |stem| {
        let dest = dir.join(format!("{stem}{extension}"));
        [
            dest.clone(),
            mdm_engine::naming::part_path(&dest),
            mdm_engine::state::state_path(&dest),
        ]
        .iter()
        .any(|p| occupied.contains(&location_key(p)))
    })
}

impl App {
    fn web_reserved_paths(&self) -> std::collections::HashSet<String> {
        let mut reserved = std::collections::HashSet::new();
        for item in &self.items {
            let paths = if let Some(spec) = &item.torrent {
                crate::torrents::torrent_file(&spec.hash)
                    .and_then(|p| std::fs::read(p).ok())
                    .and_then(|b| mdm_torrent::inspect(&b).ok())
                    .map(|meta| {
                        meta.files
                            .into_iter()
                            .filter(|f| !f.padding)
                            .map(|f| spec.folder.join(f.path))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| vec![item.dest.clone()])
            } else {
                vec![
                    item.dest.clone(),
                    mdm_engine::naming::part_path(&item.dest),
                    mdm_engine::state::state_path(&item.dest),
                ]
            };
            reserved.extend(paths.iter().map(|p| location_key(p)));
        }
        for (_, target, _) in &self.moving {
            reserved.extend(
                [
                    target.clone(),
                    mdm_engine::naming::part_path(target),
                    mdm_engine::state::state_path(target),
                ]
                .iter()
                .map(|p| location_key(p)),
            );
        }
        reserved
    }

    pub(in crate::app) fn web_new_destination(&self, dir: &Path, name: &str) -> PathBuf {
        numbered_destination(dir, name, &self.web_reserved_paths())
    }
    pub(in crate::app) fn web_admit_torrent(
        &mut self,
        request: &str,
        torrent: PendingTorrent,
        approved: bool,
    ) {
        let fail = |code| json!({ "state": "error", "code": code });
        let current_dir = crate::model::category_folder(
            torrent.category,
            &self.downloads,
            &self.settings.category_dirs,
        );
        if current_dir != torrent.dir
            || torrent
                .queue
                .is_some_and(|q| self.queues.get(q).map(|q| &q.name) != torrent.queue_name.as_ref())
        {
            self.web
                .additions
                .finish(request, fail("torrent-destination-changed"));
            return;
        }
        if let Some(item) = self.items.iter().find(|i| {
            i.torrent
                .as_ref()
                .is_some_and(|t| t.hash == torrent.meta.info_hash)
        }) {
            self.web.additions.finish(
                request,
                json!({ "state": "added", "id": item.id, "duplicate": true }),
            );
            return;
        }
        if let Err(code) = safe_destination(&self.downloads, &torrent.meta, &torrent.dir) {
            self.web.additions.finish(request, fail(code));
            return;
        }
        let base = torrent.meta.folder_in(&torrent.dir);
        let paths: std::collections::HashSet<_> = torrent
            .meta
            .files
            .iter()
            .filter(|f| !f.padding)
            .map(|f| location_key(&base.join(&f.path)))
            .collect();
        // Consent to reuse data is not permission for two list entries to write it.
        if self.web_reserved_paths().iter().any(|p| paths.contains(p)) {
            self.web
                .additions
                .finish(request, fail("destination-in-use"));
            return;
        }
        let found = torrent.meta.existing_in(&torrent.dir);
        if !approved && !found.is_empty() {
            let result = json!({ "state": "confirmation", "code": "torrent-files-exist", "name": torrent.meta.name,
                "destination": base, "existing_count": found.len(), "files": found.iter().take(20).map(|p| p.strip_prefix(&base).unwrap_or(p).to_string_lossy()).collect::<Vec<_>>() });
            self.web.additions.hold(request, torrent, result);
            return;
        }
        let next_id = self.next_id;
        self.add_torrent(crate::torrentdlg::TorrentAdd {
            meta: torrent.meta,
            dir: torrent.dir,
            files: None,
            category: torrent.category,
            start_now: torrent.queue.is_none(),
            from_magnet: torrent.from_magnet,
            in_order: false,
            queue: torrent.queue,
            quiet: true,
        });
        let result = if self.items.iter().any(|i| i.id == next_id) {
            json!({ "state": "added", "id": next_id, "reused": approved })
        } else {
            fail("torrent-save-failed")
        };
        self.web.additions.finish(request, result);
    }

    pub(super) fn web_addition_request(&mut self, v: &Value) -> Result<Value, &'static str> {
        let op = super::text(v, "op")?;
        if op == "download.add" {
            let address = super::text(v, "url")?;
            if address.len() > 8192 {
                return Err("bad-address");
            }
            let url = url::Url::parse(address.trim()).map_err(|_| "bad-address")?;
            if !mdm_engine::supported(&url) && mdm_torrent::Source::magnet(address.trim()).is_none()
            {
                return Err("unsupported");
            }
            if !url.username().is_empty() || url.password().is_some() {
                return Err("bad-address");
            }
            let choice = v["duplicate_choice"].as_str();
            if !matches!(choice, None | Some("new" | "existing")) {
                return Err("invalid-duplicate-choice");
            }
            if let Some(existing) = self
                .items
                .iter()
                .find(|i| i.url == url && i.torrent.is_none())
            {
                if choice.is_none() || v["existing_id"].as_u64() != Some(existing.id) {
                    return Ok(
                        json!({ "confirmation": "duplicate", "id": existing.id, "file": existing.file_name, "status": format!("{:?}", existing.status) }),
                    );
                }
                if choice == Some("existing") {
                    let id = existing.id;
                    return Ok(json!({ "id": id, "pending": false, "duplicate": true }));
                }
            } else if choice == Some("existing") {
                return Err("duplicate-changed");
            }
            let cmd = mdm_ipc::Command {
                url: Some(address.to_owned()),
                category: v["category"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
                queue: v["queue"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
                later: !v["start"].as_bool().unwrap_or(true),
                silent: true,
                ..Default::default()
            };
            return serde_json::from_str(&self.remote_add_policy(cmd, Some(1)))
                .map_err(|_| "invalid-add-result");
        }
        let request = super::text(v, "request_id")?;
        let state = self.web.additions.status(request)?;
        if state["state"] != "confirmation" {
            return Ok(state);
        }
        if op == "torrent.add.cancel" {
            let result = json!({ "state": "cancelled" });
            self.web.additions.finish(request, result.clone());
            return Ok(result);
        }
        if v["approve_existing"].as_bool() != Some(true) {
            return Err("confirmation-required");
        }
        let held = self
            .web
            .additions
            .held
            .remove(request)
            .ok_or("torrent-add-result-unavailable")?;
        self.web_admit_torrent(request, held, true);
        self.web.additions.status(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(path: &str) -> mdm_torrent::Meta {
        mdm_torrent::Meta {
            name: "fixture".into(),
            info_hash: "0".repeat(40),
            files: vec![mdm_torrent::FileEntry {
                path: path.into(),
                size: 1,
                padding: false,
            }],
            piece_length: 16384,
            torrent: Vec::new(),
            peers: Vec::new(),
        }
    }

    #[test]
    fn duplicate_torrents_normalize_hex_and_base32_magnets() {
        for (hash, expected) in [
            ("AB".repeat(20), "ab".repeat(20)),
            ("A".repeat(32), "00".repeat(20)),
            ("7".repeat(32), "ff".repeat(20)),
        ] {
            assert_eq!(
                source_hash(&mdm_torrent::Source::Magnet(format!(
                    "magnet:?xt=urn:btih:{hash}"
                ))),
                Some(expected)
            );
        }
        assert_eq!(
            source_hash(&mdm_torrent::Source::Magnet(
                "magnet:?xt=urn:btih:invalid".into()
            )),
            None
        );
    }

    #[test]
    fn queued_copies_and_their_sidecars_reserve_distinct_paths() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("archive.zip");
        let mut occupied = std::collections::HashSet::from([location_key(&first)]);
        let second = numbered_destination(root.path(), "archive.zip", &occupied);
        assert_eq!(second, root.path().join("archive (1).zip"));
        occupied.insert(location_key(&mdm_engine::state::state_path(&second)));
        let third = numbered_destination(root.path(), "archive.zip", &occupied);
        assert_eq!(third, root.path().join("archive (2).zip"));
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(!third.exists());
    }

    #[test]
    fn reuse_cannot_escape_root_or_use_a_directory_as_a_file() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        assert_eq!(
            safe_destination(root.path(), &meta("../escape"), root.path()),
            Err("invalid-torrent-files")
        );
        assert_eq!(
            safe_destination(root.path(), &meta("file"), elsewhere.path()),
            Err("folder-outside-downloads")
        );
        std::fs::create_dir(root.path().join("folder")).unwrap();
        assert_eq!(
            safe_destination(root.path(), &meta("folder"), root.path()),
            Err("not-a-file")
        );
        assert!(safe_destination(root.path(), &meta("new/file.bin"), root.path()).is_ok());
    }

    #[test]
    fn awaiting_consent_is_not_evicted_and_expiry_drops_metadata() {
        let mut jobs = Additions::default();
        let first = jobs.begin().unwrap();
        jobs.hold(
            &first,
            PendingTorrent {
                meta: meta("file"),
                category: Category::General,
                dir: PathBuf::new(),
                queue: None,
                queue_name: None,
                from_magnet: false,
            },
            json!({ "state": "confirmation" }),
        );
        for _ in 1..128 {
            jobs.begin().unwrap();
        }
        assert_eq!(jobs.begin().unwrap_err(), "torrent-add-busy");
        assert!(jobs.held.contains_key(&first));
        jobs.entries.get_mut(&first).unwrap().0 = Instant::now() - Duration::from_secs(901);
        assert!(jobs.status(&first).is_err());
        jobs.begin().unwrap();
        assert!(!jobs.held.contains_key(&first));
    }

    #[test]
    fn results_are_bounded_and_report_failures() {
        let mut jobs = Additions::default();
        let first = jobs.begin().unwrap();
        assert_eq!(jobs.status(&first).unwrap()["state"], "pending");
        jobs.finish(
            &first,
            json!({ "state": "error", "code": "torrent-files-exist" }),
        );
        assert_eq!(jobs.status(&first).unwrap()["code"], "torrent-files-exist");
        for _ in 0..128 {
            jobs.begin().unwrap();
        }
        assert_eq!(jobs.entries.len(), 128);
        assert!(jobs.status(&first).is_err());
        assert_eq!(jobs.begin().unwrap_err(), "torrent-add-busy");
    }
}
