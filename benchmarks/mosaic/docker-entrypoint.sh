#!/bin/sh
set -eu

dockerd_log=/var/log/mosaic-dockerd.log
dind dockerd --host=unix:///var/run/docker.sock > "$dockerd_log" 2>&1 &
dockerd_pid=$!

stop_daemon() {
  kill "$dockerd_pid" 2>/dev/null || true
  wait "$dockerd_pid" 2>/dev/null || true
}

trap stop_daemon EXIT

ready=0
attempt=0
while [ "$attempt" -lt 90 ]; do
  if docker info >/dev/null 2>&1; then
    ready=1
    break
  fi
  if ! kill -0 "$dockerd_pid" 2>/dev/null; then
    break
  fi
  attempt=$((attempt + 1))
  sleep 1
done

if [ "$ready" -ne 1 ]; then
  echo 'The benchmark Docker daemon did not become ready.' >&2
  cat "$dockerd_log" >&2
  exit 1
fi

set +e
node /benchmark/dist/mosaic-bench-host.mjs "$@"
status=$?
set -e

case "${MOSAIC_HOST_UID:-}:${MOSAIC_HOST_GID:-}" in
  *[!0-9:]* | :)
    ;;
  *)
    chown -R "$MOSAIC_HOST_UID:$MOSAIC_HOST_GID" /benchmark/results 2>/dev/null || true
    ;;
esac

exit "$status"
