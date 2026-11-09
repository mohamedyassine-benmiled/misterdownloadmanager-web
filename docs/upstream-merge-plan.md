# Upstream merge plan (future task)

Status: **planned, not started.** This document records a scan of the fork against the
official application and a plan to keep future merges clean. It does not claim that any
merge, refactor or deployment test has been performed.

Scan date: 2026-11-09. Working tree at the time: clean.

## Snapshot

| Item | Value |
| --- | --- |
| This fork | `mohamedyassine-benmiled/misterdownloadmanager-web` @ `18b7c3f` |
| Official upstream | `anefzaoui/misterdownloadmanager` @ `605a0c74` (Version 0.5.2) |
| Common ancestor | `da01924` (A finished file that was moved or deleted is no longer a duplicate) |
| This fork's work | 1 squashed commit, 50 files, +6659/−808 |
| Upstream work | 47 commits, 250 files |
| Merge simulation | **5 conflicting files**, 7 other overlapping files auto-merge |

The simulation was read-only:

```sh
git remote add upstream https://github.com/anefzaoui/misterdownloadmanager.git
git -c http.version=HTTP/1.1 fetch upstream --prune --tags
git merge-tree --write-tree --messages HEAD upstream/main
```

`http.version=HTTP/1.1` was needed once because the Windows Schannel backend failed with
`SEC_E_DECRYPT_FAILURE` during the first fetch.

## Conflicting files

Measured with `git merge-tree`; conflict sizes are hunks, not line counts.

| File | Fork change | Upstream change | Conflict | Resolution |
| --- | ---: | ---: | --- | --- |
| `.github/workflows/ci.yml` | +49 | +8 | 1 hunk: both append jobs at the same point | Move the fork's jobs to a new workflow file |
| `crates/app/src/app.rs` | +71 | +747 | 1 hunk: 3 lines in `start()` | Keep both guards (see below) |
| `extension/background.js` | +197 | +141 | 1 hunk: rules fallback for a remote destination | Reconcile by hand; then extract `remote.js` |
| `i18n/en.tsv` | +580 | +88 | 6 hunks: both inserted generated rows | Regenerate with `tools/i18n/extract.py` |
| `tools/extension/test.mjs` | +215 | +282 | 1 hunk: both append tests at end of file | Move the fork's tests to a new file |

Already auto-merging (no conflict): `crates/ipc/src/lib.rs`, `crates/torrent/src/lib.rs`,
`extension/popup.js`, `tools/extension/e2e.mjs`, `Cargo.lock`, `ROADMAP.md`.

## Semantic overlaps (the real work)

These are not textual conflicts but features built twice. Handle them during the merge.

1. **Move/Rename.** Upstream `9acad274` makes Move/Rename work on several selected
   downloads and adds `crates/app/tests/move_many.rs`. The fork's
   `crates/app/src/app/web/transfers.rs` has its own `move_exclusive` and `same_location`.
   After merging, reuse upstream's move helpers instead of keeping two implementations.
2. **Torrent file list.** Upstream `515cd55d` adds `crates/app/src/torrentfiles.rs` and
   `mdm_torrent::files_from_bytes()`. Delete the fork's `mdm_torrent::inspect()` (added in
   `crates/torrent/src/lib.rs`) and use upstream's function for the web file list. Keep the
   stored `TorrentSpec.hash` as the identity check.
3. **Capture defaults.** Upstream `8809a5aa` greatly expands the default browser file types.
   The extension's fallback list and the server's `/api/status` capture rules must both use
   the new defaults; the existing test that keeps them equal must still pass.
4. **App hooks.** Upstream changed `start()` (`stopped_by_hand`), `act()`, the torrent
   `Host`/`Act` types and capture code. The fork's guards for an in-progress file move must
   coexist with those changes.
5. **Torrent session details.** Upstream `8d319e6d` and `515cd55d` change seeder/share-ratio
   and recheck reporting. Confirm the web details payload still reads the right fields.

## Split plan

Goal: keep fork-only code in files upstream does not edit, leaving only small, stable hooks
in upstream-owned files. Work through the steps in order.

### Step 1 — CI workflow (low risk)

- [ ] Move the headless integration step, the `web` job and the `deployment-config` job from
      `.github/workflows/ci.yml` into a new `.github/workflows/web.yml`.
- [ ] Remove the fork's cache-path change from `ci.yml`, or keep the cache configuration in
      the new workflow only.
- [ ] Confirm both workflows pass `actionlint`/YAML validation and that the new workflow has
      the same triggers as `ci.yml`.

