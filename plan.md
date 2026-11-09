# Mister Download Manager Web — implementation plan

## 1. Objective and agreed direction

Turn this fork into a self-hosted Mister Download Manager (MDM) server with a polished, responsive web application that controls all server-relevant application features. Downloads run on the server and continue when the browser closes.

Confirmed direction:

- **Single owner**, with authenticated browsers and separately paired extensions; no shared-user account system in the initial release.
- **qBittorrent-style Web UI authentication settings**: username/password, optional localhost and IP/subnet bypass, consecutive-failure bans, ban duration and session timeout. Keep these controls in a modern settings interface, not a copy of the desktop dialog.
- **Downloads-first workspace**, not a dashboard dominated by charts.
- Run completely headless, without a display server, desktop session, or GUI interaction.
- Support **Docker**, **Docker Compose**, and **native Linux with systemd** as first-class deployment methods.
- Let the browser extension use a configurable server URL to capture downloads and send them to that server, without requiring a local MDM installation.
- Keep MDM's Rust download engine, capabilities, license, and upstream attribution. This is an application fork, not a replacement download engine or a remote desktop wrapper.

This document is a plan, not a claim that all proposed features are implemented. Completed work and remaining verification are recorded below.

### Implementation progress — core workspace and authentication

Implementation was subsequently authorized. Completed source changes:

- Extension URL normalization/prefix preservation, actual connectivity checks on popup open, web-UI opening, and retained server destination after token revocation.
- No silent local fallback, no remote POST-to-GET conversion, validated acceptance replies and abortable request/body timeouts.
- Configurable Compose/environment examples and native Linux/systemd templates, with ownership and private-network setup documentation.
- Extension regression suite: **79 checks passing**, including existing local behavior, remote Chromium/Firefox, server-supplied capture rules, explicit empty/inline policy and legacy status fallback.
- Argon2id owner enrollment/login, protected cookie sessions and CSRF, password-confirmed security changes, temporary bans, subnet/proxy validation and local recovery.
- Separate capture-only extension pairing and legacy download-control devices; neither grants owner settings access.
- Versioned core management API with validated allowlisted settings, custom categories, queue membership/schedules and download details/limits/descriptions on the existing application loop.
- Owner-only plain-file move/rename, paused source replacement, background checksums, and torrent file selection/order/upload settings, trackers, peers and recheck/reannounce. Constraints are documented in `docs/web-download-tools.md`.
- Responsive embedded downloads-first workspace with filtering, sorting, selection/bulk actions, authentication, devices, settings, queues and categories; themes and RTL.
- Chromium fixture suite: **20 checks passing**, including new file/torrent controls, dirty-form preservation and delayed-save regression cases, plus inspected desktop/mobile light/dark and Arabic screenshots. Real-headless integration: **10 groups passing**, including checksum/move/source/torrent operations and persisted settings (`tools/web/headless.mjs`).
- Full Windows workspace tests and Clippy pass (non-ignored tests); Rust binaries build with locked dependencies. Linux-specific tests and runtime validation remain pending.
- CLI readiness command and Docker healthcheck. CI includes web fixture checks and real Linux headless integration (new CI jobs still require a runner execution).

The development host now has a project-local Rust toolchain and Chromium. Per the latest user instruction, Linux/Docker/Compose/systemd runtime testing is deferred until application features are finished; `docs/linux-server-testing.md` is the later server acceptance checklist. Merging the official application's later releases is a separate future task: the scan, conflict list and split plan are in `docs/upstream-merge-plan.md`. Advanced parity is not complete: media/gallery workflows, pre-start torrent selection, accounts/secrets, durable capture jobs/idempotency, backup/restore and SSE remain open. See `docs/web-development.md` for exact verification results and commands.

## 2. Repository findings: extend what already exists

The cloned repository already contains important foundations:

