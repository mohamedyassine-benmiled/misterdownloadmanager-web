# Linux server test report — 2026-11-11

This report records a partial run of [the Linux server acceptance checklist](linux-server-testing.md).
It is not a release acceptance or a claim that every checklist item passed.

## Environment

- Commit: `37996ade55f902367ef0a28aa9f0c4b8cc8bba55`.
- Host: Debian 12, x86-64, kernel `6.1.0-52-amd64`; ext4 filesystem; timezone EST (UTC-05:00).
- Rust: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`; Node.js 18.20.4.
- Docker: 20.10.24; Docker Compose: 5.5.0.
- Browser automation: Playwright Chromium 145.0.7632.6, headless.
- Tests ran as an unprivileged user. Headless browser integration used a fresh test data directory;
  Docker lifecycle checks used separate disposable configuration and downloads bind mounts.

## Results

| Check | Expected | Actual |
| --- | --- | --- |
| `cargo fmt --all --check` | Formatting passes | **Pass.** |
| `cargo clippy --workspace --all-targets -- -D warnings` | No warnings | **Pass.** |
| `cargo build --locked -p mdm-app -p mdm-cli` | Native binaries build | **Pass.** |
| `cargo test --workspace` | All enabled tests pass | **Fail.** Three `mdm-crash` end-to-end tests failed; see below. Cargo stopped at that test target. |
| `cargo test --workspace --exclude mdm-crash` | Remaining workspace tests pass | **Pass**, with upstream ignored tests. Includes Linux symlink/path checks, HTTP and FTP tests, interrupted-download resume, and local torrent seeding. |
| `npm ci --prefix tools/web` | Install locked browser-test dependencies | **Pass**, no reported npm vulnerabilities. |
| `npx --prefix tools/web playwright install chromium` | Install Chromium for browser checks | **Pass.** The host dependencies were already sufficient; `--with-deps` was not run. |
| `node tools/extension/test.mjs` | Extension logic checks pass | **Pass:** 79 checks, using fake browser APIs. |
| `node tools/web/test.mjs` | Browser UI checks pass | **Pass:** 20 checks against a synthetic API fixture, including theme and German/Arabic mobile layout checks. Screenshots are in ignored `target/web-shots/`; they were not all manually inspected. |
| `node tools/web/headless.mjs` | Real headless application integration checks pass | **Pass:** 10 checks against the native binary, real HTTPS, and Chromium. |
| `docker compose -f docker/compose.yml config --quiet` | Default Compose configuration validates | **Pass.** |
| `docker compose --env-file docker/.env.example -f docker/compose.yml config --quiet` | Example configuration validates | **Pass.** |
| `docker build --build-arg PROFILE=dev -t misterdownloadmanager-web:test .` | Development image builds | **Pass.** Release image was not built or tested. |

The real headless integration covered owner enrollment and protected cookies, authentication and
CSRF rejection, settings/categories/queues, a capture-only device adding a download, checksum and
file operations, saved torrent metadata and settings, Chromium extension capture and revoked-token
fallback, shutdown/restart persistence, device revocation, login bans, and local recovery. The
torrent UI checks do not by themselves establish a real-swarm web workflow; the separate Rust
torrent tests exercised local seeding.

Ignored tests included graphical window tests requiring a graphics session, a built-website link
test, and selected tests requiring live external services or helper programs. These were not
counted as passes.

## Crash-report failures

The `mdm-crash` integration tests `a_native_crash_reports_where_it_crashed`,
`a_freeze_reports_the_frozen_function`, and `a_panic_reports_its_message_and_function` failed.
Reports and dumps were produced, but their output reported `No symbols for: crashy`; expected
function names were absent or incorrect. The cause has not been established. This remains a
failure, not a verified fix.

## Docker runtime observations

- A disposable plain Docker container ran as numeric UID/GID 1000:1000 with `--init`, fresh
  bind mounts and a loopback-only published test port. Its config directory was mode `0700`.
- The local `misterdownloadmanager-cli health` returned `{"ready":true}`; the HTTPS page
  returned HTTP 200. An unapproved `Host` header returned HTTP 421.
- `docker stop -t 60` and restart succeeded; the local health check returned ready again.
  The disposable container was removed after testing.
- A separate Compose deployment using the development image was published on a specific LAN
  address. Docker reported it healthy, and Chromium loaded the actual owner-setup page. It was
  left running with persistent host bind mounts and an `unless-stopped` restart policy.
- `ldd` on the image's application binary showed libc, libm and libgcc, with no desktop runtime
  libraries. The Debian 13 image's binary could not run directly on this Debian 12 host because
  its glibc is older; a native build on the host succeeded.

## Not yet validated

The checklist remains open for the release image, named volumes and sharing ACLs, a real Firefox
extension capture/fallback run, full screenshot inspection, installed systemd service and its
sandbox, mount disconnect/disk-full behavior, broad authentication/proxy/network-boundary cases,
production certificate trust, upgrades/rollback/backup restore, and arm64. Other unchecked
checklist cases should not be inferred from the passing checks above. `systemd-analyze verify`
against the repository unit reported that `/usr/local/bin/misterdownloadmanager` was absent;
the service was not installed or started.
