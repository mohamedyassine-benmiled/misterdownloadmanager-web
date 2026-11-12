# Web fork: development status

Implementation follows [plan.md](../plan.md). This page separates shipped source changes from
runtime verification; it is not a completed-feature announcement.

## Implemented core

- Extension server addresses retain their port and optional proxy prefix. Unsafe configuration
  URLs are rejected, requests omit cookies and refuse redirects, and timeouts include/abort
  response bodies.
- The popup checks authenticated status and opens the configured web UI.
  Revocation keeps the destination but discards its token; no implicit local fallback occurs.
- Capture-only extension tokens read server rules from `/api/status`, without reading download
  history or depending on a native host. Old servers fall back to `/api/list` only on a 404.
- Valid download acceptance is checked before canceling the browser transfer. Forms/POST and
  URLs containing credentials are not converted into anonymous remote GETs.
- Compose is configurable through `docker/.env.example`; a dedicated-user systemd service and
  environment template are in `deploy/systemd/`. Local deployment data is excluded from Git and
  Docker's build context.
- Single-owner setup/login, Argon2id passwords, protected cookie sessions, CSRF, credential
  changes, idle/absolute expiry, persistent temporary bans and local recovery are implemented.
- The Authentication screen includes username/password, optional localhost/subnet bypass,
  trusted proxies, failure threshold, ban duration and session timeout. Bypass does not grant
  owner administration. Legacy devices never become owners; new extension tokens are capture-only.
- The responsive embedded vanilla-JS workspace includes filters/sorting, bulk actions, details,
  per-download limits/descriptions, allowlisted application settings, custom categories,
  queue membership/schedules and device pairing/revocation, with themes, translation and RTL.
- Owner download tools now include bounded move/rename without overwriting, paused-link
  replacement and background checksum verification. Torrent details add saved file selection,
  piece order, upload limits, tracker additions, peer details and recheck/reannounce requests.
  See [web-download-tools.md](web-download-tools.md) for capabilities and deliberate limits.
- Management changes run on the existing single-writer application loop. Settings are allowlisted,
  configuration edits use revisions and category folders are constrained to the downloads root.
- `misterdownloadmanager-cli health` checks the application loop, listener state and auth storage;
  the Docker image declares it as its healthcheck. This is not an external TLS reachability probe.

## Verification

### Build prerequisites after the upstream 0.5.2 merge

Default engine builds now compile BoringSSL for the browser-like TLS connection. Install
CMake and LLVM's libclang; Windows also requires NASM on `PATH`. Ubuntu/Debian builders need
`cmake libclang-dev`. The fork Dockerfile and native/headless CI jobs provision these rather
than silently disabling the default engine feature. The native Linux release remains on
Ubuntu 22.04; browser fixture/headless jobs use Ubuntu 24.04 because Playwright 1.58.2 does not
support Ubuntu 26.04. Local `--no-default-features` checks do not validate a full release build.

The upstream-branded website still builds in this fork's Website workflow, but its Pages
deployment is upstream-only. No Webmister website or store publication is implied.

Run the portable extension regression suite from the repository root:

```sh
node tools/extension/test.mjs
```

It exercises the real background script against fake Chromium and Firefox APIs, including
remote pairing, prefix routing, connectivity, revoked/offline servers, response validation,
browser fallback and cancellation, secret omission, switching destinations and stalled bodies.
These are logic tests, not proof of a real browser's TLS/CORS or a running Rust server.

Latest local results (Windows, project-local Rust/Chromium):

- Extension logic: **79 checks passed**, including explicit empty capture rules and owner
  opt-in for inline file types (off by default).
- Chromium workspace fixtures: **20 checks passed**, including German and Arabic RTL mobile
  layouts, authentication controls, queue/category/settings forms, details and device revocation.
  Screenshots in `target/web-shots/` were inspected in desktop/mobile and light/dark modes.
  New checks exercise file tools, torrent controls, preservation/discard of unsaved drafts and
  a delayed save reply arriving after a newer draft has been typed.
- The core baseline passed full `cargo test --workspace`. The file/torrent increment passes
  targeted `mdm-app`, `mdm-remote` and `mdm-torrent` suites, including owner-cookie/CSRF,
  capture-scope HTTPS, non-overwriting move and session-free metadata inspection tests. Some upstream desktop tests
  intentionally require an interactive graphics session; the website-build test is also ignored.
- Real headless application: **10 integration groups passed**. Chromium enrolls an owner;
  real API calls edit settings/categories/schedules; a capture-only device downloads a real file
  from a loopback source and verifies its bytes. Clean shutdown/restart preserves credentials,
  settings, queues, categories, devices and download details. Revocation, bans and local recovery
  are tested against that process, not a simulator.
  Added real checksum match/mismatch, move collision/rename, replacement-source download and
  saved-torrent selection/order/upload/tracker/recheck/reannounce checks, including restart persistence.
  Torrent cases exercise local saved metadata, not a production swarm.
