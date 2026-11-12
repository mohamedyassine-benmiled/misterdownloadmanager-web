# Upstream integration: Web overlay and repeatable merges

Updated: 2026-11-12. **The separation pass is implemented locally; upstream has not
been merged. Changes are not yet committed or published.**

## Goal and architectural decision

Keep the original application and download engines as the shared foundation. Put
fork-specific behavior in separate modules, assets, tests and workflows, connected
through small, explicit adapters. Future upstream merges should primarily require
reviewing those adapters—not disentangling Web code from desktop dialogs.

This is a **modular overlay**, not a second downloader, a vendored source copy or a
promise of conflict-free merges. The adapter uses the real application's single-writer
loop, data model, queues and torrent host. Upstream changes to these contracts still
require adaptation and tests even when Git reports no textual conflict.

Do not rewrite pushed history, maintain a second copy of upstream engine code, or use
global `ours` merge rules to hide changes. Continue ordinary upstream merge commits.

## Refreshed snapshot

| Item | 2026-11-12 observation |
| --- | --- |
| Fork | `mohamedyassine-benmiled/misterdownloadmanager-web` |
| Fork HEAD | `197c00161c2a`; substantial additional uncommitted work |
| Upstream | `anefzaoui/misterdownloadmanager` @ `b35b42d4cab545fb63cba9765681ad4c7f67c4f2` |
| Common ancestor | `da019249c9dedbd5744917b097e506841ea9fa3d` |
| Commits since ancestor | 3 fork / 132 upstream |
| Upstream changed paths | 426 (`--no-renames` file accounting) |
| Committed-HEAD merge-tree preview, before this split | 6 conflicting files: CI, app, IPC, extension background/tests, English catalog |

The old 47-commit/five-conflict scan is obsolete. The committed preview above does
**not** include local UI/admission work or this separation pass. Use the worktree
audit below for local overlap estimates; use a real merge on an integration branch
after the work is explicitly saved for a definitive merge result. Do not compare
committed merge-tree counts with per-file worktree estimates as if they were equal.

## Implemented separation

### Application adapter

- `crates/app/src/app/web.rs`: allowlisted dispatch/schema and one `web::State` slot.
  New Web state goes here, not into more desktop `App` fields.
- `app/web/bridge.rs`: owner/local-recovery dispatch, capture status, busy guard and
  asynchronous torrent admission callbacks. `app.rs` retains calls, not HTTP or
  consent implementation.
- `app/web/additions.rs`: bounded results, held metadata, owner decisions, duplicate
  handling and destination reservations.
- `app/web/transfers.rs`: file/torrent management adapter using the existing engine.
- `app/web/texts.rs`: Web source/schema texts; `remote.rs` retains one collector hook.
- `crates/app/src/headless/web.rs`: fork CLI aliases. Existing `main.rs` startup routing
  stays untouched. `--web` enables headless Web mode; `--port` sets the HTTPS port;
  `--webhost` selects a local listener IP/adapter. `--headless`/`--listen` still work.

Keep the shared add primitive in `app.rs`: owner duplicate policy is an explicit
override; extension/legacy policy remains separate. Do not copy that primitive into
the Web layer to make the desktop file look smaller.

### HTTP/auth adapter

`crates/remote` already exists upstream; **the entire crate is not fork-owned**.

- `src/auth.rs` and `src/web_auth.rs`: fork owner authentication and management.
- `src/http/web.rs`: fork routes, v1 origin policy, browser/device authorization and
  allowlisted desktop artwork. `http.rs` retains route/policy hooks and shared parsing.
- `src/lib.rs` and `src/devices.rs`: necessary shared state, scope and recovery bridges.
- `web/`: intentional fork UI overlay. If upstream edits these assets, review its
  changes; do not automatically discard them because our UI is different.

Authorization hooks are mandatory security boundaries, not optional merge decorations.
An upstream merge must never restore token-only/anonymous owner administration,
accept owner flags from JSON IPC, or let capture devices access history/settings.

### Additive torrent API

`crates/torrent/src/inspection.rs` owns offline `inspect()` and its regression. The
upstream-owned `lib.rs` has only the module/export addition.

**Correction to the previous plan:** upstream's `files_from_bytes()` is a file-list
helper, not a replacement for all `Meta` fields. The Web adapter also needs the
normalized hash, piece length, raw metadata and validation. Do not delete `inspect()`
unless an upstream API supplies the entire contract safely.

### Browser extension

- `extension/remote.js`: private transport/cache state behind `MDMRemote.create()`.
  Dependencies on capture/status lifecycle are explicit callbacks.
