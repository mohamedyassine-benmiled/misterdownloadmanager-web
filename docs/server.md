# Run MDM on a server

> **Webmister transport:** HTTP is the default and is unencrypted. Keep it on loopback or a
> trusted protected network; credentials and session tokens can be intercepted. Set
> `MDM_REMOTE_TLS=1` and use an `https://` value for `MDM_REMOTE_URL` to enable the built-in
> self-signed HTTPS listener. The upstream HTTPS-specific examples below require that setting.

Run Mister Download Manager on a home server or a NAS (Docker, Unraid, any Linux machine) without
a window, and send it downloads from your phone or your browser: the server downloads them into
its own folders, such as a network share.

It is the same MDM as on a desktop: its list, categories, queues, Scheduler, speed limits and
torrents. You control it with the [remote control's page](remote-control.md) (from any phone or
computer on your network), the browser extension pointed at it, or its command line.

> **New:** the server mode is new in this version. Please
> [tell us how it goes](https://github.com/anefzaoui/misterdownloadmanager/issues).

## With Docker

Build the fork image from source, or use the configured GHCR publication workflow
after its first successful release; see [Web image releases](web-releases.md).

```sh
git clone https://github.com/mohamedyassine-benmiled/misterdownloadmanager-web
cd misterdownloadmanager-web
docker build -f docker/web.Dockerfile -t misterdownloadmanager .
```

The first build is long (up to an hour on a small server: the whole program is optimised as
one). The image is small, about 175 MB: Debian's slim base, MDM's two programs and the
certificates downloads are checked against. It needs no desktop libraries: MDM loads those only
to open a window, which a server never does.

Then run it, with a folder for its settings and one for the downloads. Make both folders first,
as the user MDM will run as: a folder Docker makes for you belongs to `root`, and MDM can't use it.

```sh
mkdir -p /srv/mdm-config /srv/downloads
docker run -d --name mdm --restart unless-stopped \
  -p 8463:8463 \
  -v /srv/mdm-config:/config \
  -v /srv/downloads:/downloads \
  --user "$(id -u):$(id -g)" \
  misterdownloadmanager
```

- `/config` keeps the list, the settings, the certificate and the paired devices.
- `/downloads` is where files land: each category has its folder inside (`Music`, `Video`,
  `Torrents`…). Mount a network share there to download straight onto it.
- `--user`: the user that owns those folders (MDM runs as no particular user).

[docker/compose.yml](../docker/compose.yml) does the same with Docker Compose.

### Configurable Docker Compose (web fork)

The fork's Compose template defaults to access from the Docker host only. It can build
`docker/web.Dockerfile` from this checkout or pull the fork's GHCR image after publication.
From the repository root:

```sh
cp docker/.env.example docker/.env
mkdir -p docker/config docker/downloads
```

Edit `docker/.env` before starting:

- Set `MDM_UID` and `MDM_GID` to the output of `id -u` and `id -g`. Both mounted directories
  must be writable by that user; `/config` must also be private to that user.
- `MDM_CONFIG_DIR` and `MDM_DOWNLOADS_DIR` are bind-mount paths. Relative paths resolve from
  `docker/compose.yml`'s directory, so `./config` means `docker/config`. Absolute paths work too.
- To connect from another device, set `MDM_BIND_ADDRESS` to the host's specific LAN/VPN IP,
  `MDM_REMOTE_URL` to the HTTPS URL clients will open, and `MDM_REMOTE_HOSTS` to any hostname
  used in that URL. Permit the port only on the intended private network.
- `MDM_WEB_PORT` is the host's port; the container continues listening on 8463. If you change
  it, update the public URL too. Do not set the container's listener to host loopback.
- `TZ` controls the server's timezone. `MDM_BUILD_PROFILE=dev` is useful for a faster trial
  build; use `release` for normal deployment.

```sh
docker compose --env-file docker/.env -f docker/compose.yml config --quiet
docker compose --env-file docker/.env -f docker/compose.yml up -d --build
docker compose --env-file docker/.env -f docker/compose.yml logs mdm
docker compose --env-file docker/.env -f docker/compose.yml exec mdm misterdownloadmanager-cli pair
```

For a published image instead, use `docker compose ... pull` followed by
`docker compose ... up -d --no-build`. The `edge` tag tracks tested main-branch
publications; versioned releases use `web-v*` source tags. Publication and first-run
package permissions are documented in [web-releases.md](web-releases.md).

The native app also accepts `--web --port 8463 --webhost 127.0.0.1` to run without
desktop windows; legacy `--headless`/`--listen` remain supported. `--webhost` selects
a local listener address/adapter, not the advertised URL or allowed HTTP hostname.

Compose reaps helper processes, waits up to 60 seconds for a clean stop, and rotates container
logs. The image healthcheck runs `misterdownloadmanager-cli health` against the local application
loop, HTTPS listener state and authentication storage; it needs no owner credentials. The Compose and systemd templates
still require Linux runtime validation; see [web development status](web-development.md).
The current remote API remains intended for LAN/VPN use, not direct internet exposure.

## Set up the Web UI owner

`docker logs mdm` shows the address to open and, before owner setup or while no device is paired, a one-time code with a
QR code:

```text
The remote control listens at: https://172.17.0.2:8463/
Its certificate's SHA-256 fingerprint: DE:26:7F:…
Open the address to set up the Web UI owner, or pair an extension, with code 1037 1606 …
```

Inside a container, the address MDM finds is the container's own: give it the one you open, the
**server's** address with the port you published, as `-e MDM_REMOTE_URL=https://192.168.1.10:8463/`,
and the log and the QR code show that one. Open the HTTPS page, verify the certificate fingerprint,
and choose an owner username and a password of at least 12 bytes using that code. Subsequent
browser visits use the owner login, not a saved extension token. For another code, at any
time:

```sh
docker exec mdm misterdownloadmanager-cli pair
```

The downloads-first workspace includes details, bulk actions, allowlisted application settings,
custom categories, queue membership/schedules and extension-device management. **Authentication**
contains username/password changes, localhost/subnet bypass (both off by default), failed-login
bans and inactivity timeout. Sensitive changes require the current owner password. See
[authentication and local recovery](webui-authentication.md) and [remaining feature scope](web-development.md).

Owner download details also provide [file and torrent tools](web-download-tools.md): safe
move/rename, paused source replacement, checksum verification, torrent file/order/upload
controls, tracker additions and peer information. Deployment testing is deliberately deferred
to [the Linux server checklist](linux-server-testing.md) until application work is complete.

To open the page by the server's name (`https://nas.local:8463/`), give MDM that name:
`-e MDM_REMOTE_HOSTS=nas.local` (several: separated by commas). Other names are refused, so that
no web page can reach MDM through a name of its own.

## On Unraid

[docker/unraid.xml](../docker/unraid.xml) is a template: copy it to
`/boot/config/plugins/dockerMan/templates-user/` on the flash drive, then **Docker → Add
Container** and pick **misterdownloadmanager**. It maps `/config` to `appdata`, `/downloads` to a
share, the port 8463, and runs as `nobody:users` (99:100), as Unraid's shares expect. The image
must be built first (above), on the server or pushed to a registry you use. If the container's
log says a folder belongs to another user, give it to `nobody:users` once:
`chown -R 99:100 /mnt/user/appdata/misterdownloadmanager`.

## Without Docker

On any Linux machine with MDM installed (the `.deb`, `.rpm`, AppImage or `tar.gz`):

```sh
misterdownloadmanager --headless --data ~/mdm-server --downloads /srv/downloads
```

| Flag | Or the variable | What it sets |
|---|---|---|
| `--data <folder>` | `MDM_DATA_DIR` | the list, settings, certificate and paired devices |
| `--downloads <folder>` | `MDM_DOWNLOADS` | where downloads go (each category's folder inside) |
| `--port <number>` | `MDM_REMOTE_PORT` | the remote control's port (8463) |
| `--listen <address>` | `MDM_REMOTE_LISTEN` | where it listens: every local network address unless given |
| `--hosts <names>` | `MDM_REMOTE_HOSTS` | names besides its addresses the page may be opened by |
| | `MDM_REMOTE_URL` | the page's address as you open it, shown with the pairing code |

The remote control is on in this mode (`MDM_REMOTE=off` turns it off). It stops cleanly, saving
its list, when the system stops it (`docker stop`, systemd, Ctrl+C). It works on Windows and
macOS too, the same way.

The command line talks to it as to a desktop MDM, with the same data folder:
`MDM_DATA_DIR=~/mdm-server misterdownloadmanager-cli status` (once `api on`), `pair`, and adding
an address: `misterdownloadmanager /d https://example.com/file.zip`.

### Run as a systemd service (web fork)

After installing the release binaries (or building them from source), place
`misterdownloadmanager` and `misterdownloadmanager-cli` in `/usr/local/bin`. The example service
assumes those paths and a dedicated non-login account. On a system using `useradd`:

```sh
sudo useradd --system --user-group --home-dir /var/lib/mdm --shell /usr/sbin/nologin mdm
sudo install -d -o mdm -g mdm -m 0700 /var/lib/mdm
sudo install -d -o mdm -g mdm -m 0750 /srv/downloads
sudo install -d -o root -g root -m 0755 /etc/mdm
sudo install -o root -g root -m 0600 deploy/systemd/mdm.env.example /etc/mdm/mdm.env
sudo install -o root -g root -m 0644 deploy/systemd/mdm.service /etc/systemd/system/mdm.service
```

If the account or folders already exist, inspect them first rather than recreating or recursively
changing ownership. Edit `/etc/mdm/mdm.env` for your directories, URL and listener. The default
listener is `127.0.0.1`; for private remote access, choose a specific LAN/VPN address assigned to
the host. Do not use `0.0.0.0`: the existing runtime intentionally refuses wildcard listeners.

```sh
sudo systemd-analyze verify /etc/systemd/system/mdm.service
sudo systemctl daemon-reload
sudo systemctl enable --now mdm
sudo journalctl -u mdm -n 50 --no-pager
sudo -u mdm env MDM_DATA_DIR=/var/lib/mdm /usr/local/bin/misterdownloadmanager-cli pair
```

The service has no desktop session requirement and logs to journald. `systemctl stop mdm`
allows 60 seconds to persist the list before forced termination. The configuration/state folder
is private; new files use a restrictive umask. The service cannot access home directories; use
a service-owned directory under `/srv` or a mounted share instead. If another service must read
completed downloads, deliberately configure directory/ACL access rather than running MDM as root.

For another downloads mount, use `sudo systemctl edit mdm` and replace the mount dependency:

```ini
[Unit]
RequiresMountsFor=
RequiresMountsFor=/mnt/downloads
```

Also set `MDM_DOWNLOADS=/mnt/downloads` in `/etc/mdm/mdm.env`, grant `mdm` access to that directory,
then reload and restart the service. Ensure network shares are mounted before starting; otherwise
files may land on the underlying local filesystem. Environment overrides persisted settings,
and CLI flags override both. Back up `/var/lib/mdm` (including authentication/TLS material) while
the service is stopped, plus the downloads/partial files you need; protect that backup as a secret.

## Send downloads from your browser

In the browser extension's menu, **Send downloads to → Another Mister Download Manager (a server)**: type the server's address,
open it once in that browser to accept its certificate, then enter a pairing code (`pair`
above). Downloads the extension catches, and **Download with Mister Download Manager** on a link,
then go to the server instead of this computer. The browser's cookies are not sent to the server:
session-bound or IP-bound links may fail on the server even when they work in the browser.
POST/form downloads and URLs containing embedded credentials stay with the browser; use the
local destination when a download requires your browser session.

New extension pairings are capture-only: they can add downloads and read server capture rules/status,
but cannot read history, delete downloads or change owner settings. Issue a fresh one-time code
from **Extensions → Generate pairing code** (requires owner password) or the local CLI. Existing
legacy download-control tokens keep their previous permissions; revoke unneeded devices in the UI.

The extension preserves an optional URL prefix, such as `https://nas.example/mdm/`; the proxy
must route that prefix to MDM and preserve its Host/Origin policy. Prefix preservation in the
extension is implemented, but end-to-end reverse-proxy deployment remains a separate validation
task. Plain HTTP, embedded server credentials, query strings and fragments are refused in the
server setting. Tokens are local to this browser and are not sent through redirects.

Opening the extension popup checks the actual server response rather than assuming a saved token
means it is online. The URL remains visible and **Open Mister Download Manager** opens that
server's web interface. If the token is revoked, pair again using a new code; the extension keeps
the configured URL and never silently switches to a local MDM. Only selecting **This computer**
switches the destination back.

The browser download is canceled only after a valid acceptance response. Failed or malformed
responses leave it with the browser. The legacy API has no idempotency support, so a request whose
reply is lost can still have been accepted by the server; the extension does not automatically
retry it. Check the web list before sending again. Torrent metadata requests still use the
existing `pending` acknowledgment; persistent job IDs and durable capture acceptance are planned.

## What a server doesn't do

- Video pages (yt-dlp) and image galleries need their helpers, which the image doesn't include.
- Nothing opens a window: a download that would ask a question (a duplicate, files already
  there) takes the choice set in the settings, or isn't added.
- Don't publish the port on the internet; reach your home network through a VPN instead.
