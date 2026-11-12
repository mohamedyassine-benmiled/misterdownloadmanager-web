# Webmister parameters and coordinated releases

## Application parameters

The normal app binary also runs the server; no separate desktop instance is needed:

```sh
misterdownloadmanager --web --port 8463 --webhost 127.0.0.1 \
  --data /srv/mdm/config --downloads /srv/downloads
```

- `--web`: enable headless Web mode. It explicitly overrides `MDM_REMOTE=off`.
- `--port N` or `--port=N`: HTTPS port, default 8463. The existing `MDM_REMOTE_PORT`
  environment variable still works; an explicit flag wins.
- `--webhost ADDRESS` or `--webhost=ADDRESS`: local listener IP or network-adapter name,
  e.g. `127.0.0.1`, `[::1]`, a specific LAN IP or `eth0`. It takes precedence over
  legacy `--listen`/`MDM_REMOTE_LISTEN`. Without either, the existing server selects
  local-network addresses. Unspecified `0.0.0.0`/`::` listeners remain refused.
- `--headless` and `--listen` remain compatible with previous deployments. Unlike
  explicit `--web`, legacy `--headless` continues respecting `MDM_REMOTE=off`.

`--webhost` is **not** the public browser URL or a DNS hostname allowlist. Use
`MDM_REMOTE_URL` for the advertised HTTPS URL, and `--hosts`/`MDM_REMOTE_HOSTS` for
additional allowed HTTP hostnames. In a container, omit `--webhost` to select its
local network interface; container loopback is not the Docker host's loopback.
Use Docker's published host address to restrict external access.

Normal startup without `--web`/`--headless` still starts the desktop app. The fork
retains upstream's download engine, queues, storage and desktop/local-extension paths.

## Image and release policy

Release controller: [`.github/workflows/web-release.yml`](../.github/workflows/web-release.yml).
Reusable image build: [`.github/workflows/web-image.yml`](../.github/workflows/web-image.yml).
Recipe: [`docker/web.Dockerfile`](../docker/web.Dockerfile). The root upstream Dockerfile
is not the fork release recipe. Current target platform: **linux/amd64** only.

Configured registry name:

```text
ghcr.io/mohamedyassine-benmiled/webmisterdownloadmanager
```

The workflow lowercases `github.repository`, so another fork publishes under its own
owner/repository, not ours. **No image is claimed available until GitHub successfully
runs this workflow and publishes it.**

| Event | Build/test | Publication |
| --- | --- | --- |
| Code push to `main` | Web/headless regressions, image build, runtime smoke | `edge` and `sha-<full-commit>` |
| Pull request | Same verification, including isolated image smoke | Never logs into GHCR or publishes |
| Push `web-v1.2.3` | Windows/Linux release-binary tests, extension packaging, Web gates, image smoke, complete-asset verification | All native/extension downloads, checksums, image `1.2.3`/`latest`/SHA tags and one GitHub release |
| Push `web-v1.2.3-rc.1` | Same gates | Same downloads, prerelease image tag and SHA; no `latest` |
| Manual Webmister release dispatch | Complete build/test and artifacts | Never publishes |
| Manual image-only dispatch | Image build/test | Only when `publish` is checked, on `main` or a `web-v*` tag; does not create a downloads release |

Web image versions are fork release identifiers, not a claim that the desktop
`Cargo.toml` version has changed. Original desktop `v*` release tags/workflows remain
separate. `edge`/`latest` are moving aliases; pin a published SHA-256 digest for controlled
deployments. SHA tags identify a source commit but are not registry-enforced immutable.

The image job reuses the Web checks, runs native/remote extension regressions and
publication-policy tests, builds locally, then smoke-tests **that same image** before
pushing it. Smoke checks non-root startup, `--web`/`--port`/`--webhost`, health, HTTPS,
the owner setup gate and clean shutdown. Smoke uses fresh tmpfs directories, never
the owner's state; its logs/enrollment codes are not uploaded.

Publication uses the repository's `GITHUB_TOKEN` with job-scoped `packages: write`;
no Docker Hub password/PAT is required. On first publication:

1. Ensure repository/org Actions policy permits package creation and these actions.
2. Allow the workflow repository to write the linked GHCR package if an existing
   package has restrictive access settings.
3. Set package visibility to **public** if anonymous pulls are intended; workflow
   publication does not silently change package visibility.
4. Check the workflow's published digest and release manifest, not just its green build.

Web tags first build and check both browser extensions and both required native
platforms. Native builds use `--locked --release` and execute the real-headless suite
against the release binary and the actual versioned Chromium release extension
before packaging. Only after those jobs succeed is the
tested image published. The controller then checks required files, versions, source
commit agreement, image digest, extension contents and archive layout, and writes
`SHA256SUMS.txt` and `RELEASE.json`. Missing platforms are failures, not optional skips.

