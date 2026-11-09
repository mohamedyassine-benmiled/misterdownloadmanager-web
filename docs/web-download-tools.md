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

- Choose at least one file, set sequential/first-last-piece order and a per-torrent upload limit.
- Ask for **Recheck** or **Reannounce**, and inspect the asynchronous outcome in Activity.
- List existing trackers and add bounded HTTP/HTTPS/UDP tracker addresses; the engine's tracker
  filtering still applies. Tracker removal/replacement is not implemented by this increment.
- Inspect connected peer addresses/clients and received/uploaded bytes.

The web file editor is bounded to 10,000 entries; larger metadata is explicitly read-only there.
The engine currently supports selection and piece order, not arbitrary per-file priority levels.
Initial torrent addition still resolves metadata asynchronously and chooses all files; selection
can be changed afterward. A pre-start metadata/file-picker flow remains future work.

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

Application rejections currently use the compatibility response envelope with `error` and
`code`, sometimes under HTTP 200; clients must check both HTTP status and the JSON error.
An `ok`/`pending` acknowledgment means a request was scheduled, not that its disk/torrent
operation has succeeded or become a durable job. Poll details and inspect Activity for outcomes.

Linux, Docker and systemd runtime acceptance is intentionally deferred to
[linux-server-testing.md](linux-server-testing.md).
