# Web download tools

The owner can open a download's name to inspect or change it. These controls run through the
authenticated application loop, not desktop dialogs. Extension capture/control tokens and
network-bypass sessions cannot use owner management operations.

## Regular files

Under download details:

- **Move or rename** changes a completed file's location/name, or a destination before data has
  arrived. The folder is relative to the server downloads root; empty means that root. Existing
  files, partial downloads and destinations used by another list entry are refused. Active or
  partially received unfinished files must not be moved through this workflow.
- **Replace expired link** changes a paused unfinished download's source, keeping its partial
  data and resume validators. The next start still checks that the source is the same file.
  Old per-request cookies, form bodies and logins are cleared; privacy status is retained.
  HTTP/HTTPS/FTP/FTPS URLs are supported, without embedded credentials.
- **Verify checksum** accepts MD5, SHA-1, SHA-256, SHA-512 or CRC32, optionally named with an
  algorithm prefix. Finished files are read on a background thread, with a visible match,
  mismatch or read-error result and the actual digest. For paused unfinished files, the expected
  hash applies on the next run. An empty value clears it. At most two web verifications run
  simultaneously; results are transient and bounded, while expected hashes are persisted.

Move/copy operations run in the background and report completion through the destination and
Activity log. They never replace an existing destination, including a destination created
after validation. Cross-filesystem copies are not durable transactions: interrupted copies may
leave an incomplete destination alongside the original. Inspect it before retrying.

These file tools deliberately exclude torrent, video, gallery and debrid-transfer containers.
Changing a category alone does not move existing files; use the explicit move action.

## Torrents

The details panel exposes the files in saved metadata even when a torrent is stopped; metadata
inspection does not itself start a session or contact peers. Live peer and piece data is shown
only when the torrent session has it, not as fabricated zero progress.

- Choose at least one file and set sequential/first-last-piece order. The torrent's
  upload limit is in the progress dialog's **Speed Limiter** tab, alongside its
  download limit (0 means unlimited).
- Ask for **Recheck** or **Reannounce**, and inspect the asynchronous outcome in Activity.
- List existing trackers and add bounded HTTP/HTTPS/UDP tracker addresses; the engine's tracker
  filtering still applies. Tracker removal/replacement is not implemented by this increment.
- Inspect connected peer addresses/clients and received/uploaded bytes.

The web file editor is bounded to 10,000 entries; larger metadata is explicitly read-only there.
The engine currently supports selection and piece order, not arbitrary per-file priority levels.
Initial torrent addition still resolves metadata asynchronously and chooses all files; selection
can be changed afterward. A pre-start metadata/file-picker flow remains future work.

The Add dialog closes after a request is accepted, including a torrent still awaiting
metadata. Acceptance is not a claim that the torrent has joined the list. The owner
workspace's **Torrent additions** section polls its admission result and reports
confirmed addition, an existing list entry, a request for confirmation, or a failure.

When torrent files already exist, the owner can review their destination and file
names and explicitly choose **Verify and use existing files**. The warning checkbox
starts unchecked: matching pieces are retained, while missing/damaged pieces are
downloaded again and nonmatching data at those torrent paths may be replaced.
Verification runs when the torrent starts; a queued torrent retains its queue and
waits for its scheduled/manual start. **Do not add** cancels without modifying files.
Closing the prompt without approving also leaves files untouched and retains a
Review action in Torrent additions until the request expires.

Consent applies to the held torrent metadata and configured destination, not arbitrary
paths supplied by the browser. Approval rechecks the folder, queue identity and
conflicts with other list entries; paths outside the downloads root are refused.
Files reserved by another download cannot be adopted, including its partial/state
files. These checks are not an atomic OS filesystem sandbox: do not let untrusted
local processes change destination symlinks/files while an approved transfer starts.

The same torrent already in the list opens its existing entry rather than creating
another writer. Hex and Base32 v1 magnet hashes are normalized for that check.
Recheck/Resume remain available for existing entries; rechecking does not itself
mean "resume now". Removing list history alone never deletes the files.

## Duplicate Web download links

The owner Add dialog asks before adding an address already in the list. Choose
**Open existing download** to inspect/resume that entry, or **Add numbered copy**
for a separate, safely named download. Dismissing the question creates nothing.
Destination allocation includes queued list entries, their partial/state sidecars,
torrent file paths and pending move targets, not only files already on disk.
This owner flow does not silently apply the desktop's destructive overwrite policy.
The desktop's explicit overwrite/delete-and-replace choice is not exposed here.
Existing extension/legacy capture policy remains unchanged.

## API notes

All operations below use `POST /api/v1/manage` with an authenticated owner cookie and
`X-CSRF-Token`. They cannot be issued through the capture-only extension API.

| `op` | Fields in addition to `op` |
| --- | --- |
| `details` | `id` |
| `download.move` | `id`, `name`, `folder` (relative directory) |
| `download.source` | `id`, `url` |
| `download.checksum` | `id`, `expected` (empty clears) |
| `torrent.update` | `id`, `files` (unique file indexes), `in_order`, `ends_first`, `upload_limit_kb` (0: unlimited) |
| `torrent.action` | `id`, `action` (`recheck`, `reannounce`, `add-trackers`); `trackers` is newline-separated for the last action |
| `torrent.add.status` | `request_id` from the pending add response |
| `torrent.add.confirm` | `request_id`, `approve_existing: true` |
| `torrent.add.cancel` | `request_id` |
| `download.add` | `url`, `category`, `queue`, `start`; after a duplicate question: `existing_id`, `duplicate_choice` (`existing` or `new`) |

Remote torrent add responses include `pending: true` and a `request_id` while
metadata/destination validation is running. The owner-only status operation returns
`state: pending`, `state: confirmation` with a destination, total existing-file
count and up to 20 preview names, `state: cancelled`, or `state: added` with a
download `id` (`duplicate: true` for an existing entry, `reused: true` after
approval). Failures return `state: error` with codes such as `destination-in-use`,
`torrent-destination-changed`, `torrent-read-failed` or `torrent-save-failed`.
Confirmation is owner-only and idempotent: repeating an accepted confirmation
returns its previous result, not another download. Unknown/expired results return
`torrent-add-result-unavailable`; check the list and logs before retrying.

An immediate duplicate-torrent result may return `id`, `pending: false` and
`duplicate: true` without a request id. `download.add` returns a
`confirmation: duplicate` response before choosing an existing Web entry or new
copy; clients must not treat that response as an accepted download. If the existing
entry changed/disappeared, the choice must be reviewed again rather than silently
creating something different.

Admission results are in-memory only, limited to 128 entries, retained up to 15
minutes and lost on server restart. Held metadata awaiting consent is additionally
limited to 64 MiB; awaiting-consent records are not evicted to admit new requests.
This is outcome/consent reporting, **not** durable capture or cross-restart retry
deduplication.
The browser tracks submissions made in its current page session; reloading does
not restore that tracking. Capture-only extensions do not gain management access.

Application rejections currently use the compatibility response envelope with `error` and
`code`, sometimes under HTTP 200; clients must check both HTTP status and the JSON error.
An `ok`/`pending` acknowledgment means a request was scheduled, not that its disk/torrent
operation has succeeded or become a durable job. Poll details and inspect Activity for outcomes.

Linux, Docker and systemd runtime acceptance is intentionally deferred to
[linux-server-testing.md](linux-server-testing.md).
