# Docker sandbox provider

`docker` implements the provider-neutral `sandbox` runtime using the Docker
Engine API. It requests CPU, memory, and writable-layer disk limits whenever a
sandbox is provisioned.

## Writable-layer disk limits

Docker accepts a writable-layer limit through `StorageOpt.size` only when its
storage driver and backing filesystem support per-container quotas. For
example, Docker's `overlay2` driver requires an XFS backing filesystem mounted
with the `pquota` option.

The provider first creates each sandbox with the requested disk limit. If the
daemon specifically rejects that option, it retries once without
`StorageOpt.size` so Docker Desktop installations on unsupported filesystems
can still run sandboxes. In that fallback, CPU and memory limits remain
enforced, but the sandbox writable layer is not size-limited by Docker.

Use a quota-capable Docker host when writable-layer disk isolation is required.