| Area | Existing implementation | Planned work |
| --- | --- | --- |
| Headless runtime | `crates/app/src/headless.rs`: `--headless`, persistent state, queues, scheduler, signal handling; drives `App::tick` using a bare egui context | Audit every feature for unattended operation; separate reusable application services incrementally |
| Remote server | `crates/remote/src/http.rs`: Axum HTTPS, pairing, bearer tokens, Host/Origin guards, embedded assets | Add a versioned, permission-aware management API and live updates |
| Web UI | `crates/remote/web/`: small mobile-oriented remote page with list/add/pause/start/remove and queue controls | Replace the limited remote surface with a complete downloads workspace |
| Application state | `crates/app/src/app.rs`, `settings.rs`, and `store.rs` | Expose validated application operations instead of directly editing storage |
| Contracts | `crates/ipc/src/api.rs`, `docs/automation-api.schema.json` | Extend typed contracts and publish the HTTP API contract |
| Extension | `extension/background.js`, `popup.*`, Chromium and Firefox manifests | Improve existing URL/pairing configuration, connectivity feedback, metadata, and capture reliability |
| Containers | Root `Dockerfile`, `docker/compose.yml` | Production hardening, configurable Compose, optional helper image, health checks and release images |
| Deployment docs | `docs/server.md`, `docs/remote-control.md` | Expand native Linux, proxy, security, backup and upgrade instructions |

Important limitations to address:

- The current remote interface intentionally permits only basic download operations, not settings or unrestricted folder access. Full control requires a new authorization boundary, not simply forwarding arbitrary IPC commands.
- Existing extension server mode sends only the URL and start flag to `api/add`; it does not forward browser cookies. It also normalizes configuration to the URL origin, losing any path prefix.
- A stored extension pairing token is currently treated as connectivity in several UI paths; server health and token validity need explicit checks.
- Existing remote behavior assumes local-network access and self-signed HTTPS. Public HTTPS/reverse-proxy support needs explicit design and tests; it is not already production-ready internet exposure.
- The existing container omits video/gallery helpers. Headless Linux may lack a system keychain, so stored server credentials require a deliberate protection and recovery policy.
- Root `PRODUCT.md` and `DESIGN.md` describe the marketing website, not this management application. Preserve that website and document the web app's own context separately during implementation.

## 3. Target architecture

```text
Browser web app ── same-origin HTTPS + authenticated live updates ─┐
                                                               │
Browser extension ── configured HTTPS URL + scoped device token ─┼─> Rust API
                                                               │      │
CLI / existing local IPC ────────────────────────────────────────┘      v
                                                           Application services
                                                                  │
                              HTTP/FTP engine, torrents, queues, scheduler,
                              video/gallery helpers, extraction, persistence
```

### Backend

- Keep Axum/Tokio and the existing Rust workspace; do not introduce a second backend or external database just to serve the UI.
- Initially retain the functioning `--headless` runtime. Isolate operations for downloads, queues, settings, accounts and capabilities behind services shared by the desktop and HTTP interfaces.
- Keep the runtime as the single writer of application state. HTTP handlers must not manipulate `downloads.json` or race the engine.
- Preserve atomic state/resume persistence. Add versioned migrations and a backup before changing persisted formats.
- Give every workflow a deterministic headless outcome: configured policy, explicit pending decision in the web UI, or an actionable error; never wait for an invisible desktop dialog.
- Longer term, remove UI dependencies from the server build where practical. A pure daemon binary is a follow-up architectural improvement, not a prerequisite that delays the first usable release.

### Frontend

- The core workspace uses the existing embedded vanilla JS/CSS in `crates/remote/web/`, separate from the marketing `website/`, to avoid a new production build dependency. React + TypeScript + Vite was the initial recommendation, not the implemented stack.
- Production assets are embedded by `mdm-remote`; app-specific design guidance is in `crates/remote/web/DESIGN.md`. A future framework migration must preserve the API/security boundary and existing tests.
- Production is one server process/container and one origin; Node is a build dependency only. No separate frontend server, CDN, or mandatory cloud service at runtime.
- Maintain translations and RTL support; do not regress existing localization when replacing the remote page.
- Currently uses bounded polling, paused in hidden tabs. Planned: an authenticated fetch-based SSE stream with reconnect/backoff and snapshot fallback. Do not put tokens in stream URLs.
- Bound event rates, coalesce frequent progress updates, support resynchronization after missed events, and stop background polling when unnecessary.

## 4. Complete application-control inventory

Before implementing screens, create a feature-parity checklist from desktop actions, `settings.rs`, scheduler/torrent modules, helpers, CLI and current remote behavior. Map every item to a service operation, HTTP permission, screen, test and headless policy.

