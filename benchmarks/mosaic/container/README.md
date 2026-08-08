# Analysis container

Build only from a locally available immutable R image reference. Restoring the
lockfile may access CRAN during image construction; the study run itself must
not have network access.

```sh
docker build --pull=false \
  --build-arg R_IMAGE='rocker/r-ver@sha256:<verified-digest>' \
  -f container/Dockerfile -t mosaic-analysis:local .
```

Record the resulting OCI digest in the freeze manifest. Run power through
`verify-power.sh` and analysis/reporting through `verify-determinism.sh`. Both
reject tags, disable pulling and networking, make the root filesystem
read-only, drop all capabilities, run twice, and require byte-identical output
before writing SHA-256 entries. Building may use the network to pull the pinned
base and restore CRAN packages; execution never does.

The ARM64 source build of `RcppEigen` needs more than 2 GiB of builder memory.
Give the Docker VM at least 4 GiB of RAM or equivalent temporary swap while
building; running the completed image does not need that extra allocation.
