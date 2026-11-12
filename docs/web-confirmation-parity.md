# Download confirmation parity

This is a focused audit of desktop download-admission prompts, not a claim of full
desktop/Web feature parity. Updated during the existing-torrent reuse work.

| Desktop case | Web behavior |
| --- | --- |
| Torrent files already in the destination (`ExistingFiles`, `dialogs.rs`) | Owner reviews the retained metadata/destination, acknowledges that nonmatching data may be replaced, then approves reuse or cancels. No disk modification before approval. |
| Same torrent already in the list | Open the existing entry instead of creating a second writer; existing Recheck/Resume controls remain available. |
| Duplicate Web address (`Duplicate`, `dialogs.rs`) | Owner chooses Open existing or Add numbered copy. Stale existing-entry choices are revalidated. |
| Desktop duplicate overwrite/delete-and-replace | Not newly exposed: no silent destructive default in the owner add flow. Legacy/extension settings behavior is unchanged. |
| Missing destination folder (`ensure_folder`) | Web categories use bounded server-relative folders created through validated management operations; no browser-native filesystem picker or arbitrary server path is introduced. |
| Remove entry versus delete files | Existing removal confirmation retains the explicit delete-files checkbox; Delete completed only removes history. |
| Move/rename collision | Existing web file tools refuse occupied paths; this work does not weaken those safeguards. |
| Paused source replacement / checksum / torrent Recheck | Existing owner controls remain available. Recheck is distinct from resuming. |

Pre-start torrent file selection and a general-purpose server file browser remain
future work. Reuse uses all files, just as the current remote add path does; existing
data outside the configured downloads root cannot be approved through this flow.

## Evidence

- 29 Chromium fixture groups, including consent, dismissal, explicit cancellation,
  expiry recovery and duplicate-link choices.
- 11 real-headless integration groups, including a torrent with one correct and
  one incorrect piece: only the correct piece is verified; original matching data
  remains unchanged. No claim that the offline test downloaded replacement pieces.
- Five focused Rust additions tests cover bounds/expiry, hash normalization,
  destination/path restrictions and queued-copy/sidecar reservations.
- The real-headless regression queues two copies before their files exist, checks
  distinct destination paths, then starts both and verifies distinct completed
  paths and correct contents. Pending move targets are also reserved by allocation.

Transient metadata/result limits and API details are in
[web-download-tools.md](web-download-tools.md). Linux/container runtime acceptance
remains deferred.
