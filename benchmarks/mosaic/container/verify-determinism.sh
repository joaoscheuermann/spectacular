#!/bin/sh
set -eu

if [ "$#" -ne 4 ]; then
  echo "usage: verify-determinism.sh <image@sha256:digest> <scores.csv> <config.json> <output-directory>" >&2
  exit 64
fi

image=$1
scores=$2
config=$3
output_root=$4

case "$image" in
  *@sha256:*|sha256:*) ;;
  *)
    echo "analysis image must be addressed by an immutable digest" >&2
    exit 65
    ;;
esac

docker image inspect "$image" >/dev/null
mkdir -p "$output_root"
run_dir=$(mktemp -d "$output_root/determinism.XXXXXX")
scores_dir=$(CDPATH= cd -- "$(dirname -- "$scores")" && pwd)
config_dir=$(CDPATH= cd -- "$(dirname -- "$config")" && pwd)
scores_name=$(basename -- "$scores")
config_name=$(basename -- "$config")

run_analysis() {
  destination=$1
  docker run --rm --pull never --network none --read-only \
    --cap-drop ALL --security-opt no-new-privileges \
    --tmpfs /tmp:rw,noexec,nosuid,size=256m --env TMPDIR=/tmp \
    --user "$(id -u):$(id -g)" \
    --mount "type=bind,src=$scores_dir,dst=/input/scores,readonly" \
    --mount "type=bind,src=$config_dir,dst=/input/config,readonly" \
    --mount "type=bind,src=$run_dir,dst=/output" \
    "$image" /benchmark/analysis/analyze.R \
    "/input/scores/$scores_name" "/input/config/$config_name" "/output/$destination"
}

run_report() {
  result=$1
  destination=$2
  docker run --rm --pull never --network none --read-only \
    --cap-drop ALL --security-opt no-new-privileges \
    --tmpfs /tmp:rw,noexec,nosuid,size=256m --env TMPDIR=/tmp \
    --user "$(id -u):$(id -g)" \
    --mount "type=bind,src=$scores_dir,dst=/input/scores,readonly" \
    --mount "type=bind,src=$run_dir,dst=/output" \
    "$image" /benchmark/analysis/report.R \
    "/output/$result" "/input/scores/$scores_name" "/output/$destination"
}

run_analysis result-1.json
run_analysis result-2.json
cmp "$run_dir/result-1.json" "$run_dir/result-2.json"
run_report result-1.json report-1.md
run_report result-2.json report-2.md
cmp "$run_dir/report-1.md" "$run_dir/report-2.md"
shasum -a 256 \
  "$run_dir/result-1.json" "$run_dir/result-2.json" \
  "$run_dir/report-1.md" "$run_dir/report-2.md" >"$run_dir/SHA256SUMS"
echo "$run_dir"
