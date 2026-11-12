# Testing the experimental qBittorrent API

This runbook tests Webmister, **not your existing qBittorrent**. The adapter is experimental;
passing these checks does not establish replacement readiness. Record actual results rather than
assuming success. See [compatibility notes](qbittorrent-compatibility.md) for current limitations.

## 1. Prepare an isolated instance

- Use a Linux host with Docker Compose and a checkout containing the adapter. Build a **local
  image** from that checkout: a published image may not contain these changes.
- Use a separate Compose project, image tag, configuration directory, downloads directory and
  unused port. Do not use the existing qBittorrent configuration or downloads as writable test
  mounts. Create test directories owned by the configured `MDM_UID:MDM_GID`.
- Copy `docker/.env.example` to a **separate test environment file**, set those test paths and
  port, and set `MDM_QBIT_API=1`. Keep `MDM_QBIT_AUTH_BYPASS=0`. Start with
  `docker compose -p webmister-qbit-test --env-file <test-env> -f docker/compose.yml up -d --build`
  from the repository root. Relative paths in the environment file are resolved relative to
  `docker/compose.yml`, not necessarily the shell directory.
- The allowlist defaults to loopback **inside the container**. Determine the actual source IPs
  seen by Webmister (including Docker gateway/NAT as applicable) and explicitly set
  `MDM_QBIT_ALLOWED_SUBNETS` to trusted peers only. `/0`, invalid entries and forwarded-IP
  headers are rejected. Restart the test service after environment changes. Configure
  `MDM_REMOTE_URL`, `MDM_REMOTE_HOSTS` and port binding for the address used by each client.
  Keep the endpoint on a protected network; HTTP sends credentials unencrypted.
