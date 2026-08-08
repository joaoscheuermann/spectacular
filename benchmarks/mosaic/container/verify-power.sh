#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: verify-power.sh <image@sha256:digest> <config.json> <output-directory>" >&2
  exit 64
fi

image=$1
config=$2
output_root=$3

case "$image" in
  *@sha256:*|sha256:*) ;;
  *)
    echo "analysis image must be addressed by an immutable digest" >&2
    exit 65
    ;;
esac

docker image inspect "$image" >/dev/null
mkdir -p "$output_root"
run_dir=$(mktemp -d "$output_root/power-determinism.XXXXXX")
config_dir=$(CDPATH= cd -- "$(dirname -- "$config")" && pwd)
config_name=$(basename -- "$config")

run_once() {
  destination=$1
  docker run --rm --pull never --network none --read-only \
    --cap-drop ALL --security-opt no-new-privileges \
    --tmpfs /tmp:rw,noexec,nosuid,size=256m --env TMPDIR=/tmp \
    --user "$(id -u):$(id -g)" \
    --mount "type=bind,src=$config_dir,dst=/input/config,readonly" \
    --mount "type=bind,src=$run_dir,dst=/output" \
    "$image" /benchmark/analysis/power.R \
    "/input/config/$config_name" "/output/$destination"
}

run_once power-1.json
run_once power-2.json
cmp "$run_dir/power-1.json" "$run_dir/power-2.json"
shasum -a 256 "$run_dir/power-1.json" "$run_dir/power-2.json" >"$run_dir/SHA256SUMS"
echo "$run_dir"