| Feature group | Required web controls |
| --- | --- |
| Downloads | Add one/many URLs, start/pause/resume/retry/stop, bulk actions, rename/move where supported, refresh expired links, duplicate handling, history and removal |
| Download details | Source, destination, status, progress, speed, ETA, connections/segments, errors, checksums, retry history and supported per-download limits |
| Categories/storage | Create/edit/delete categories, file-type rules, destinations within allowed roots, disk-space warnings and collision policy |
| Queues | Create/edit/delete, reorder downloads, concurrency, start/stop, limits and queue membership |
| Scheduler | Start/stop windows, recurring schedules, timezone, download quotas and queue behavior; explicitly label daylight-saving behavior |
| Network | Global/per-download/per-host limits, connection counts, HTTP/FTP behavior, proxy/SOCKS, DNS and adapter binding where supported |
| Torrents | Magnets and `.torrent` upload, metadata progress, file selection/priorities, peers/trackers/pieces, seeding/ratio controls, ports, DHT, blocklists and limits where supported |
| Video/galleries | Server-side inspection, format/quality selection, playlists/gallery jobs, progress, helper capabilities and clear missing-helper errors |
| Extraction | Enable/configure supported extraction, password requests, output location and failures; constrain paths and resource usage |
| Accounts | Site logins and debrid accounts, add/update/remove/test, masked/write-only secrets and connection state |
| Integration/security | Capture rules, extension pairing, named devices, token scopes, revoke/rotate, advertised URL and listener status |
| Server operations | Version, health, storage, redacted logs, configuration backup/restore, pending decisions and restart-required settings |

“Control every part” means all features meaningful on a server, not remote access to the host desktop. Tray icons, desktop startup, OS shell integration, sounds and rendering backends must be marked desktop-only or replaced by a suitable server/web equivalent. Do not silently omit them or expose arbitrary shell commands. Application upgrades are performed through the chosen deployment method; the web UI reports version/status rather than replacing a running container binary.

## 5. Modern web UI and UX

### Visual direction

- Mode: **Operate**. The owner should immediately see what is downloading, what failed and what action is available.
- Preserve the recognizable MDM identity and logo, using blue for navigation/selection and meaningful state colors. Use restrained neutral work surfaces, crisp dividers and readable typography rather than copying desktop windows or marketing-page decoration.
- Provide light/dark/system modes, tabular transfer figures, consistent spacing, a coherent icon set and minimal purposeful motion. Respect reduced motion.
- Establish application-specific tokens and component guidance in `webui/DESIGN.md` during implementation; leave the marketing design system untouched.

### Workspace structure

- Left navigation: All downloads, Active, Queued, Paused, Completed, Failed, Categories and Queues; secondary links to Scheduler, Settings and Server.
- Main toolbar: prominent **Add download**, search, filters, sort and selection-aware bulk actions.
- Desktop download table: name, status/progress, size, speed, ETA, category and queue, with configurable columns and obvious row actions.
- Details side panel: overview, connections, files/torrent information and errors; opening it must preserve search, selection and scroll position.
- Compact status strip: total transfer rates, active count, free space and connection health. Charts are secondary and appear only when they help diagnosis.
- Add flow: paste URLs, upload/drop torrents or choose media formats, then choose destination/category/queue and start policy. Advanced options remain discoverable without overwhelming the default flow.
- Settings: grouped sections, labels/help text, field-level validation, dirty-state warning, save feedback and clear “applies now” versus “restart required” indicators.
- Extension/server setup: show the configured public URL, pairing/revocation steps and a connection test.

### Responsive behavior and quality bar

- Desktop uses a table/details layout; tablet can overlay details; mobile uses compact rows and a full-screen details view with touch-friendly actions.
- Support keyboard operation, visible focus, semantic tables/forms/dialogs, accessible status announcements and WCAG 2.2 AA contrast.
- Design first-run, empty, no-results, unknown-size, metadata-pending, long-name, permission-denied, partial bulk failure, disconnected, expired-token and server-restart states.
- Keep stale data visible with a clear disconnected indicator; never imply that a failed request succeeded. Avoid disruptive progress announcements every second.
- Validate small and large lists, long translations and Arabic RTL. Use pagination or measured virtualization for large histories without breaking keyboard access.
- Inspect real rendered desktop/tablet/mobile screens in both themes before accepting the UI; final visual composition remains a review checkpoint, not a claim settled by this plan.

