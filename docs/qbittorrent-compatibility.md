# Experimental qBittorrent API work

Webmister is **not yet a verified qBittorrent replacement**. Do not replace an existing
qBittorrent container. The opt-in experimental adapter now advertises API 2.8.0 for a future
side-by-side client test, but has not been tested with real Sonarr or Radarr. Login, category
creation/listing, torrent submission, torrent pause/resume and reporting are implemented.
A file-list endpoint uses MDM's torrent metadata and live
per-file progress, or completion status for selected files. Missing per-file progress is reported
as zero; truncated metadata returns unavailable. Magnet submission returns success when MDM
accepts the metadata lookup, which can later fail or require confirmation for existing files.

The delete endpoint maps the file-deletion choice to MDM's existing remove and delete actions.
The adapter asks MDM to stop running torrents and waits briefly before removing them. Deleting
files requires the torrent to be in MDM's active session. As in MDM's existing action, cleanup is asynchronous and a successful response does not
confirm that every file was deleted. This is not yet verified as full qBittorrent removal behavior.
Pausing or removing a completed torrent that is still seeding can return unavailable; MDM's
completed seeding session is not controlled by its ordinary download pause action.

Experimental torrent-info and properties endpoints translate MDM's torrent counters and stored
paths. They return unavailable if any selected torrent has missing metadata or an unknown size. Common
filters, selected sort keys and pagination are implemented; other filters and sort keys return not
implemented. These endpoints still require validation
against real qBittorrent clients and must not be treated as a compatibility guarantee.
The experimental preferences response includes MDM's configured save path, DHT and seeding
settings; it is not a complete qBittorrent preferences object.
Assigning an existing MDM category updates the torrent's category without moving its existing
files. Clearing a category and automatic category-based moves are not yet supported.
Per-torrent ratio and seeding-time limits use MDM's existing sharing-limit logic. The separate
inactive-seeding timer is not supported; MDM's global keep-sharing setting still applies.

`MDM_QBIT_API=1` enables `POST /api/v2/auth/login`, accepting form-encoded `username` and
`password` for the existing Webmister owner account. It uses the existing login throttling and
issues a separate `SID` cookie. Sessions are validated and can be revoked through logout.
This is a development switch, not a supported deployment feature.
When HTTP is used, credentials and cookies are unencrypted; use a protected network or HTTPS.

`MDM_QBIT_ALLOWED_SUBNETS` is a comma-separated CIDR allowlist, defaulting to
`127.0.0.1/32,::1/128`. For the example Docker network it could be
`127.0.0.1/32,::1/128,172.20.0.0/24`; verify actual peer addresses before deploying.
Invalid entries, an empty list and `/0` networks fail closed. Forwarding headers are rejected:
reverse proxies are not supported by this experimental boundary. `MDM_QBIT_AUTH_BYPASS=1`
optionally permits requests without cookies from allowed peers without a password; it does not
affect Webmister's normal UI. Every container on that network could control the eventual API,
so prefer a dedicated network. Bypass is disabled by default. It also authorizes the experimental
pause/resume operations for torrents already present in MDM.

Category listing translates MDM's actual category keys and configured folders. Creation uses
MDM's existing category operation and a deterministic folder under the configured downloads root.
Earlier experimental category records are left untouched on disk, but are not read or exposed;
any migration must be explicit and preserve existing MDM categories. Custom save paths, automatic
moves on recategorization and some qBittorrent category names remain unsupported.
MDM's existing torrent-admission check rejects symlinks and Windows reparse points in existing
destination ancestors, but remains path-based and is not resistant to concurrent replacement.
The pinned librqbit filesystem storage opens output files by path after admission. Although it
offers custom storage factories, its JSON session persistence rejects factories other than the
standard filesystem factory. Race-resistant storage therefore needs a design that also preserves
session recovery; a one-time directory check is not sufficient.

The app also has an owner-only internal torrent snapshot containing actual engine/list fields.
It is not a qBittorrent response: Webmister category keys and paths cannot be substituted for
qBittorrent categories or import paths without additional mapping and validation.

Before compatibility can be claimed, exercise the complete torrent lifecycle, restart recovery,
the exact pinned Sonarr/Radarr versions and the user's Linux Docker volumes. The
user's qBittorrent data and configuration must remain intact during any side-by-side tests.

## Side-by-side test preparation

Use a separate Compose project and a locally built image from the checkout containing these
changes; a published image does not necessarily contain the adapter. Give the test its own
`MDM_CONFIG_DIR`, `MDM_DOWNLOADS_DIR`, image tag and unused `MDM_WEB_PORT`. Never mount the
existing qBittorrent configuration or downloads into the test as its writable directories.
Create the directories with the configured UID/GID before starting the container. Keep the
existing qBittorrent and its Sonarr/Radarr configuration unchanged for rollback.

Set `MDM_QBIT_API=1` in the test environment file. Determine the actual source address and
network seen by Webmister, then explicitly set `MDM_QBIT_ALLOWED_SUBNETS` to include only
the necessary trusted peers; container-to-container traffic is not loopback. Keep
`MDM_QBIT_AUTH_BYPASS=0` and use the Webmister owner credentials. Configure the URL and allowed
host names for the address clients actually use; a published port bound to loopback is not
reachable from another machine. Do not expose the experimental endpoint publicly.

Add Webmister as a **separate** qBittorrent client in Sonarr/Radarr, with test categories and
controlled test downloads. Verify the deployed container mounts and any remote-path mappings:
the paths returned by Webmister must resolve to the same files inside each client. Test discovery,
category creation, magnet and torrent-file adds, actual completion and import, seeding, controls,
restart recovery and removal. Treat failures, asynchronous admission errors and any requested
existing-file confirmation as test failures to investigate, not proof of successful operation.
Stop and remove only the separate test project when finished; retain its data until reviewed.

## Reference observations

Read-only inspection of the user's qBittorrent reports version 5.1.0, Web API 2.11.4,
categories `tv-sonarr` and `radarr` with empty category-specific paths, default `/downloads`,
and `auto_tmm_enabled=false`. Actual client settings and paths for completed torrents still
need verification. [Radarr's client](https://github.com/Radarr/Radarr/blob/develop/src/NzbDrone.Core/Download/Clients/QBittorrent/QBittorrentProxyV2.cs)
uses version-dependent add parameters and category/share-limit operations. [TorBoxarr](https://github.com/MrJoiny/TorBoxarr)
is another example of an adapter that translates this API to a different engine; its claims do
not establish Webmister compatibility. Pin source and app versions for future contract tests.
