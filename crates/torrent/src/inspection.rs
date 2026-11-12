//! Offline inspection needed by the Web adapter. No session, tracker or peer access.
//! Kept additive: a file-list-only upstream helper is not a replacement for Meta.
use super::*;

pub fn inspect(torrent: &[u8]) -> Result<Meta, TorrentError> {
    if torrent.len() as u64 > MAX_TORRENT_FILE {
        return Err(TorrentError::NotATorrent);
    }
    let parsed = librqbit::torrent_from_bytes(torrent).map_err(|_| TorrentError::NotATorrent)?;
    let info = parsed.info.data.validate().map_err(engine)?;
    safe_names(info.iter_file_details())?;
    Ok(Meta {
        name: info
            .name()
            .map(|n| n.into_owned())
            .unwrap_or_else(|| parsed.info_hash.as_string()),
        info_hash: parsed.info_hash.as_string(),
        files: info
            .iter_file_details()
            .map(|f| FileEntry {
                path: f.filename.to_pathbuf(),
                size: f.len,
                padding: f.attrs().padding,
            })
            .collect(),
        piece_length: info.lengths().default_piece_length(),
        torrent: torrent.to_vec(),
        peers: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn torrent_naming(name: &str) -> Vec<u8> {
        let mut t = b"d4:infod5:filesld6:lengthi5e4:pathl6:folder".to_vec();
        t.extend(format!("{}:{name}", name.len()).bytes());
        t.extend(b"eee4:name4:test12:piece lengthi16384e6:pieces20:");
        t.extend([0u8; 20]);
        t.extend(b"ee");
        t
    }

    #[test]
    fn held_metadata_is_inspected_without_a_session() {
        let meta = inspect(&torrent_naming("fine.bin")).unwrap();
        assert_eq!(meta.files.len(), 1);
        assert_eq!(meta.files[0].size, 5);
        assert!(meta.files[0].path.ends_with("fine.bin"));
        assert_eq!(meta.info_hash.len(), 40);
        assert!(meta.peers.is_empty());
        assert!(inspect(b"not a torrent").is_err());
        let unsafe_meta = inspect(&torrent_naming("../escape"));
        assert!(
            unsafe_meta.is_err(),
            "unsafe metadata was accepted: {unsafe_meta:?}"
        );
    }
}
