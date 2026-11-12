//! Small integration points called from upstream-owned app.rs.
//! Keep owner policy, recovery and admission outcomes here, not in desktop command handlers.
use super::super::*;
use super::additions::{PendingTorrent, source_hash};

impl App {
    pub(in crate::app) fn web_busy(&self, id: u64) -> bool {
        self.moving.iter().any(|(moving, _, _)| *moving == id)
    }

    /// Before the legacy API gate: local recovery or internally authenticated owner commands.
    /// Returning None leaves ordinary desktop/IPC/device behavior to the existing dispatcher.
    pub(in crate::app) fn web_answer(&mut self, cmd: &ipc::Command) -> Option<String> {
        use ipc::api::{ErrorAnswer, line};
        if !cmd.remote && cmd.query.as_deref() == Some("web-health") {
            let ready = self.remote.as_ref().is_some_and(|r| {
                matches!(r.status(), mdm_remote::Status::Listening(_))
                    && r.auth_recovery("status").is_ok()
            });
            return Some(if ready {
                serde_json::json!({ "ready": true }).to_string()
            } else {
                line(&ErrorAnswer {
                    error: "web server is not ready".into(),
                    code: Some("not-ready".into()),
                })
            });
        }
        if !cmd.remote
            && let Some(operation) = cmd
                .query
                .as_deref()
                .and_then(|q| q.strip_prefix("web-auth:"))
        {
            return Some(
                match self
                    .remote
                    .as_ref()
                    .ok_or_else(|| "remote-off".to_owned())
                    .and_then(|r| r.auth_recovery(operation))
                {
                    Ok(value) => value.to_string(),
                    Err(code) => line(&ErrorAnswer {
                        error: code.clone(),
                        code: Some(code),
                    }),
                },
            );
        }
        cmd.web_request.as_ref().map(|request| {
            if cmd.remote && cmd.web_owner && self.settings.remote_on {
                self.web_request(request)
                    .unwrap_or_else(|code| serde_json::json!({ "error": code, "code": code }))
                    .to_string()
            } else {
                line(&ErrorAnswer {
                    error: "owner required".into(),
                    code: Some("owner-required".into()),
                })
            }
        })
    }

    /// Called only after the existing remote/API authorization gate, not from web_answer.
    pub(in crate::app) fn web_capture(&self) -> String {
        serde_json::json!({
            "api_version": 1, "connected": true,
            "rules": { "types": self.settings.auto_types, "capture": true,
                "excludedSites": self.settings.excluded_sites, "excludedAddresses": self.settings.excluded_addresses,
                "opened": self.settings.capture_opened, "minKb": self.settings.capture_min_kb }
        }).to_string()
    }

    /// Err is an immediate, serialized API answer (duplicate or busy), not a task failure.
    pub(in crate::app) fn web_begin_torrent(
        &mut self,
        source: &mdm_torrent::Source,
    ) -> Result<String, String> {
        if let Some(hash) = source_hash(source)
            && let Some(existing) = self
                .items
                .iter()
                .find(|i| i.torrent.as_ref().is_some_and(|t| t.hash == hash))
        {
            return Err(
                serde_json::json!({ "id": existing.id, "pending": false, "duplicate": true })
                    .to_string(),
            );
        }
        self.web.additions.begin().map_err(|code| {
            ipc::api::line(&ipc::api::ErrorAnswer {
                error: "too many torrent additions are pending".into(),
                code: Some(code.into()),
            })
        })
    }

    pub(in crate::app) fn web_finish_torrent(
        &mut self,
        result: Result<mdm_torrent::Meta, String>,
        category: Option<Category>,
        queue: Option<usize>,
        from_magnet: bool,
        request_id: &str,
        queue_name: Option<String>,
    ) {
        let meta = match result {
            Ok(meta) => meta,
            Err(e) => {
                tracing::warn!("remote control: a torrent could not be read: {e}");
                self.web.additions.finish(
                    request_id,
                    serde_json::json!({ "state": "error", "code": "torrent-read-failed" }),
                );
                return;
            }
        };
        if let Some(existing) = self
            .items
            .iter()
            .find(|i| i.torrent.as_ref().is_some_and(|t| t.hash == meta.info_hash))
        {
            self.web.additions.finish(
                request_id,
                serde_json::json!({ "state": "added", "id": existing.id, "duplicate": true }),
            );
            return;
        }
        let category = category.unwrap_or_else(|| crate::torrentdlg::category_of(&meta));
        let dir =
            crate::model::category_folder(category, &self.downloads, &self.settings.category_dirs);
        self.web_admit_torrent(
            request_id,
            PendingTorrent {
                meta,
                category,
                dir,
                queue,
                queue_name,
                from_magnet,
            },
            false,
        );
    }
}
