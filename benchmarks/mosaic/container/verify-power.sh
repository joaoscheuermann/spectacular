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
config_digest=$(shasum -a 256 "$config" | awk '{print $1}')
run_dir="$output_root/power-$config_digest"
mkdir -p "$run_dir"
config_dir=$(CDPATH= cd -- "$(dirname -- "$config")" && pwd)
config_name=$(basename -- "$config")

run_once() {
  destination=$1
  checkpoint=$2
  if [ -f "$run_dir/$destination" ]; then
    return
  fi
  docker run --rm --pull never --network none --read-only \
    --cap-drop ALL --security-opt no-new-privileges \
    --tmpfs /tmp:rw,noexec,nosuid,size=256m --env TMPDIR=/tmp \
    --user "$(id -u):$(id -g)" \
    --mount "type=bind,src=$config_dir,dst=/input/config,readonly" \
    --mount "type=bind,src=$run_dir,dst=/output" \
    "$image" /benchmark/analysis/power.R \
    "/input/config/$config_name" "/output/$destination" "/output/$checkpoint"
}

run_once power-1.json power-1.checkpoint.json
run_once power-2.json power-2.checkpoint.json
cmp "$run_dir/power-1.json" "$run_dir/power-2.json"
shasum -a 256 "$run_dir/power-1.json" "$run_dir/power-2.json" >"$run_dir/SHA256SUMS"
echo "$run_dir"
