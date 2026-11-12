//! Lossless engine-side snapshot for the future qBittorrent adapter.
//! These are internal facts, not qBittorrent response objects: in particular, MDM categories
//! and destination paths must not be presented as qBittorrent category or content paths.

use crate::model::Item;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[cfg(test)]
fn ordinary_directory(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return false;
    }
    // Junctions and other reparse points need not be reported as symbolic links on Windows.
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

/// Prepare a category folder within the configured downloads root. Never interpret the label as
/// a path. This is not a substitute for revalidating paths at torrent admission and file creation.
/// Test-only: the path-based implementation is vulnerable to directory replacement races.
#[cfg(test)]
pub(super) fn prepare_category_folder(
    root: &Path,
    category: &str,
) -> Result<PathBuf, &'static str> {
    let name = mdm_remote::qbittorrent::category_folder_name(category).ok_or("invalid-category")?;
    let root = root
        .canonicalize()
        .map_err(|_| "downloads-folder-unavailable")?;
    if !root.is_dir() {
        return Err("downloads-folder-unavailable");
    }
    let mut current = root.clone();
    for component in ["webmister-qbit-categories", name.as_str()] {
        current.push(component);
        match std::fs::create_dir(&current) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err("category-folder-unavailable"),
        }
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|_| "category-folder-unavailable")?;
        if !ordinary_directory(&metadata) {
            return Err("unsafe-category-folder");
        }
        let canonical = current
            .canonicalize()
            .map_err(|_| "category-folder-unavailable")?;
        if !canonical.starts_with(&root) {
            return Err("unsafe-category-folder");
        }
        current = canonical;
    }
    Ok(current)
}

pub(super) fn snapshot(items: &[Item]) -> Value {
    Value::Array(items.iter().filter_map(torrent_snapshot).collect())
}

/// Resolve hashes on the app's single-writer thread so an `all` request cannot affect
/// non-torrent downloads and the selection cannot change between lookup and the action.
pub(super) fn torrent_ids(items: &[Item], hashes: &str) -> Result<Vec<u64>, &'static str> {
    let selection = mdm_remote::qbittorrent::parse_hashes(hashes)?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let torrent = item.torrent.as_ref()?;
            match &selection {
                mdm_remote::qbittorrent::TorrentSelection::All => Some(item.id),
                mdm_remote::qbittorrent::TorrentSelection::Hashes(hashes) => hashes
                    .iter()
                    .any(|hash| torrent.hash.eq_ignore_ascii_case(hash))
                    .then_some(item.id),
            }
        })
        .collect())
}

/// Derive paths from MDM's stored torrent output folder and metadata, not from a category label.
/// A missing or ambiguous metadata record must stay unknown rather than advertise a guessed path.
fn content_paths(meta: &mdm_torrent::Meta, folder: &Path) -> Option<(PathBuf, PathBuf, PathBuf)> {
    use std::path::Component;
    if !folder.is_absolute() {
        return None;
    }
    let files: Vec<_> = meta.files.iter().filter(|file| !file.padding).collect();
    if files.is_empty()
        || files.iter().any(|file| {
            file.path.as_os_str().is_empty()
                || file
                    .path
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
        })
    {
        return None;
    }
    if files.len() == 1 {
        Some((
            folder.to_owned(),
            PathBuf::new(),
            folder.join(&files[0].path),
        ))
    } else {
        Some((
            folder.parent()?.to_owned(),
            folder.to_owned(),
            folder.to_owned(),
        ))
    }
}

pub(super) fn import_paths(item: &Item) -> Option<Value> {
    let torrent = item.torrent.as_ref()?;
    let meta = super::transfers::metadata(torrent).ok()?;
    item_import_paths(&meta, &torrent.folder, &item.dest)
}

fn item_import_paths(meta: &mdm_torrent::Meta, folder: &Path, destination: &Path) -> Option<Value> {
    let (save, root, content) = content_paths(meta, folder)?;
    // A single-file torrent can be renamed after download. Its metadata still contains the old
    // name; do not claim that stale location is the file an importer should use.
    if root.as_os_str().is_empty() && destination != content {
        return None;
    }
    Some(json!({ "save_path": save, "root_path": root, "content_path": content }))
}

