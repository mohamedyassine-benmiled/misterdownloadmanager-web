# Deferred Linux server acceptance

Linux, Docker/Compose and systemd runtime testing is intentionally deferred until the
application features are finished. This is a test checklist, not evidence of a passing deployment.
The development results in [web-development.md](web-development.md) were obtained on Windows.

## Record the test environment

- [ ] Record commit, distribution/version, architecture, kernel, Rust version, Docker/Compose
      versions, filesystem, timezone and browser versions.
- [ ] Use disposable configuration/download directories and an unprivileged test account.
      Never test reset/restore/destructive operations against production state.
- [ ] Record command, expected result, actual result, relevant redacted logs and pass/fail for
      each case below. Protect authentication files, profiles, tokens and backups as secrets.

## Build and static checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --locked -p mdm-app -p mdm-cli
npm ci --prefix tools/web
npx --prefix tools/web playwright install --with-deps chromium
node tools/extension/test.mjs
node tools/web/test.mjs
node tools/web/headless.mjs
```

- [ ] Include Linux-only symlink/path tests. Explain ignored upstream graphical/live-service tests.
- [ ] Run real Firefox extension capture and rejected/offline/revoked fallback without a native host.
- [ ] Inspect desktop/mobile, light/dark and Arabic RTL screenshots from the actual build.

## Native headless process

- [ ] Start without DISPLAY/WAYLAND_DISPLAY or a desktop session, as the dedicated service user.
- [ ] Verify writable/private configuration and downloads directories; clear errors for denied,
      read-only/full filesystems and an already-running instance.
- [ ] Enroll the owner using the local one-time code; no default credentials or anonymous settings.
- [ ] Confirm downloads continue with the browser closed and `misterdownloadmanager-cli health`
      works without owner credentials. Corrupt auth storage must fail closed and fail readiness.
- [ ] Add HTTP/FTP/torrent work, edit settings/queues/categories and exercise each newly completed
      feature documented in [web-development.md](web-development.md).
- [ ] SIGTERM saves state and shuts down helper processes. Restart preserves settings, auth,
      devices, history, selected torrent files, queues and partial-download resume data.
- [ ] Test abrupt termination/resume and compare finished file hashes; no duplicate acceptance
      claims unless durable capture/idempotency tests actually pass.
- [ ] File tools: finished-file and not-yet-started rename/move, existing destination/partial-file
      collisions, cross-filesystem moves, denied paths/symlink escapes and interrupted moves.
      A failed/interrupted copy must retain the original; inspect any incomplete destination
      before retrying. Moves are not yet durable transactional jobs.
- [ ] Checksum tools: match/mismatch, malformed hash, clear expected hash, resumed verification,
      missing file and huge-file checks without blocking API activity or clean shutdown.
      Expected hashes persist; transient verification results must be rerun after restart.
- [ ] Replace a paused expired link, including existing partial/resume data. Verify size/content
      checks still prevent joining different files and old cookies/form bodies/logins are cleared.
- [ ] Torrent tools: saved metadata while stopped, file selection including invalid/duplicate/
      padding indexes, sequential/first-last behavior, upload limits, recheck/reannounce outcomes,
      tracker filtering and peer/piece counts. Use a disposable local swarm; do not infer a real
      swarm test from a metadata-only fixture. Verify file choices survive restart.

## Plain Docker and Compose

Follow [server.md](server.md), using both plain `docker build`/`docker run` and Compose.

```sh
docker compose -f docker/compose.yml config --quiet
docker compose --env-file docker/.env.example -f docker/compose.yml config --quiet
docker build -t misterdownloadmanager-web:test .
```

- [ ] Test release image (and dev image separately if supported); no display/runtime desktop libs.
- [ ] Test numeric UID/GID, fresh bind mounts and named volumes; `/config` stays private and
      downloads are readable by the deliberately configured sharing account/ACL, not world-writable.
- [ ] Confirm loopback-only publishing by default, explicit LAN/VPN publishing, allowed hosts,
      advertised URL, port remapping and certificate fingerprint.
- [ ] Healthcheck reports ready/unready correctly; clean `docker stop` within 60 seconds,
      restart/resume, init/helper reaping, log rotation and persistent TLS/auth material.
- [ ] Test mounted shares, missing mounts, disconnect/reconnect, file locking and disk-full errors.
      Ensure a missing share cannot silently redirect downloads onto the underlying filesystem.

## systemd

```sh
sudo systemd-analyze verify /etc/systemd/system/mdm.service
sudo systemctl daemon-reload
sudo systemctl enable --now mdm
sudo journalctl -u mdm -n 100 --no-pager
```

- [ ] Install binaries, account, folders, environment and unit exactly as documented.
- [ ] Verify private state permissions, restrictive umask, mount dependencies, localhost default,
      journald logging, restart-on-failure and absence of GUI/password/keychain prompts.
- [ ] Local CLI works as `mdm` with the correct MDM_DATA_DIR; other accounts cannot use protected IPC.
- [ ] Stop/restart/reboot, SIGTERM, share dependencies and all sandbox restrictions permit intended
      downloads/helpers while preventing unrelated home/system writes.

## Authentication and network boundary

- [ ] Verify credential changes, logout, inactive/absolute expiry, ban threshold/duration,
      successful-login reset, persisted bans and local recovery commands.
- [ ] Test IPv4/IPv6/mapped IPv6, direct localhost bypass, empty/invalid/catch-all subnets,
      separate owner administration and capture/control token permissions.
- [ ] Verify Host/Origin/CSRF protections and hostile forwarding headers. A loopback proxy must
      not grant remote visitors localhost bypass; ambiguous/missing proxy metadata fails closed.
- [ ] Validate production certificate trust in Chromium and Firefox without ignoring TLS errors.
- [ ] If proxy deployment is supported, test root and `/mdm/` paths, assets/API/cookie paths,
      Host/Origin handling, upstream TLS verification and overwritten forwarding headers.
- [ ] Verify the implemented outbound request policy against private/loopback/link-local/metadata
      sources, redirects and DNS changes. Record remaining limitations; do not assume SSRF
      hardening exists simply because owner login works. Keep access private LAN/VPN-only.

## Recovery, upgrade and release gate

- [ ] Back up stopped configuration/TLS/auth state and required downloaded/partial files; restore
      into a fresh test instance. Verify documented secret-storage limitations and access rights.
- [ ] Test old configuration/device migration, upgrade and rollback without data loss or owner
      elevation. Revoke test tokens and remove disposable credentials before sharing diagnostics.
- [ ] Record outstanding failures and release blockers. Publish images only after the documented
      native, Docker and Compose core flows pass; validate arm64 separately before claiming it.