- Set up the Webmister owner account using the pairing code and Web UI as described in
  [server setup](server.md#set-up-the-web-ui-owner). Confirm you can log in to the normal UI.
  Disable any automatic handling in Sonarr/Radarr until ready to test.

Run the following commands from a trusted Linux shell with `curl`. The URL must reach the **test
instance**, not qBittorrent. With a self-signed HTTPS certificate, configure curl to trust the
certificate rather than disabling verification. Avoid sharing terminal output containing cookies,
credentials, private paths or API keys.

```bash
BASE='http://127.0.0.1:8463'  # replace with your test address and port
COOKIE="$(mktemp)"
chmod 600 "$COOKIE"
```

## 2. Authentication and discovery

With bypass disabled, an unauthenticated version request from an allowed peer should return 403:

```bash
curl -sS -i "$BASE/api/v2/app/webapiVersion"
```

Log in using the Webmister owner account. `read -s` avoids putting the password in shell history;
the expanded curl argument may still be visible to privileged local process observers.

```bash
read -r -p 'Webmister owner username: ' USERNAME
read -r -s -p 'Webmister owner password: ' PASSWORD; printf '\n'
curl -sS -i -c "$COOKIE" --data-urlencode "username=$USERNAME" \
  --data-urlencode "password=$PASSWORD" "$BASE/api/v2/auth/login"
unset PASSWORD
```

Expect HTTP 200, body `Ok.` and a `SID` cookie. HTTP 200 with `Fails.` is a failed login,
not a pass. Avoid repeated guesses: normal login throttling applies. Then:

```bash
curl -sS -i -b "$COOKIE" "$BASE/api/v2/app/webapiVersion"
curl -sS -i -b "$COOKIE" "$BASE/api/v2/app/version"
curl -sS -i -b "$COOKIE" "$BASE/api/v2/app/preferences"
```

Expect version bodies `2.8.0` and `v4.3.0` and a JSON preferences object with the actual
`save_path`. These version strings are discovery compatibility responses, **not** full API parity.

## 3. Categories and admission

Create a unique test category and inspect its returned path:

```bash
CAT='webmister-api-test'
curl -sS -i -b "$COOKIE" --data-urlencode "category=$CAT" \
  "$BASE/api/v2/torrents/createCategory"
curl -sS -i -b "$COOKIE" "$BASE/api/v2/torrents/categories"
```

Expect 200 and the category in the JSON listing. A duplicate can return 409. Check the actual
folder and permissions; custom category save paths are not supported by this adapter.

Use a small, legally distributable torrent with available peers and no pre-existing files in the
test directory. Set `MAGNET` to its magnet URI and submit it as a URL-encoded form:

```bash
MAGNET='REPLACE_WITH_TEST_MAGNET'
curl -sS -i -b "$COOKIE" --data-urlencode "urls=$MAGNET" \
  --data-urlencode "category=$CAT" "$BASE/api/v2/torrents/add"
curl -sS -i -b "$COOKIE" "$BASE/api/v2/torrents/info"
```

Alternatively, use a **different** test torrent file to exercise multipart upload:

```bash
TORRENT_FILE='/path/to/test.torrent'
curl -sS -i -b "$COOKIE" -F "torrents=@$TORRENT_FILE" -F "category=$CAT" \
  "$BASE/api/v2/torrents/add"
```

An HTTP 200 `Ok.` means MDM accepted asynchronous admission, **not** that metadata arrived or
the download succeeded. Watch the Webmister UI and logs for lookup failures or existing-file
confirmation. Wait for the torrent to appear in `info`, download and complete; record failures.
Unsupported add options can return 501. Do not retry blindly against existing files.

## 4. Inspect and control a test torrent

Copy the test torrent's 40-character v1 `hash` from `info`; do not use `all`.

```bash
HASH='REPLACE_WITH_TEST_HASH'
curl -sS -i -b "$COOKIE" --get --data-urlencode "hash=$HASH" "$BASE/api/v2/torrents/files"
curl -sS -i -b "$COOKIE" --get --data-urlencode "hash=$HASH" "$BASE/api/v2/torrents/properties"
curl -sS -i -b "$COOKIE" --data-urlencode "hashes=$HASH" "$BASE/api/v2/torrents/pause"
curl -sS -i -b "$COOKIE" --data-urlencode "hashes=$HASH" "$BASE/api/v2/torrents/resume"
```

Check file names, progress, category, state, `save_path`, `content_path` and `root_path` against
the UI and actual files. Test pause/resume while downloading and verify state changes rather than
relying on HTTP status alone. Pausing or removing a completed torrent still seeding can return
unavailable. Per-file progress can be zero when unavailable; truncated metadata may return 503.

Optional: test share limits on the isolated torrent; `-2` means use the global limit. A specific
inactive-seeding limit is unsupported and should return 501.

```bash
curl -sS -i -b "$COOKIE" --data-urlencode "hashes=$HASH" \
  --data-urlencode 'ratioLimit=1.5' --data-urlencode 'seedingTimeLimit=60' \
  --data-urlencode 'inactiveSeedingTimeLimit=-2' "$BASE/api/v2/torrents/setShareLimits"
```

## 5. Sonarr and Radarr end-to-end

1. Record Webmister commit/image, Sonarr and Radarr versions, Compose networks, actual peer IPs,
   UID/GID and relevant mounts. Never include credentials or API keys in the record.
2. Add Webmister as a **separate** qBittorrent download client in each app, with test categories;
   leave the existing client and its data untouched. Test each client's connection and category
   creation. Prevent production grabs from being routed to the experimental client.
3. Inspect real deployed mounts and remote-path mappings. Confirm every returned completed path
   refers to the **same file** inside the client container, with read/import permissions. A
   matching path string alone is not proof that mounts point to the same storage.
4. Submit controlled test releases through each client. Verify admission, completion, file
   visibility, actual import and seeding; capture status and sanitized logs for failures.
5. Restart only the test Webmister service and verify torrent list, paths and seeding recovery.
   Test pause/resume and removal on disposable torrents. Note unsupported behavior explicitly.

## 6. Cleanup and results

Removal is a destructive operation. Only after verifying the hash belongs to a disposable test
torrent, remove its entry while keeping files:

```bash
curl -sS -i -b "$COOKIE" --data-urlencode "hashes=$HASH" \
  --data-urlencode 'deleteFiles=false' "$BASE/api/v2/torrents/delete"
curl -sS -i -b "$COOKIE" -X POST "$BASE/api/v2/auth/logout"
rm -f "$COOKIE"
```

Removal and file cleanup are asynchronous; a successful response does not confirm file deletion.
Stop only the separate test project with `docker compose -p webmister-qbit-test --env-file
<test-env> -f docker/compose.yml down`. Retain test data and logs until reviewed. Do not delete
the existing qBittorrent project or files.

Record **pass / fail / not tested** for: isolation and authentication; discovery; category;
magnet; torrent upload; download; paths; Sonarr import; Radarr import; seeding; pause/resume;
restart; and removal. A failure or untested item means replacement readiness remains unverified.