- Real Chromium extension: the unpacked extension pairs through its popup and intercepts a
  browser download; the server verifies the file and the browser keeps no duplicate transfer.
  Revoking its token leaves the next download with the browser and retains the server destination.
  The test copy substitutes only an unregistered native-host name so it cannot contact or launch
  a locally installed MDM. Browser TLS errors are ignored only for the isolated loopback test.
- Workspace Clippy passes with `--all-targets -- -D warnings`; formatting and generated English
  catalog extraction pass. Chromium/Firefox/store extension packaging and package checks pass.
- Full Linux/container/systemd runtime validation and new CI job executions are still pending.
- UI finish review found a same-download draft-loss bug; regression coverage was added and
  its verdict pass scored that finding resolved. Documentation review found the new controls
  consistent with the incumbent design; no redesign or design-format migration was performed.

Install/run the browser tests from the repository root:

```sh
npm ci --prefix tools/web
npx --prefix tools/web playwright install chromium
node tools/web/test.mjs
cargo build --locked -p mdm-app -p mdm-cli
node tools/web/headless.mjs
```

The headless test uses a fresh `target/headless-tests/run-*` directory and loopback ports;
it never connects to your existing instance. Set `MDM_TEST_APP` for a different binary path.
The test deliberately accepts its isolated server's self-signed certificate; this is not a
deployment TLS recommendation. Test directories contain authentication state: do not publish
them as artifacts. Only synthetic workspace screenshots are uploaded by CI.

Use `PLAYWRIGHT_BROWSERS_PATH` if Chromium was installed to a nondefault location. The local
Windows installation uses `target/web-tests/browsers`. For a synthetic workspace preview:

```sh
node tools/web/preview.mjs
```

Open `http://127.0.0.1:8767/mdm/`; all transfers and credentials there are explicitly fake.
For popup interaction/layout checks without installing an extension:

```sh
node tools/extension/preview.mjs
```

Open `http://127.0.0.1:8766/?state=connected&lang=en`. Supported fixture states are `connected`,
`offline` and `revoked`, with `en`, `de` and `ar` locales. The mock pairing code is `12345678`;
no token, native host or real server is used. This development-only server serves an allowlist of
popup assets and binds to loopback. It is not included in the browser extension package.

The initial Windows checkout exposed a CRLF-sensitive source-contract assertion in that suite.
The assertion now normalizes line endings before parsing; no test was removed or relaxed.

Docker and WSL are unavailable on the development host. Do not mark deployment checks complete
until they run on Linux:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
docker compose --env-file docker/.env -f docker/compose.yml config --quiet
docker compose --env-file docker/.env -f docker/compose.yml up -d --build
systemd-analyze verify deploy/systemd/mdm.service
```

Also run real Chromium and Firefox remote cases through `tools/extension/e2e.mjs` after building
the binaries and packing the extension (see that script's prerequisites). Verify native systemd
and Compose startup without a display environment, ownership, mounted shares, shutdown,
restart/resume, certificate trust and extension operation with no native host installed.

## Remaining work (not full feature parity)

The target, staged implementation plan and acceptance gate for complete desktop
functional coverage are in [web-functional-parity-plan.md](web-functional-parity-plan.md).

Linux/Docker/Compose/systemd testing is deferred by request; the later server acceptance
checklist is [linux-server-testing.md](linux-server-testing.md). Application features come first.
The initial modular separation pass is now implemented locally; integrating the current
upstream release remains separate. See [upstream-merge-plan.md](upstream-merge-plan.md)
for adapter ownership, audits and the safe merge procedure. Server flags and the fork-owned
Docker/GHCR release workflow are described in [web-releases.md](web-releases.md).

- Advanced media/gallery inspection and jobs; pre-start torrent metadata/file picker, tracker
  removal/replacement and advanced priorities beyond engine-supported selection/piece order.
- Site-login/debrid account management and deliberate server-secret storage. On Linux without
  a keychain, existing secrets rely on a private data folder, not encryption against host access.
- Built-in category editing, extraction password prompts, quota status/diagnostics and broader
  server controls beyond the implemented settings allowlist. File tools are implemented for
  plain downloads, not aggregate media/gallery/debrid containers or partial-file relocation.
- Persistent asynchronous capture jobs/idempotency and lost-acknowledgment reconciliation.
  A valid acceptance ID is not yet a durable capture transaction.
- Backup/restore UI, typed published HTTP contract, large-history pagination/virtualization,
  server audit events, more inline field errors and SSE (current updates use bounded polling).
- Explicit outbound destination/SSRF policy, including redirects, DNS and metadata endpoints.
  Authenticated users/devices can currently ask the server to fetch private-network sources;
  restrict pairing and network access accordingly. Do not expose this release to the internet.
- Actual Firefox extension-to-server tests with no native host installed, production certificate
  trust/CORS, reverse-proxy/prefix acceptance, Linux service/container lifecycle and mounted shares.
- Publish registry images only after Linux/container verification passes.