- `background.js`: adapter load/call sites; native capture and media logic stay shared.
- Firefox loads the adapter before the background script; Chromium imports it from
  its classic service worker. The packer includes it in every variant, and package
  checks verify its bytes and loading order.
- `tools/extension/remote.test.mjs`: remote-only regressions. `test.mjs` keeps its
  upstream tests and passes an explicit harness object at one call site, away from
  the frequently appended test tail. The loader follows actual browser script order.

Preserve public-address-only remote capture, no browser-secret forwarding, no POST
to GET conversion, no ambiguous retries, and no native fallback after token revocation.

### Catalogs, CI and image releases

- `tools/i18n/web_sources.py`: fork schema/HTML/JS collector. The upstream extractor
  retains recursive module discovery and a collector call.
- `i18n/en.tsv`: regenerate, never hand-merge. Preserve reviewed non-English rows.
- `.github/workflows/web.yml`: fork regression/deployment checks, also reusable by
  the image workflow. Desktop CI retains only its build-cache safety adjustment.
- `.github/workflows/web-image.yml`: Docker build, smoke gate, GHCR publication and
  image receipt. `.github/workflows/web-release.yml` now owns the separate `web-v*`
  release: both native server platforms, both fork extensions, checksums and the image
  must succeed before the GitHub downloads release is made public. Desktop `v*`
  releases/store publishing remain separate.
- `docker/web.Dockerfile`: fork entry point/healthcheck. Root `Dockerfile` is back to
  the common ancestor's content; the fork's Compose build uses the separate file.
- `tools/docker/`: tested image naming/publication policy and isolated runtime smoke.
- `tools/web-release/`: fork archive packaging, extension version/compatibility
  metadata, source agreement, public-file inventory and coordinated-release checks.
- Root `README.md`, `brand/` and `docs/images/webmister/`: intentional fork identity
  and documentation overlay. Preserve Webmister attribution/setup instructions while
  incorporating relevant upstream behavior changes; do not restore upstream's entire
  marketing README over it. The new logo does not replace desktop engine artwork.

See [web-releases.md](web-releases.md) for parameters, image tags and first-publication
requirements. Adding a workflow is not evidence that GitHub built or published an image.

## Ownership ledger and audit

[`tools/upstream/boundaries.json`](../tools/upstream/boundaries.json) distinguishes:

1. **Fork-only paths**: modules absent from the original base. An upstream addition
   at one of those paths is a collision to review, not permission to keep ours blindly.
2. **Overlays**: deliberately replaced shared assets, currently the Web UI.
3. **Bridges**: upstream-owned files with a documented reason for fork changes.
4. **Generated paths**: regenerate or resolve dependencies using their owning tools.
5. **Review**: unclassified paths; inspect them rather than silently treating them as fork-owned.

Commands (no commits, staging, checkout or automatic fetch):

```sh
python tools/upstream/audit.py --check
python -m unittest discover -s tools/upstream -p 'test_*.py'
python tools/upstream/audit.py --upstream upstream/main --preview-worktree
python tools/upstream/audit.py --upstream upstream/main --preview-worktree --json
```

`--check` works in shallow CI without an upstream remote. It detects lost adapter
hooks and known implementation leakage; it is **not** a behavioral/security proof.
The report includes tracked uncommitted changes and non-ignored untracked paths.
Ignored private runtime/test data is not included.

The preview uses temporary per-file three-way text merges. It normalizes checkout
CRLF and reports binary/add/delete cases for manual review. It does **not** model
renames, custom merge filters, file modes, directories or semantic compatibility.
Tests verify it leaves HEAD, index and worktree status unchanged.

## Repeatable upstream-update procedure

### 1. Preserve work and pin the target

- Review `git status`, including untracked files. Do not merge in a dirty worktree,
  stash private data automatically, or create a commit without owner authorization.
- When authorized, save all intended implementation files before starting integration;
  exclude `target/`, credentials, browser profiles, download data and local env files.
- Refresh refs with `git -c http.version=HTTP/1.1 fetch upstream --prune --tags`.
- Record `git rev-parse upstream/main` and use that exact SHA throughout the attempt.
- Run the ownership/overlap audit and read upstream's release notes and shared API diff.

### 2. Integrate on a dedicated branch/worktree

After the saved commit and clean-tree check, create an integration branch/worktree
from the saved fork revision. Never merge directly into the active release branch.

```sh
git worktree add -b integration/upstream-YYYYMMDD ../mdm-upstream-YYYYMMDD HEAD
# In that worktree, using the previously recorded SHA:
git merge --no-ff --no-commit <upstream-sha>
```

