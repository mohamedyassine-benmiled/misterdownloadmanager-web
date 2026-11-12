#!/usr/bin/env bash
# Linux CI smoke gate. Fresh tmpfs state only; no user mounts or uploaded runtime logs.
set -euo pipefail
image=${1:?image tag required}
name="mdm-web-smoke-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}"
entrypoint=$(docker image inspect --format '{{json .Config.Entrypoint}}' "$image")
python3 - "$entrypoint" <<'PY'
import json
import sys
assert json.loads(sys.argv[1]) == ['misterdownloadmanager', '--web'], 'image must enable Web mode explicitly'
PY
port=$(python3 - <<'PY'
import socket
with socket.socket() as probe:
    probe.bind(('127.0.0.1', 0))
    print(probe.getsockname()[1])
PY
)
created=false
cleanup() { if [ "$created" = true ]; then docker rm --force "$name" >/dev/null 2>&1 || true; fi; }
trap cleanup EXIT
docker run --detach --name "$name" --network host \
  --tmpfs /config:rw,nosuid,nodev,uid=1000,gid=1000,mode=0700 \
  --tmpfs /downloads:rw,nosuid,nodev,uid=1000,gid=1000,mode=0700 \
  --env MDM_OFFSCREEN=1 --env MDM_REMOTE_URL= \
  "$image" --webhost=127.0.0.1 --port "$port" >/dev/null
created=true
ready=false
for attempt in $(seq 1 60); do
  if docker exec "$name" misterdownloadmanager-cli health >/dev/null 2>&1; then
    ready=true
    break
  fi
  if [ "$(docker inspect --format '{{.State.Running}}' "$name")" != true ]; then
    echo "Web image exited before readiness (runtime logs withheld: they contain enrollment codes)." >&2
    exit 1
  fi
  sleep 2
done
[ "$ready" = true ] || { echo "Web image readiness timed out." >&2; exit 1; }
[ "$(docker exec "$name" id -u)" = 1000 ]
python3 - "$port" <<'PY'
import json
import ssl
import sys
import urllib.request
base = f'https://127.0.0.1:{sys.argv[1]}/'
context = ssl._create_unverified_context()  # isolated fixture's generated self-signed cert
with urllib.request.urlopen(base + 'api/v1/auth/session', context=context, timeout=10) as response:
    view = json.load(response)
    assert view['configured'] is False and view['authenticated'] is False and view['owner'] is False
with urllib.request.urlopen(base, context=context, timeout=10) as response:
    assert b'<html' in response.read().lower()
PY
docker stop --time 60 "$name" >/dev/null
[ "$(docker inspect --format '{{.State.ExitCode}}' "$name")" = 0 ]
echo "Web image: non-root startup, --web/--port/--webhost, HTTPS, owner gate, health and clean stop passed."