## 6. Management API and security

### API work

- Add a versioned API under `/api/v1`; preserve existing `/api/*` routes as compatibility adapters while migrating the extension and remote page.
- Define typed request/response DTOs, stable IDs, structured errors, field validation, pagination, bulk-result semantics and a published OpenAPI contract.
- Proposed resource groups: downloads/details/actions, queues, categories, schedules, settings sections, capabilities, helpers, accounts, devices, pending decisions and server status/events.
- Return capabilities from the running server so unsupported OS features and missing helpers are explained rather than shown as broken controls.
- Settings endpoints expose allowlisted fields, not a raw serialized `Settings` object. Secrets are write-only and represented by “configured” state when read.
- Use revision checks for conflicting configuration edits and idempotency keys for additions/retries, especially extension requests whose acceptance response can be lost.
- Preserve a real server-side accepted download/job ID; torrent metadata discovery and other asynchronous work need a trackable job state.

### Authentication and authorization

- Extend existing one-time pairing as the initial single-owner bootstrap, then let the owner choose the Web UI username and password. Use username/password login for subsequent browser sessions; keep extension pairing separate. Obtain the owner enrollment code through local CLI/container logs; do not ship default credentials or an anonymous admin setup page.
- Browser sessions and extension tokens are separate, revocable identities. Introduce explicit permissions such as `downloads:read`, `downloads:write`, `settings:write` and `devices:manage`.
- Default extension permission is capture/add only, plus the minimum capabilities/rules it needs. Existing paired tokens must not automatically become administrators after upgrade.
- Provide owner recovery/revocation through local CLI, plus session expiry, rate limiting and an audit trail for sensitive changes. Require recent owner confirmation for high-risk operations.
- Preserve and test Host/Origin protections, DNS-rebinding protection, CSP and body limits. Explicitly support authorized extension requests/preflights without wildcard credentialed CORS or treating an extension origin as authentication.
- Keep tokens out of URLs/logs and never sync extension credentials to a browser cloud account. Define browser-session storage deliberately; cookie sessions need CSRF protection, and bearer sessions need strong XSS/storage safeguards.
- Restrict file operations to configured download roots; prevent traversal, symlink escapes and torrent/archive extraction escapes. Confirm deletion separately from removing a history entry.
- Treat outgoing download URLs as a server-request risk: validate schemes, redirects and resolved addresses; block metadata/control-plane destinations. Allow private-network download sources only through an explicit owner policy so legitimate NAS use remains possible.
- Do not expose arbitrary command execution, helper arguments, host shutdown or unrestricted file browsing. Sensitive optional integrations require narrow validation and explicit enablement.
- Define encrypted server-secret storage with a protected persistent key or a mounted secret; document fallback limitations, permissions and backup/recovery. Never imply that encoding or a key stored alongside ciphertext protects against full host compromise.

### Required Web UI authentication options

Implemented in the **Authentication** screen. Detailed behavior and release acceptance criteria are in [docs/webui-authentication.md](docs/webui-authentication.md); deployment validation remains in progress.

| Option | Default / expected behavior |
| --- | --- |
| Username | Chosen during owner setup; editable after password confirmation; no hardcoded default account |
| Password | Set during setup; masked change-password action; never return the stored password/hash to the UI |
| Bypass authentication for clients on localhost | **Off**; explicit opt-in, restricted to verified direct loopback connections |
| Bypass authentication for clients in whitelisted IP subnets | **Off**; separate opt-in with an initially empty IPv4/IPv6 IP/CIDR list |
| Ban client after consecutive failures | **5** failed login attempts per effective client IP |
| Ban for | **3600 seconds**; configurable temporary ban, visible units and expiry behavior |
| Session timeout | **3600 seconds of inactivity**; enforced server-side, not only by a browser timer |