pub(super) fn torrent_snapshot(item: &Item) -> Option<Value> {
    let torrent = item.torrent.as_ref()?;
    Some(json!({
        "id": item.id,
        "hash": torrent.hash,
        "name": item.file_name,
        "destination": item.dest,
        "torrent_folder": torrent.folder,
        "size": item.size,
        "downloaded": item.downloaded,
        "status": item.status,
        "uploaded": torrent.uploaded,
        "seeded_seconds": torrent.seeded_secs,
        "ratio_limit": if torrent.ratio_override { torrent.ratio_limit.unwrap_or(-1.0) } else { -2.0 },
        "seeding_time_limit": if torrent.seed_time_override { torrent.seed_time_limit.map(|seconds| (seconds / 60) as i64).unwrap_or(-1) } else { -2 },
        "selected_files": torrent.files,
        "category_key": item.category.key(),
        "live": item.live.is_some(),
        "download_speed": if item.live.is_some() { item.rate() as u64 } else { 0 },
        "upload_speed": torrent.up_speed as u64,
        "error": item.error,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Category, Status};
    use crate::torrents::TorrentSpec;
    use std::path::PathBuf;

    #[test]
    fn import_paths_follow_mdm_single_and_multi_file_layout() {
        let root = tempfile::tempdir().unwrap();
        let mut meta = mdm_torrent::Meta {
            name: "Show".into(),
            info_hash: "0".repeat(40),
            piece_length: 16384,
            torrent: Vec::new(),
            peers: Vec::new(),
            files: vec![mdm_torrent::FileEntry {
                path: "episode.mkv".into(),
                size: 1,
                padding: false,
            }],
        };
        let folder = root.path();
        assert_eq!(
            content_paths(&meta, folder),
            Some((
                folder.to_owned(),
                PathBuf::new(),
                folder.join("episode.mkv")
            ))
        );
        assert!(item_import_paths(&meta, folder, &folder.join("episode.mkv")).is_some());
        assert!(item_import_paths(&meta, folder, &folder.join("renamed.mkv")).is_none());
        meta.files.push(mdm_torrent::FileEntry {
            path: "second.mkv".into(),
            size: 1,
            padding: false,
        });
        let folder = root.path().join("Show");
        assert_eq!(
            content_paths(&meta, &folder),
            Some((root.path().to_owned(), folder.clone(), folder))
        );
        meta.files[0].path = "../escape".into();
        assert!(content_paths(&meta, root.path()).is_none());
    }

    #[test]
    fn category_folder_is_created_under_root_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let first = prepare_category_folder(dir.path(), "projects/linux").unwrap();
        assert!(first.is_dir());
        assert!(first.starts_with(dir.path().canonicalize().unwrap()));
        assert_eq!(
            first,
            prepare_category_folder(dir.path(), "projects/linux").unwrap()
        );
        assert_ne!(
            first,
            prepare_category_folder(dir.path(), "projects-linux").unwrap()
        );
        assert_eq!(
            prepare_category_folder(dir.path(), "../escape"),
            Err("invalid-category")
        );
    }

    #[test]
    fn category_folder_refuses_existing_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("webmister-qbit-categories"),
            b"not a folder",
        )
        .unwrap();
        assert!(prepare_category_folder(root.path(), "example").is_err());
        assert_eq!(
            std::fs::read(root.path().join("webmister-qbit-categories")).unwrap(),
            b"not a folder"
        );
    }

    #[cfg(unix)]
    #[test]
    fn category_folder_refuses_symlinked_parent() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            outside.path(),
            root.path().join("webmister-qbit-categories"),
        )
        .unwrap();
        assert_eq!(
            prepare_category_folder(root.path(), "tv-sonarr"),
            Err("unsafe-category-folder")
        );
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn category_folder_refuses_symlinked_leaf() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let base = root.path().join("webmister-qbit-categories");
        std::fs::create_dir(&base).unwrap();
        let name = mdm_remote::qbittorrent::category_folder_name("tv-sonarr").unwrap();
        std::os::unix::fs::symlink(outside.path(), base.join(name)).unwrap();
        assert_eq!(
            prepare_category_folder(root.path(), "tv-sonarr"),
            Err("unsafe-category-folder")
        );
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[test]
    fn snapshot_excludes_non_torrents_and_preserves_unknown_values() {
        let make = |id| {
            Item::new(
                id,
                "https://example.test/file".parse().unwrap(),
                PathBuf::from("/downloads/item"),
                Category::General,
            )
        };
        let plain = make(1);
        let mut torrent = make(2);
        torrent.status = Status::Stopped;
        torrent.torrent = Some(TorrentSpec {
            hash: "0123456789abcdef0123456789abcdef01234567".into(),
            files: None,
            folder: PathBuf::from("/downloads/item"),
            uploaded: 10,
            seeded_secs: 20,
            peers: 0,
            up_speed: 0.0,
            upload_limit: None,
            ratio_override: false,
            ratio_limit: None,
            seed_time_override: false,
            seed_time_limit: None,
            in_order: false,
            ends_first: true,
        });
        let items = [plain, torrent];
        assert_eq!(torrent_ids(&items, "all").unwrap(), vec![2]);
        assert_eq!(
            torrent_ids(&items, "0123456789ABCDEF0123456789ABCDEF01234567").unwrap(),
            vec![2]
        );
        assert!(
            torrent_ids(&items, "ffffffffffffffffffffffffffffffffffffffff")
                .unwrap()
                .is_empty()
        );
        let result = snapshot(&items);
        assert_eq!(result.as_array().unwrap().len(), 1);
        let record = &result[0];
        assert_eq!(record["id"], 2);
        assert_eq!(record["status"], "Stopped");
        assert_eq!(record["uploaded"], 10);
        assert_eq!(record["seeded_seconds"], 20);
        assert!(record["size"].is_null());
        assert!(record["selected_files"].is_null());
        assert_eq!(record["live"], false);
        assert_eq!(torrent_ids(&[make(3)], "all").unwrap(), Vec::<u64>::new());
        assert!(torrent_ids(&[], "bad-hash").is_err());
    }
}
