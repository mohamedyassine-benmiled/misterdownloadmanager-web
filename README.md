<p align="center"><img src="brand/webmister.svg" width="88" height="88" alt="Webmister: a W and a download arrow"></p>

# Webmister Download Manager

**Mister Download Manager, on your server and in your browser.**

Webmister is an **independent, Web-focused fork of [Mister Download Manager](https://github.com/anefzaoui/misterdownloadmanager)** by Ahmed Nefzaoui and its contributors. This fork adds a browser workspace, owner authentication, remote extension capture and self-hosted deployment around MDM's existing Rust application and download engines. It is not the official MDM project or an official MDM release.

[Downloads](https://github.com/mohamedyassine-benmiled/misterdownloadmanager-web/releases) · [Docker Compose](#run-with-docker-compose) · [Browser extension](#browser-extension) · [Native server](#run-without-docker) · [Known limitations](#status-and-limitations)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/webmister/workspace-dark.png">
  <img src="docs/images/webmister/workspace-light.png" width="1100" alt="Webmister Web UI: download list, categories, source filters and queue toolbar">
</picture>

*The actual shipped Web UI, captured with explicitly labeled demonstration transfers. These are not live downloads or speed benchmarks. No private account or server data is shown.*

## What this fork adds

- **A downloads-first Web workspace:** add links, magnets and torrent files; start, pause, remove, search and filter downloads.
- **Familiar MDM controls:** persistent toolbar, category/queue tree, light/dark themes and desktop-style progress dialogs.
- **Headless operation:** run the normal application with `--web`; control it without a desktop session.
- **Owner administration:** protected login sessions, CSRF checks, authentication settings and separately scoped extension devices.
- **Queues and schedules:** manage queue membership, concurrency and supported scheduling options.
- **File and torrent tools:** checksums, bounded move/rename, source replacement, post-add torrent selection, peers, trackers and rechecking.
- **Explicit data-reuse decisions:** review existing torrent files before verification/reuse; choose an existing Web download or a separately named copy for duplicate links.
- **Remote browser capture:** send downloads to your server without installing a native messaging host on the browsing computer.

Downloads are written to the **server**, not to the computer running your browser. The underlying executable names, data formats and protocol IDs remain compatible with MDM; the fork identity lives in the Web UI, extension and release packaging.

## Screenshots

<details>
<summary>Progress and torrent file controls</summary>

<img src="docs/images/webmister/download-details.png" width="600" alt="Webmister download progress dialog with connection details">

<img src="docs/images/webmister/torrent-files.png" width="600" alt="Webmister torrent dialog with post-add file selection">

*Rendered controls with synthetic sample transfers. Torrent selection shown here is after adding, not a pre-start file picker.*
</details>

<details>
<summary>Mobile workspace</summary>

<img src="docs/images/webmister/workspace-mobile.png" width="320" alt="Webmister download workspace on a narrow mobile viewport">

*On narrow screens, the toolbar, navigation and transfer table can scroll horizontally to keep controls accessible.*
</details>

The screenshots and their source hashes are reproducible with [`tools/web/readme-shots.mjs`](tools/web/readme-shots.mjs). The original logo source is [`brand/webmister.svg`](brand/webmister.svg).

## Downloads

Use **this fork's [GitHub Releases](https://github.com/mohamedyassine-benmiled/misterdownloadmanager-web/releases)** and select a `web-vX.Y.Z` Webmister release—not the original project's downloads or extension-store listings.

| Release artifact | Purpose |
| --- | --- |
| `Webmister-X.Y.Z-windows-x86_64.zip` | Windows server bundle, `webmister.cmd` launcher and underlying MDM programs |
| `Webmister-X.Y.Z-linux-x86_64.tar.gz` | Linux x86-64 server bundle and `webmister` launcher; built on Ubuntu 22.04 |
| `Webmister-X.Y.Z-extension-chromium.zip` | Manual installation for Chromium-family browsers |
| `Webmister-X.Y.Z-extension-firefox.xpi` | **Unsigned** Firefox extension for temporary developer loading; normal installation requires Mozilla signing |
| `SHA256SUMS.txt` | SHA-256 hashes of the downloads and release metadata |
| `RELEASE.json` / `image-release.json` | Source commit, engine/fork versions, artifact inventory and published container digest |

Native bundles include both extension packages, the GPL license and upstream attribution. They are portable server archives, **not branded desktop installers**. Executables are not claimed to be code-signed; checksum files are not claimed to be signed attestations. Inspect the source and verify the download hashes.

The registry path stays tied to the existing repository:

```text
ghcr.io/mohamedyassine-benmiled/misterdownloadmanager-web
```

`latest` is the latest stable Webmister image; `edge` follows successful main-branch publications. Version tags and `sha-<full-commit>` tags are also produced. Pin the published `@sha256:…` digest for controlled deployments. Container support currently targets **linux/amd64**, not ARM.

> A successful Webmister publication is required before these downloads/image tags exist. If no `web-v*` release is available yet, use [Build from source](#build-from-source). This README does not claim an unpublished release is already downloadable.

## Run with Docker Compose

Requires Docker Engine and the Docker Compose plugin on a Linux host. Use a non-root host user with Docker access, or deliberately choose the non-root UID/GID that will own the mounted folders.

### 1. Create the folders

```sh
mkdir -p webmister/config webmister/downloads
cd webmister
chmod 700 config
printf 'MDM_UID=%s\nMDM_GID=%s\n' "$(id -u)" "$(id -g)" > .env
```

The configured UID/GID must own and be able to write both folders. Do not let Docker create root-owned bind mounts for you. Keep `config/` private: it contains the owner account, device state, settings and TLS material.

### 2. Save this as `compose.yml`

```yaml
services:
  webmister:
    image: ghcr.io/mohamedyassine-benmiled/misterdownloadmanager-web:latest
    restart: unless-stopped
    init: true
    user: "${MDM_UID:-1000}:${MDM_GID:-1000}"
    stop_grace_period: 60s
    ports:
      - "127.0.0.1:8463:8463"
    environment:
      MDM_DATA_DIR: /config
      MDM_DOWNLOADS: /downloads
      MDM_REMOTE_PORT: "8463"
      MDM_REMOTE_URL: https://localhost:8463/
      MDM_REMOTE_HOSTS: localhost
      TZ: Etc/UTC
    volumes:
      - ./config:/config
      - ./downloads:/downloads
    security_opt:
      - no-new-privileges:true
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

A configurable standalone version is also provided at [`docker/compose.release.yml`](docker/compose.release.yml). It does not require a source checkout or a Rust toolchain.

### 3. Start and enroll the owner

```sh
docker compose config --quiet
docker compose pull
docker compose up -d
docker compose logs webmister
```

1. Open **https://localhost:8463/** on the Docker host. For a remote server, use a LAN/VPN address as described below or an SSH tunnel: `ssh -L 8463:127.0.0.1:8463 user@server`.
2. Compare the self-signed certificate's SHA-256 fingerprint with the server logs before accepting it, or install a certificate trusted by your browser.
3. Use the one-time setup code in the logs to create the owner username/password. This is a server-local account, not a cloud registration.
4. Add a download or pair your browser extension. Files appear under the host's `downloads/` folder.

Get a fresh setup/pairing code when needed:

```sh
docker compose exec webmister misterdownloadmanager-cli pair
```

For access from another device, replace the published `127.0.0.1` with the Docker host's specific **LAN/VPN IP**, update `MDM_REMOTE_URL` to the HTTPS URL clients will use, and add any DNS hostname to `MDM_REMOTE_HOSTS`. Restrict access with the host firewall/VPN. Do not publish the port directly to the Internet. Leave the **container** listener automatic; its loopback is not the Docker host's loopback.

### Update or stop

Back up `config/` before upgrading; preserve `downloads/`. Then:

```sh
docker compose pull
docker compose up -d
# Clean stop; mounted files remain:
docker compose down
```

Check saved-state compatibility before downgrading. The image has an owner-credential-free healthcheck; Compose allows time for a clean stop and rotates logs. Pairing/setup codes in logs are sensitive—do not publish those logs.

## Browser extension

Use the **Webmister packages from the same release**, not the original project's store listings. No fork store publication or Firefox signing is implied.

1. **Chromium:** unzip `…-extension-chromium.zip`, open your browser's extensions page, enable Developer mode, choose **Load unpacked**, and select the folder containing `manifest.json`.
2. **Firefox:** open `about:debugging` → **This Firefox** → **Load Temporary Add-on** and select the unsigned XPI or its unpacked `manifest.json`. Temporary installation lasts until Firefox restarts. A normal persistent installation requires Mozilla signing, which this release pipeline does not perform.
3. Open your server's HTTPS page in that same browser and verify/trust its certificate.
4. In the Web UI, open **Browser extensions and devices**, generate a pairing code, then enter the server URL and code in the extension's **Another MDM** destination.

Remote mode does not need a native host. Revocation keeps the selected server destination and returns the transfer to the browser rather than silently switching to local MDM. Browser-session cookies, proxy credentials and POST bodies are not forwarded to the server; authenticated/browser-session downloads may need to stay in the browser.

The fork intentionally retains MDM's extension/protocol IDs for compatibility. Do not try to keep the official extension and this fork installed side-by-side under the same identity. Local/native behavior remains available, but server capture is the Webmister workflow described here.

## Run without Docker

Extract the matching server archive. The `webmister.cmd`/`webmister` launcher enables `--web` automatically; you can also run the underlying binary directly:

```sh
./misterdownloadmanager --web --port 8463 --webhost 127.0.0.1 \
  --data ./config --downloads ./downloads
```

On Windows, use `misterdownloadmanager.exe` or `webmister.cmd` with the same flags. The binary's desktop mode still uses upstream MDM names/artwork; these releases are intended to run as the headless server. There is no fork-native auto-updater: upgrade from this fork's releases manually rather than using the desktop's upstream updater.

| Parameter | Meaning |
| --- | --- |
| `--web` | Explicitly enable headless Web operation |
| `--port N` | HTTPS port; default 8463 |
| `--webhost IP` | Local IP or adapter to bind, e.g. `127.0.0.1`, `[::1]` or a LAN address—not a public URL |
| `--data PATH` | Private, persistent configuration/account folder |
| `--downloads PATH` | Server download root |

Legacy `--headless` and `--listen` remain supported. Omit the host selection to use the existing local-network address selection; unspecified `0.0.0.0`/`::` bindings remain refused. See [server parameters and releases](docs/web-releases.md) and the [systemd examples](deploy/systemd/).

## Build from source

```sh
git clone https://github.com/mohamedyassine-benmiled/misterdownloadmanager-web.git
cd misterdownloadmanager-web
docker build -f docker/web.Dockerfile -t webmister:local .
```

Use `webmister:local` as the Compose image instead of GHCR. Or build natively using the toolchain in `rust-toolchain.toml`:

```sh
cargo build --locked --release -p mdm-app -p mdm-cli -p mdm-nmhost -p mdm-mcp
```

The build-from-checkout Compose file is [`docker/compose.yml`](docker/compose.yml); see [`docker/.env.example`](docker/.env.example). Developer tests and environment details are in [Web development](docs/web-development.md).

## Status and limitations

This is an evolving fork, **not 100% desktop/Web parity**. RSS, batch/link grabbing, import/export, advanced media/gallery workflows, accounts/secrets, extraction and broader settings still need work. See the [parity plan](docs/web-functional-parity-plan.md).

Use trusted LAN/VPN deployments for trials. Do not treat this as hardened public-Internet hosting or an unattended important-data service: outbound source/SSRF hardening, filesystem races and interrupted operations still require care. Existing torrent reuse can replace nonmatching data at the approved torrent paths; read its consent prompt.

Automated checks cover implemented browser, authentication, extension and real-headless flows. Full Linux/Compose/systemd acceptance, production proxy/certificate setups and real Firefox remote runtime are not all validated. Docker build/smoke and native release-binary checks must pass on GitHub before publication; configuration alone is not a successful release.

## Releases and upstream integration

The fork-owned **Webmister release** workflow prepares one `web-vX.Y.Z` release from one source commit: Windows/Linux archives, both extension packages, a tested Docker image, checksums and provenance metadata. Manual workflow runs are build-only. Missing required artifacts block publication; native builds do not silently become optional. Download checksums are provided without claiming upstream signing authority.

See [release operations](docs/web-releases.md) and the [upstream integration plan](docs/upstream-merge-plan.md). Fork-specific code stays in dedicated modules/workflows where practical, with small shared integration hooks. Upstream updates still require review and regression testing.

## Credits and license

Webmister builds on **[Mister Download Manager](https://github.com/anefzaoui/misterdownloadmanager)**, created by **Ahmed Nefzaoui and contributors**. The original application, download engines, translations and desktop artwork remain credited to their authors; see [ATTRIBUTION.md](ATTRIBUTION.md) and the source notices. The new Webmister name/mark identify this independent fork, not an endorsement by the upstream project.

Licensed under **[GPL-3.0-or-later](LICENSE)**. Source is available in this repository and at each release tag. Please report fork/Web issues [here](https://github.com/mohamedyassine-benmiled/misterdownloadmanager-web/issues), and support upstream development through its own project.