Resolve each bridge deliberately. Keep upstream changes to the shared engine/runtime,
then adapt the fork callbacks to its new contract. Retain Web behavior and safeguards.
Do not use whole-file `ours`/`theirs` on shared files or an automatic merge driver for code.

For new collisions, update the ledger and regression tests. Do not simply increase
allowlists until a guard becomes green.

### 3. Regenerate and verify

```sh
python tools/i18n/extract.py
# MDM_WRITE_SCHEMA=1 cargo test -p mdm-ipc schema  (set the env var for your shell)
cargo fmt --all
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/i18n/extract.py --check
python tools/upstream/audit.py --check
node tools/extension/test.mjs
node tools/extension/check-pack.mjs  # after tools/extension/pack.ps1
node tools/web/test.mjs
node tools/web/headless.mjs
```

Use the repository-local Rust/browser environment described in
[web-development.md](web-development.md) on the current Windows host. Shut down the
private review process cleanly before replacing its executing binary; preserve its
data directory and owner account. Never include isolated authentication fixtures in a release.

Run the Docker workflow/smoke gate on Linux before publishing its image. Broader
Linux/Docker/Compose/systemd acceptance and real Firefox testing remain separate gates;
passing Windows tests does not establish either.

### 4. Review and promote

- Inspect schema/catalog changes and preserve reviewed translations.
- Exercise desktop/local extension behavior as well as the Web path.
- Test migration against **copies** of existing state, not the user's live data.
- Refresh the desktop-to-Web parity ledger for newly imported capabilities.
- Only after review and explicit authorization, finish the merge commit and promote it.
  Publish `web-v*` only after that integrated revision's checks pass.

If integration fails, leave the integration worktree/branch available for diagnosis.
`git merge --abort` is appropriate only in that dedicated worktree if it had no
pre-existing changes. Never reset the original working tree or delete a branch with
unreviewed work. Rolling an image back also requires checking state-format compatibility;
keep a pre-upgrade `/config` backup.

## First integration: known semantic review queue

These remain work for the **actual upstream merge**, not claims that the split solved them:

- [ ] Preserve upstream `stopped_by_hand`/start/stop and retry behavior alongside the
      fork's in-progress-move guard. Review deferred rename and multi-selection moves.
- [ ] Reuse shared move helpers only if they preserve exclusive/no-overwrite semantics,
      sidecars and root constraints. An auto-merging helper is not automatically safe.
- [ ] Preserve new native quality-menu/video IPC fields, `wants_reply()` cases and
      reply-timeout semantics alongside internally skipped owner request fields.
- [ ] Reconcile upstream extension capture/media/Safari changes with adapter loading,
      status/rules callbacks and test harness additions. Do not add Safari support claims
      merely because upstream has a Safari manifest.
- [ ] Review torrent `Host`/`Act`, session identity and seeding/recheck reporting against
      Web payloads. Preserve full offline metadata inspection, consent and queued intent.
- [ ] Keep server/extension default capture types equal after new upstream defaults.
- [ ] Review the new engine-backed HLS/DASH workflows rather than assuming our old
      media/gallery adapter automatically understands new transfer containers.
- [ ] Reconcile dependencies/lockfile and packaging with the pinned upstream toolchain.
- [ ] Regenerate English/schema; verify no owner secrets reach API/schema/logs.

## Boundaries for subsequent feature work

- Put new Web features/tests in fork-owned modules, not desktop menu handlers.
- Add an upstream-file hook only when necessary; document its contract in the ledger.
- Share real engine operations; do not fork business logic for a superficial clean diff.
- Keep Web image/release jobs in fork-owned workflows and paths.
- Treat newly changed upstream-owned files as an explicit maintenance cost in review.
- Do not call the fork "always mergeable" until the pinned integration build and both
  desktop/Web regressions have passed. File separation lowers cost; it cannot freeze upstream.

## Validation of this separation pass

Local evidence (Windows, current worktree; not a merged upstream build):

- 79 native/remote extension checks; Chromium/Firefox packaging checks.
- 29 Web browser fixture groups and 11 real-headless integration groups, including
  new startup flags, legacy restart flags, preserved state and native-host-free capture.
- 9 filtered app/headless Web tests; 39 IPC tests (including owner-flag injection and
  legacy result compatibility), 18 remote unit tests, 4 HTTPS server tests and the
  offline torrent-inspection regression.
- 4 audit tests and 6 image-publication policy/output tests.
- Affected-crate all-target Clippy, formatting, catalog checks, actionlint and Bash
  syntax checks passed.

The private review instance was cleanly restarted with `--web --webhost 127.0.0.1
--port 18463`, retaining its existing data and owner account. Docker execution, GHCR
publication, real Firefox runtime and the actual upstream merge are **not validated**
by those local results.