The release remains a GitHub draft while files are attached, and becomes public only
after the complete set is uploaded. Existing public releases are not overwritten on
a rerun. Drafts may be resumed; check their source/tag before retrying. GHCR and GitHub
Releases are separate services: a later publication failure may leave an image already
pushed, so this is a guarded sequence, not a cross-service atomic transaction.

Artifacts are `Webmister-X.Y.Z-windows-x86_64.zip`,
`Webmister-X.Y.Z-linux-x86_64.tar.gz`, the Chromium extension ZIP, unsigned Firefox
XPI, checksums and JSON receipts. Windows/Linux archives include the existing MDM
programs, a `webmister` launcher, extension packages, GPL and upstream attribution.
These are portable **server bundles**, not renamed desktop installers; no macOS/ARM
release is advertised by this controller. Native code signing, a fork auto-updater,
Mozilla signing and extension-store publication are not configured. Do not use
upstream's signing keys or store credentials to publish fork artifacts.

Browser manifests use the numeric base of the fork release version. Their standard
`version_name` records `X.Y.Z[-prerelease] (MDM <engine-version>)`; the native-host
compatibility check uses that engine baseline, not the independently numbered fork
release. This avoids false "update app" warnings when the fork and engine versions
differ while retaining warnings for genuinely old native hosts. Prerelease and stable ZIP/XPI
packages are installed/reloaded manually, not an automatic store-update channel.
Extension IDs/protocol compatibility are deliberately retained; official and fork
extensions must not be installed side-by-side under the same identity.

The workflows do not push source commits or create Git tags themselves. A version/tag
and explicit owner authorization are required before an actual publication attempt.

## Run or build

After the first successful publication, for example:

```sh
docker pull ghcr.io/mohamedyassine-benmiled/webmisterdownloadmanager:edge
mkdir -p /srv/mdm-config /srv/downloads
# Set ownership/private permissions for UID/GID 1000, or choose --user appropriately.
docker run -d --name mdm --restart unless-stopped \
  -p 127.0.0.1:8463:8463 \
  -v /srv/mdm-config:/config -v /srv/downloads:/downloads \
  ghcr.io/mohamedyassine-benmiled/webmisterdownloadmanager:edge
```

The image entry point already supplies `--web`; extra flags follow the image name.
If changing `--port`, also change the container side of the Docker port mapping.
Use a specific LAN/VPN host address instead of host loopback when remote access is
intended; do not directly expose this server to the public Internet.

Build the fork image without the registry:

```sh
docker build -f docker/web.Dockerfile -t misterdownloadmanager-web:local .
```

Compose keeps both options. After editing `docker/.env` and creating its directories:

```sh
docker compose --env-file docker/.env -f docker/compose.yml pull
docker compose --env-file docker/.env -f docker/compose.yml up -d --no-build
# Or build this checkout instead:
docker compose --env-file docker/.env -f docker/compose.yml up -d --build
```

Back up `/config` before an upgrade; preserve `/downloads`. Check state compatibility
before rolling a container back. The smoke gate is not exhaustive Linux/container,
reverse-proxy, Compose, systemd, extraction/helper or ARM acceptance.

## Local validation status

Preparation pass, 2026-11-12: Webmister branding/README/screenshots received a scoped
presentation SHIP review. Local checks passed: 80 extension groups, 29 Web browser
fixture groups, 11 real-headless groups, 10 coordinated packaging/documentation
tests, 6 image-policy tests, 4 boundary tests, 4 HTTPS server tests, affected-crate
Clippy/formatting, catalog and workflow lint. Chromium/Firefox packages were packed
and checked; a Windows bundle was assembled and inspected using local **debug**
binaries. Linux archive tests use explicit synthetic binaries to check packaging
and executable modes, not evidence that a Linux binary ran here.

The 11 real-headless groups also passed with the packaged Chromium extension at
fork version `9.0.0` against engine `0.5.1`, including pairing, capture and revocation.
This exercised the same versioned-package override used by the native release jobs.

These checks do not replace the controller's optimized Windows/Linux release-binary
jobs, Docker build/smoke or a real GitHub publication. No tag/release was created and
no image or extension was published in this pass.

The development host has no Docker engine. Publication policy tests and workflow
lint can be run locally; actual image build/smoke/push must still run on GitHub or a
Linux Docker host. Adding these workflows is not a completed deployment acceptance.

See [upstream-merge-plan.md](upstream-merge-plan.md) for adapter ownership and safe
integration, and [linux-server-testing.md](linux-server-testing.md) for broader acceptance.