- Validate network entries and numeric ranges server-side and show inline errors. Require explicit confirmation before allowing a catch-all such as `0.0.0.0/0` or `::/0`; do not prefill the screenshot's catch-all whitelist.
- Explain that bypass grants access to anyone able to connect from an allowed source, including other local users/processes. Never enable it automatically for Docker, a private network or an empty whitelist.
- A reverse proxy connecting through loopback must not make every visitor eligible for localhost bypass. Resolve client addresses only through explicitly trusted proxies; ignore spoofed forwarding headers from other peers.
- Keep Host/Origin checks, CSRF protection and API permissions in force for bypassed clients. Require a real owner login/current-password confirmation to change credentials, bypass rules or trusted proxies; bypass alone must not authorize these changes.
- Revoke browser sessions after credential changes, provide local CLI recovery/unban, keep extension device tokens separately scoped, and test proxy/NAT/IPv6 cases before release.

### Network deployment policy

- Default recommendation: LAN/VPN access with HTTPS. Retain self-signed TLS with fingerprint verification for private deployments.
- Add and document a hardened HTTPS reverse-proxy deployment only after authentication and proxy tests pass. Do not recommend forwarding the raw legacy port to the internet.
- Distinguish bind address, allowed hosts, advertised/public URL and trusted proxies. Never trust forwarded headers from arbitrary clients.
- Choose and test the proxy-to-backend TLS policy, including upstream certificate verification. Any optional plaintext backend listener must be private/loopback-only, not the public default.
- Support a reverse-proxy base path end to end, including assets, API, events, extension requests and redirect behavior; prefer root-domain deployment in the simplest guide.

## 7. Extension: configurable server destination

Improve the existing server mode rather than adding a parallel capture implementation.

### Configuration experience

- Explicit destinations: **This computer** and **Remote server**.
- Remote settings: server base URL (hostname, port and optional path prefix), pairing code, device name, **Test connection**, **Open web UI**, **Disconnect** and visible status/last error.
- Validate/canonicalize URLs while preserving the base path. Reject embedded credentials, query strings/fragments and unsupported schemes; require HTTPS except an explicitly documented loopback development exception.
- Display the actual destination host before pairing. Store the token locally and never send it to a different origin through redirects.
- Keep the configured URL on authentication failure and request re-pairing; do not silently switch to local capture or erase useful configuration.
- Migrate current `{url, token}` settings compatibly. Keep local native-messaging behavior available.

### Capture contract and reliability

- Send URL, safe filename/content metadata, source/referrer when appropriate, selected category/queue and start policy through a documented request schema. Send sensitive metadata only when required and permitted.
- Fetch server capture rules/capabilities rather than depending permanently on local defaults or a local native host.
- Support intercepted downloads, context-menu “Download with MDM”, manual URL sending, magnets/torrent handling where browser APIs allow, and detected media supported by the server.
- Cancel the browser's transfer only after durable server acceptance with a job/download ID. On rejection/unreachability, preserve the browser download and explain why.
- Reconcile ambiguous timeouts using idempotency rather than blindly submitting twice. Test service-worker suspension/restart and double-click submission.
- **Never forward cookies, Authorization headers, form bodies or site credentials by default.** Public/signed URLs work first; browser-session-bound or IP-bound links may not work remotely. Explain the limitation and retain a safe local/browser fallback. Any later credential-forwarding feature needs separate explicit per-site consent and security review.
- Keep DRM-protected media unsupported. Respect browser-store rules and clearly document browser/platform limitations.
- Test Chromium MV3 and Firefox independently, including permission requests, certificate trust, CORS/preflights, unavailable servers, revoked tokens and server mode with no native host installed.

## 8. Docker and Docker Compose

Build on the root `Dockerfile` and `docker/compose.yml`.

