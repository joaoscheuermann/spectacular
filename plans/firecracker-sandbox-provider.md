# Firecracker and Docker Sandbox Providers

## Decision

Doric uses a provider-neutral contract owned by `packages/sandbox`. Docker remains the default provider and Firecracker is selected with `DORIC_SANDBOX_PROVIDER=firecracker`. Every sandbox receives explicit CPU, memory, and writable-layer disk limits. Networking is denied by default; optional SSH is key-only and loopback-bound unless an advertised remote host is explicitly configured.

Docker, Firecracker, and Sandpool depend inward on Sandbox. Firecracker talks directly to Firecracker/KVM and public OCI registries; it must not import Docker, mount a Docker socket, or create helper containers.

## Contract

`SandboxProvider.provision` returns a `SandboxRuntime` supporting exec, binary transfer, optional SSH access, and idempotent disposal. `createSandbox` adds provider-neutral root/path validation, Git clone and diff helpers, text-file helpers, timeouts/aborts, and disposed-session guards. The root defaults to `/workspace`.

Resources are `{ cpuCount, memoryMiB, diskMiB }`; disk is writable-layer capacity excluding the immutable image. Network policy is `disabled` or public `egress`, with IP-literal DNS servers, explicit CIDR/protocol/port private exceptions, and optional SSH configuration. Omitted networking normalizes to disabled with SSH off. SSH promotes disabled networking to effective egress, defaults to loopback and an automatic port, and remote binding requires `advertisedHost`. Effective egress requires DNS and blocks host, peer, loopback, link-local, metadata, and private destinations unless exactly allowed.

## Providers

Docker maps resources to Nano CPUs, memory bytes, and writable-layer quota, failing actionably when quota or host-side nftables isolation cannot be guaranteed. SSH uses per-sandbox Ed25519 keys and pinned Dropbear. Provisioning and cleanup are transactional and idempotent without workload `NET_ADMIN` or helper containers.

Firecracker v1 targets Linux x86_64/KVM and uses Firecracker 1.16.1, jailer, `/dev/kvm`, `/dev/net/tun`, cgroup v2, nftables, a pinned Linux 6.18 kernel, and a reproducible BusyBox/Dropbear initramfs. It uses the Unix-socket HTTP API directly, one `/30` per VM, a read-only OCI-derived ext4 base disk, and a sparse ext4 writable overlay disk. OCI acquisition uses pinned skopeo/umoci directly, validates linux/amd64 and safe extraction, and publishes digest/version-keyed images atomically into a persistent 20 GiB in-use-aware LRU cache.

Each jailed VM owns its process, socket, disks, TAP, firewall rules, proxy, keys, and state directory. Management exec/file transfer uses private SSH and bundled static helpers. Startup reconciles stale owned resources; failure, cancellation, timeout, and disposal clean up transactionally.

## Container harness

A multi-stage Linux Doric image contains the built agent, Firecracker/jailer, verified kernel/initramfs, skopeo, umoci, ext4/iproute2/nftables/OpenSSH tooling, and bootstrap artifacts. The Firecracker Compose profile uses host networking, KVM/TUN, writable cgroup/state/cache mounts, and required jailer/network privileges without a Docker socket. The Docker profile mounts the Docker socket and grants host-network firewall access. This privileged setup is a development/e2e harness, not a production isolation boundary.

## Verification

Unit and contract tests cover validation, network normalization, exec and binary transfer, Git helpers, SSH/disposal guards, Sandpool capacity/forwarding, Docker resource/SSH/firewall cleanup, Firecracker API/preflight/OCI/cache/network/readiness cleanup, and secret-safe errors. Linux e2e runs the same `node:22-slim` scenarios against both providers, including strict-host-key SSH, public and protected networking, disk isolation/limits, cache reuse, pool lifecycle, and complete resource cleanup. Firecracker e2e requires a dedicated Linux x86_64 KVM runner and fails clearly without KVM.

## Deferred

Firecracker v1 excludes non-amd64 guests, authenticated registries, daemon-local images, snapshots, suspend/resume, IPv6, domain egress, public SSH by default, multi-replica shared state, production container hardening, and A2A SSH credential distribution.
