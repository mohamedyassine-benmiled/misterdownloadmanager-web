# Control MDM from your phone

Add a download from your phone, or from another computer at home, and watch it run on the
computer where MDM is: paste a link or a magnet link, send a `.torrent` file, see the list with
its progress, and pause, start or remove downloads. Your phone opens the web workspace that MDM
itself serves; nothing is installed on the phone, and no outside service is involved.

On a desktop the remote control is **off until you turn it on**; headless mode enables it.
This fork uses an owner username/password for browser access and separate scoped tokens for
extensions. Keep it on a private LAN/VPN and do not publish the port to the internet.

![Options, Remote](images/options-remote.webp)

## Turn it on

1. **Options → Remote**.
2. Tick **Let phones and computers on the local network control MDM**.
3. Click **OK**.

The page then says **On** and shows the address to open, such as `https://192.168.1.20:8463/`.

The first time, Windows may ask whether to let MDM communicate on networks: allow it on private
networks, or the phone can't reach MDM.

## Set up and sign in

1. On the computer: **Options → Remote → Pair a device...** It shows a QR code, the address and an
   8-digit code.
2. On the phone, on the same Wi-Fi: scan the QR code with the camera (or type the address).
3. The browser warns that the connection's certificate isn't trusted. That is expected: MDM made
   its own certificate, on your computer, and no public authority knows it. To be sure the page is
   your MDM's, open the warning's details: the certificate's **SHA-256 fingerprint** is the one
   **Options → Remote** shows. Then continue to the page (**Advanced → Proceed** in Chrome,
   **Show details → visit this website** in Safari).
4. Before the owner is configured, enter the code, choose an owner username and a password of
   at least 12 bytes, and tap **Create account**. After setup, use that username/password on
   each browser; a pairing code alone no longer signs a browser into the owner workspace.

A code works once, for ten minutes, and not after five wrong tries: click **New code** for
another. To keep the page one tap away, add it to the phone's home screen (the browser's menu:
**Add to Home screen**).

The older upstream phone screenshot is not representative of this fork's redesigned workspace.

## What the page does

- **Add**: a download's address or a magnet link, or a `.torrent` file from the phone. Pick a
  **Category** (or let the file's type pick it), and **When**: **Start now**, or wait in a queue.
  A torrent is added with all its files, once MDM has read them (a magnet link can take a moment).
- **The list**: filter by status/category, search and sort, start/pause individual downloads,
  select several for bulk actions, and inspect details, descriptions and per-download limits.
  Removing history and deleting files are separate confirmed actions.
- **Queues and scheduler**: start/stop queues, edit membership, concurrency and start/stop windows.
- **Categories**: create/edit custom categories and destinations within the downloads root.
- **Application settings**, **Web UI authentication**, and **Browser extensions and devices**:
  owner-only administration, with current-password confirmation for sensitive changes.
- The bar at the top shows the total speed and the free space where your downloads go.

Downloads added from the phone are like the ones you add on the computer: they go into the
category's folder, follow the queues, the Scheduler and the speed limits, and go through the
network adapter you chose in **Options → Proxy / Socks** (and stop when it disconnects). The
page shows the language your phone uses when MDM has it, and offers light, dark and system themes.
Advanced server-feature parity remains incomplete; see [development status](web-development.md).

## Extensions and browser sessions

**Options → Remote** lists the paired devices, when each was paired and when it was last seen.
Select one and click **Remove**: it is refused from its next request on, at once (no need for
**OK**). The web workspace also provides password-confirmed pairing/revocation. New extension
tokens are capture-only; old download-control devices retain their previous scope, never owner
administration. See [server extension setup](server.md#send-downloads-from-your-browser).

Browser access uses server-side sessions, not entries in the paired-device list. Sign out to
revoke the current browser session; changing owner credentials revokes all browser sessions.
See [authentication and local recovery](webui-authentication.md). To stop all remote access on
a desktop, untick the remote-control box so nothing listens.

## Where it listens

- **Listen on**: **Every local network address** (the default) listens on each private address of
  the computer (`192.168.x.x`, `10.x.x.x`, `172.16–31.x.x`, and IPv6's `fd…`), one by one, and
  follows them when you join another network. **This computer only** listens on `127.0.0.1`. Or
  pick one address of one adapter.
- **Port**: `8463` unless another program uses it; then pick another (1024 to 65535).

Don't make MDM reachable from the internet (a router's port forwarding): the remote control is
made for your home network. To reach it while away, use a VPN to your home network instead
(WireGuard, Tailscale): with Tailscale, choose its address in **Listen on**.

## Security

What keeps the remote control safe, in short:

- **Off by default on desktop**, and nothing listens while it is off; headless mode enables it.
- **Owner browser sessions**: Argon2id passwords, protected cookies, CSRF checks, inactivity
  expiry and temporary login bans. Localhost/subnet bypass are explicit opt-ins and do not
  authorize owner administration. Both switches are initially off.
- **Scoped devices**: pairing codes work once, for ten minutes, with five paced tries. Device
  storage keeps token hashes, not reusable tokens. Newly paired extensions can capture but
  cannot read history or change settings; legacy devices retain download-control scope.
- **Your local network only**: MDM listens on the computer's private addresses, one by one, never
  on every address at once.
- **Encrypted**: HTTPS, with a certificate made on your computer and kept; its fingerprint is in
  Options, to compare with the one the phone's browser shows.
- **Host/Origin guards**: requests must name an allowed server host and pass origin checks;
  another website cannot embed MDM's page. Authorized extension origins still need a device
  token and cannot use browser administration endpoints.
- **Separate permissions**: owner settings use a new authorization boundary, never arbitrary
  desktop commands. Paired devices cannot become owners, open windows or browse host folders.

Found a problem? Please report it privately, as [SECURITY.md](../SECURITY.md) says.