- Multi-stage frontend + Rust build; embed production UI assets and keep Node/build tools out of the final image. Pin compatible toolchains and use locked dependencies.
- Default unprivileged runtime; configurable numeric UID/GID, clear bind-mount ownership guidance and no privileged/host-network requirement.
- Persist `/config` (settings, history, resume/auth/TLS state and secret material) and `/downloads` (completed and partial files). Document any temporary/cache mount explicitly.
- Provide a minimal image and, if helper dependencies warrant it, an optional helper-enabled image with pinned yt-dlp/ffmpeg/gallery-dl/7-Zip support and license notices. Surface availability in the UI.
- Parameterize Compose using documented `.env.example` values for image/tag, UID/GID, bind address/port, public URL, allowed hosts, mounts and timezone. Do not commit credentials or a private-network example as a required fixed address.
- Add liveness/readiness checks that work without leaking credentials or disabling TLS verification; distinguish a healthy process from a ready command/engine loop.
- Set restart policy, graceful-stop timeout, log rotation and resource guidance. Verify SIGTERM preserves state and child helpers exit cleanly.
- Document optional torrent TCP/UDP ports independently of the web port, persistent seeding state and NAS/network-share permissions.
- Publish tagged GHCR images from the fork, starting with tested Linux amd64; add arm64 only after build/runtime tests pass. Avoid a mandatory database or message broker.
- Provide tested plain `docker build`/`docker run`, Compose build-from-source, Compose image-pull, and optional reverse-proxy examples.

Planned user workflows (validate and update the existing docs when implementing):

```sh
# Build and start with Compose.
docker compose -f docker/compose.yml up -d --build
docker compose -f docker/compose.yml logs mdm

# Issue a new enrollment code using the existing CLI entry point.
docker compose -f docker/compose.yml exec mdm misterdownloadmanager-cli pair
```

## 9. Native Linux deployment

- Ship/document a release archive and build-from-source workflow that includes the same embedded UI as Docker. Keep compatibility with the repository's existing Linux packaging where practical.
- Run under a dedicated non-root `mdm` user, with configuration in an explicitly documented protected location and downloads in an owner-selected directory.
- Supply `deploy/systemd/mdm.service` and an environment-file example. Configure `ExecStart` with `--headless`, restart-on-failure, graceful shutdown, mount dependencies and only the filesystem/network access required.
- Apply systemd hardening after testing it against helpers, extraction, adapter binding, mounted shares and torrent behavior; do not publish incompatible blanket restrictions.
- Document first-run directory ownership, pairing, `systemctl` lifecycle, journald logs, firewall rules and HTTPS/proxy setup. No desktop, X11, Wayland or user keyring session should be required.
- Document configuration precedence: CLI overrides environment, which overrides persisted settings. Mark deployment-owned settings read-only in the UI with an explanation.
- Provide safe backup/restore, upgrade, migration and rollback instructions. Backups must explain credential/TLS keys, partial files and version compatibility, not just completed downloads.

Existing native entry point to retain and test:

```sh
misterdownloadmanager --headless --data /var/lib/mdm --downloads /srv/downloads
```

## 10. Delivery phases and exit criteria

### Phase 0 — baseline and parity map

- [ ] Run the current headless server, web remote, extension server pairing and container on Linux.
- [ ] Record current contracts, UI states, supported controls and headless blockers.
- [ ] Produce the feature/permission/headless-policy matrix and agree the first-release boundary.
- [ ] Capture baseline tests and protect existing desktop/local-extension workflows.

**Exit:** reproducible baseline and explicit gap list; no duplicate engine/backend work.

### Phase 1 — secure application services and API

- [ ] Extract the first shared operations, add typed/versioned routes and capability reporting.
- [x] Implement owner enrollment, scoped identities, revocation and legacy-token migration.
- [x] Implement owner username/password login, verified-client bypass policy, consecutive-failure bans, configurable ban duration and server-enforced session timeout.
- [ ] Add download details/actions, settings validation, pending decisions and live updates.
- [ ] Implement storage-root and request-policy protections before exposing broader controls.

**Exit:** API integration tests prove authorized operations, denied operations and restart persistence without a desktop.

### Phase 2 — downloads-first web application

- [x] Build authentication/setup, workspace shell, responsive list, add flow, bulk actions and details.
- [x] Add core queues, custom categories, scheduler and grouped allowlisted settings (advanced parity remains Phase 4).
- [x] Add Web UI Authentication with every requested qBittorrent-style option, clear bypass warnings, validation and password-change/session feedback.
- [x] Implement loading/error/disconnect states, keyboard use, themes, translations and RTL (formal accessibility acceptance remains open).
- [ ] Review rendered screens with realistic downloads and large histories.

**Exit:** the owner can perform the daily download workflow entirely in the browser.

### Phase 3 — remote extension workflow