Result: `ci.yml` no longer conflicts.

### Step 2 — Extension tests (low risk)

- [ ] Move the fork's remote-mode assertions from `tools/extension/test.mjs` into
      `tools/extension/remote.test.mjs`.
- [ ] Share the existing harness (`fakeApi`, `load`, `ask`, `settle`, `requests`, `served`,
      `ok`, `passed`, `complaints`) through one explicit import or a single documented
      harness object.
- [ ] Add exactly one hook at the end of `test.mjs` to run the new file.
- [ ] Keep `node tools/extension/test.mjs` as the entry point; assert the same check count.

Result: `test.mjs` conflicts only in a one-line hook, if at all.

### Step 3 — Translation catalog (mechanical)

- [ ] Treat `i18n/en.tsv` as generated. Never hand-merge it.
- [ ] After the merge, run `python tools/i18n/extract.py` and commit the result.
- [ ] Run the CI check (`extract.py --check`, as used by the workflow) to confirm the
      catalog is current.
- [ ] Preserve upstream's reviewed translator rows in other languages; the extractor only
      regenerates English sources.

Result: `en.tsv` conflicts disappear after regeneration.

### Step 4 — Extension remote client (medium risk)

- [ ] Move the fork's remote client from `extension/background.js` into a new
      `extension/remote.js`: `remoteTarget`, `remoteAddress`, `remoteCall`, `noteRemote`,
      `remoteProblem`, `remoteInfo`, `fetchRemoteInfo`, `remoteCheck`, `remoteSend`,
      `remotePair`, `remoteOff`.
- [ ] Expose a single `globalThis.MDMRemote` facade with the small dependencies it needs
      (`ext`, `withTimeout`, `showStatus`, `refreshRules`, `rules` get/set, `DEFAULT_TYPES`).
- [ ] Load it before `background.js` in both manifests:
      Chrome `background.service_worker` + top-level `importScripts("remote.js")`;
      Firefox `background.scripts: ["remote.js", "background.js"]`.
- [ ] Keep the server-destination rule intact: a revoked token must never fall back to a
      local MDM, and a remote POST must never become a GET.
- [ ] Re-run the extension suite and the real Chromium/Firefox cases.

Result: `background.js` edits shrink to call sites; future upstream changes to capture and
video code do not touch the remote client.

### Step 5 — Application hooks (low risk, optional)

- [ ] Keep the remaining `app.rs` hook minimal: one `self.web_busy(id)` guard in `start()`
      and the existing `act()` skip.
- [ ] Keep `web_checks`, command dispatch and management operations inside
      `crates/app/src/app/web.rs` and `app/web/transfers.rs`.
- [ ] Do not move headless startup out of `main.rs`; upstream owns that file.

Result: at most one trivial `app.rs` conflict.

## Merge procedure

Run the split steps first, commit, then merge. Do not rebase the fork's pushed commit.

```sh
git switch -c upstream-merge
git merge --no-ff upstream/main
# resolve the remaining hunks (expected: app.rs, maybe background.js)
python tools/i18n/extract.py
cargo fmt --all
cargo check --workspace
```

Resolve `app.rs` by keeping both lines:

```rust
self.stopped_by_hand.remove(&id);      // upstream
if self.web_busy(id) { return; }       // fork
```

## Verification after the merge

- [ ] `cargo test --workspace` (non-ignored tests).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] `cargo fmt --all --check`.
- [ ] `node tools/extension/test.mjs` — keep the existing check count plus the remote file.
- [ ] `node tools/web/test.mjs` — all fixture checks, including file/torrent controls.
- [ ] `node tools/web/headless.mjs` — real HTTPS/cookies/Chromium/extension cases.
- [ ] `python tools/i18n/extract.py --check`.
- [ ] Inspect the regenerated `i18n/en.tsv` diff for lost or duplicated rows.
- [ ] Confirm the web UI still shows torrent files/peers and that move/rename does not
      overwrite existing files.
- [ ] Confirm the extension still refuses to fall back to a local MDM after revocation.

## Rollback

The merge lives on its own branch until verification passes. If it fails:

```sh
git switch main
git branch -D upstream-merge
```

Nothing on `main` or `origin/main` changes until the merge is pushed.

## Out of scope

- Linux/Docker/Compose/systemd runtime acceptance: deferred to
  [linux-server-testing.md](linux-server-testing.md).
- Rewriting the fork's history into per-feature commits; file separation is what makes
  future merges clean, not commit granularity.
- Adopting upstream's desktop-only features into the web UI unless they are exposed by the
  application loop.