- [x] Improve URL configuration and pairing/test/status UX.
- [ ] Integrate versioned capture, rules/capabilities, idempotency and durable acknowledgment.
- [x] Preserve browser downloads on failure and document session-bound link limitations.
- [ ] Pass Chromium and Firefox tests with no local MDM/native host.

**Exit:** configuring a server URL is sufficient to capture a public download remotely and see it in the web UI.

### Phase 4 — advanced feature parity

- [ ] Complete torrent selection/details, media/gallery workflows, extraction and account management.
- [ ] Complete supported networking/limits, diagnostics, device management and server settings.
- [ ] Resolve or explicitly document every parity-matrix item; distinguish desktop-only and deployment-managed items.

**Exit:** every server-relevant feature has a working web path or a clearly identified release blocker, not a silent omission.

### Phase 5 — deployment and release hardening

- [ ] Finish Docker/Compose, systemd/native packaging, health checks and optional helper image.
- [ ] Validate private HTTPS and supported reverse-proxy configurations.
- [ ] Publish CI-built artifacts/images and installation, backup, recovery and upgrade docs.
- [ ] Update `README.md`, `docs/server.md`, `docs/ARCHITECTURE.md`, `ROADMAP.md` and changelog as appropriate for implemented changes; retain GPL and attribution notices.

**Exit:** a clean Linux machine can install either deployment mode from the documentation without a desktop or source edits.

## 11. Verification and release acceptance

- **Rust:** targeted and workspace tests, formatting and Clippy; HTTP contract tests and existing lab-server fault/resume cases.
- **API/security:** pairing expiry/rate limits, username/password login, password hashing, credential-change/session revocation, idle timeout, consecutive-login bans/expiry/unban, direct-localhost and IPv4/IPv6 whitelist bypass, unsafe catch-all confirmation, permissions, token revocation/migration, hostile origins/hosts, proxy spoofing, invalid settings, secret redaction, URL policy and filesystem/archive escape tests.
- **Frontend:** type/build checks, interaction tests and browser end-to-end flows, including partial bulk failures, live reconnect, unknown sizes, disk-full and read-only destinations.
- **Extension:** existing `node tools/extension/test.mjs` and `node tools/extension/e2e.mjs`, expanded for remote URL/path handling, offline/revoked servers, redirects, acknowledgment, duplicate prevention and safe browser fallback.
- **Linux/container:** fresh install, arbitrary UID/GID, mounted volumes, no display environment, helpers, graceful stop, abrupt restart/resume, backup/restore and configuration migration.
- **Deployment parity:** the same core acceptance flows pass in plain Docker, Compose and native systemd; test reverse-proxy TLS and base paths separately.
- **Visual/accessibility:** desktop/tablet/mobile, both themes, keyboard/screen reader, German long strings and Arabic RTL; inspect rendered states rather than accepting screenshots of the happy path alone.

Final acceptance scenario:

1. Start MDM on a clean Linux server using Compose or systemd, without a GUI.
2. Enroll the owner browser securely and open the modern downloads workspace.
3. Add a download; change its queue/category/limits; watch live progress and inspect details.
4. Configure the extension with the server URL, pair it and capture a public browser download without installing local MDM.
5. Confirm it appears exactly once; unavailable-server capture leaves the browser download usable.
6. Close the web UI, restart the service/container and verify settings, jobs and resumable state persist.
7. Verify the documented server-relevant parity checklist, revoke an extension, and confirm its next request is denied.

## 12. Defaults and remaining implementation decisions

Agreed defaults: one owner with username/password Web UI login, both authentication-bypass switches off, empty IP whitelist, ban after 5 consecutive failures for 3600 seconds, browser idle timeout of 3600 seconds, downloads-first UI, existing Rust engine/runtime, one production origin, local/VPN HTTPS first, Docker/Compose and native Linux supported equally, URL-only remote capture without automatic credential forwarding.

Resolved: embedded vanilla-JS frontend, Argon2id owner credentials, HttpOnly/Secure/SameSite cookies with CSRF, configurable idle expiry plus 24-hour absolute expiry. Still open: server-secret/key policy, helper-image distribution, supported Linux release targets, arm64 validation and hardened internet-facing deployment acceptance.

Out of scope initially: multi-tenant accounts, hosted SaaS, billing, DRM bypass, remote desktop streaming, arbitrary host administration and rewriting the download engine.
