# MOSAIC benchmark

This private benchmark compares `mosaic-direct` and `mosaic` on the same public
tasks. Its primary reading is the paired quality delta: MOSAIC wins when its
mean reward is higher on valid evidence from the same tasks. Cost per reward
unit is the efficiency reading, and a strict quality-up/cost-down Pareto win is
retained as a stronger aspirational result. SkillsBench is the primary
benchmark; Terminal-Bench 2 is a secondary confirmation that may run only after
a valid SkillsBench comparison.

The former empirical protocol 0.2 is historical. Its complete sources and
artifacts are preserved in the
[mosaic-validation-v0.2-archive release](https://github.com/joaoscheuermann/spectacular/releases/tag/mosaic-validation-v0.2-archive).

## What SkillsBench establishes

The primary question is deliberately narrow:

> With the same model on the same tasks and under the same conditions, does
> MOSAIC improve result quality, and what does each unit of reward cost?

SkillsBench compares two arms:

| Arm             | Agent behavior                                                         |
| --------------- | ---------------------------------------------------------------------- |
| `mosaic-direct` | A minimal direct model-and-terminal agent loop                         |
| `mosaic`        | The same model and tools with MOSAIC planning, revision, and execution |

Both arms receive the same 87 public SkillsBench tasks, task prompt, required
task skills, terminal and sandbox, provider, model, low reasoning effort,
single-shot policy, and zero retries. The treatment being measured is the
MOSAIC orchestration. Each task is graded by its official SkillsBench verifier,
while BenchFlow records trusted model usage and cost.

The generated adapter fixes `low` effort in both arms. The campaign does not
pass BenchFlow's ACP reasoning-effort option because the external manifest
contract cannot declare the config-option identifier that BenchFlow requires;
the closed comparator therefore requires the harness field to remain null.

The ACP adapter treats only an authentic agent-owned
`invalid_structured_output` exhaustion as a scoreable task failure. It writes
only that safe code and returns `end_turn`, so the official verifier runs and
the Direct arm can continue. Other authentic Agent or Provider failures become
JSON-RPC `-32603` errors carrying at most `{source, code}`. Unknown failures
carry no data. Messages, diagnostics, causes, stacks, and captured error objects
never cross the wire or stderr.

The primary decision and efficiency readings are:

```text
quality_win = mean_reward(mosaic) > mean_reward(mosaic-direct)
cost_per_reward = total_model_cost / sum(task_reward)
pareto_win = quality_win AND total_model_cost(mosaic) < total_model_cost(direct)
```

The report also lists task-level MOSAIC wins, regressions, and ties. Before
applying these readings, the comparator rejects evidence unless both arms
have the exact same task set and neutral configuration, valid run and health
artifacts, no runtime or verifier errors, finite rewards, and trusted positive
usage and cost telemetry. Missing tasks, mismatched settings, or incomplete
telemetry therefore cannot produce a MOSAIC win.

A quality win establishes an observed improvement on the pinned paired
SkillsBench campaign. It does not by itself establish statistical significance
or universal superiority across models, benchmarks, or repeated stochastic
runs. A Pareto win supports the stronger additional claim that the observed
quality improvement also used less total model cost.

Terminal-Bench 2 is a secondary confirmation of whether that result transfers
beyond the skill-oriented primary benchmark; it can run only after a valid
SkillsBench comparison.

## Layout

```text
agents/       BenchFlow manifests for mosaic-direct and mosaic
src/          ACP adapters, campaign domain, comparison, and Nx host
tests/        Isolated Node.js tests
dist/         Generated ACP, host, and Docker-launcher bundles
results/      Ignored, fresh paid-campaign artifacts
```

`mosaic-bench-acp.mjs` is the released agent bundle used inside benchmark
containers. `mosaic-bench-host.mjs` is the generated local Nx dispatcher for
host-only orchestration such as campaign resume. `mosaic-bench-docker.mjs`
builds and starts the portable Linux coordinator. None of these files is source
code; do not invoke them directly.

## Commands

Run from the repository root. Every command below uses the Nx target; do not
invoke the generated bundle directly.

```sh
# Free portable preflight: image, Docker, Git, release assets, and pinned refs.
npx nx run mosaic-benchmark:docker-run -- campaign skillsbench check

# Paid one-task smoke. Explicit confirmation is required.
npx nx run mosaic-benchmark:docker-run -- campaign skillsbench smoke --yes-paid-run

# Paid diagnostic pilot: a fixed, varied subset of 10 tasks.
npx nx run mosaic-benchmark:docker-run -- campaign skillsbench pilot --yes-paid-run

# Resume an interrupted or errored pilot in its existing campaign directory.
npx nx run mosaic-benchmark:docker-run -- campaign skillsbench resume \
  --campaign benchmarks/mosaic/results/<skillsbench-pilot-campaign> \
  --yes-paid-run

# Paid primary campaign: SkillsBench v1.1, 87 tasks.
npx nx run mosaic-benchmark:docker-run -- campaign skillsbench run --yes-paid-run

# Paid secondary confirmation: Terminal-Bench 2, 89 tasks.
npx nx run mosaic-benchmark:docker-run -- campaign terminalbench run \
  --yes-paid-run \
  --skillsbench-report benchmarks/mosaic/results/<skillsbench-campaign>/compare.json

# Compare the two arm directories from one campaign.
npx nx run mosaic-benchmark:run -- compare \
  --direct benchmarks/mosaic/results/<campaign>/mosaic-direct \
  --mosaic benchmarks/mosaic/results/<campaign>/mosaic \
  --report benchmarks/mosaic/results/<campaign>/compare.json

# Build the standalone ACP bundle and write its SHA-256 sidecar.
npx nx run mosaic-benchmark:release
```

`docker-run` builds `mosaic-benchmark:local` from pinned Linux base images and
runs BenchFlow against a nested Docker daemon. The host needs Node/Nx and a
Docker daemon running Linux containers; `uvx`, Python, Git, and curl are inside
the coordinator image. The target works with native Linux and Docker Desktop
from Windows, macOS, or WSL. It accepts only `campaign` commands; use `run` for
local comparison and `release` for release artifacts.

The coordinator runs with `--privileged`, does not mount the host Docker socket,
and bind-mounts only `benchmarks/mosaic/results`. Named volumes
`mosaic-benchmark-docker` and `mosaic-benchmark-uv` retain task-image and Python
tool caches between runs. The image is rebuilt on every invocation, with normal
Docker layer caching making unchanged builds fast.

## Paid execution checklist

Complete every item below before running `smoke`, `pilot`, `resume`, or `run`:

- [ ] Run from the repository root on the benchmark revision intended for the
      campaign.
- [ ] Confirm Docker is running in Linux-container mode. For the recommended
      `docker-run` target, `uvx`, Python, Git, and curl come from the image.
- [ ] Provide `OPENROUTER_API_KEY` through the process environment. Never place the
      credential in a manifest, command argument, committed file, or result
      directory.
- [ ] Validate the exact adapter source and generated standalone bundle:

  ```sh
  npx nx run mosaic-benchmark:typecheck --skip-nx-cache
  npx nx run mosaic-benchmark:test --skip-nx-cache
  npx nx run mosaic-benchmark:release
  ```

- [ ] Confirm the public `mosaic-benchmark-v0.1.15` release contains exactly
      `mosaic-bench-acp.mjs` and `mosaic-bench-acp.mjs.sha256`. The generated
      bundle hash, published sidecar, and `BF_BUNDLE_SHA256` in both agent
      manifests must be identical.
- [ ] Run the free preflight for the benchmark being purchased and inspect its
      JSON output. Every individual check and the top-level `ok` field must be
      `true`:

  ```sh
  npx nx run mosaic-benchmark:docker-run -- campaign skillsbench check
  # Or, before a Terminal-Bench campaign:
  npx nx run mosaic-benchmark:docker-run -- campaign terminalbench check
  ```

- [ ] Review the fixed treatment before approving spend: model
      `openrouter/deepseek/deepseek-v4-pro-0813`, low reasoning effort, two sequential arms, required
      usage tracking, zero retries, and one task/build worker at a time.
- [ ] Confirm the available provider budget. `--yes-paid-run` is the explicit
      acknowledgement that the command may incur model and container costs; it
      does not bypass any preflight check.

For a paid smoke, run exactly one task in each arm:

```sh
npx nx run mosaic-benchmark:docker-run -- \
  campaign skillsbench smoke --yes-paid-run
```

The smoke is operational evidence only. It confirms that both agents install,
launch, use the provider, execute the task, and produce BenchFlow artifacts; it
does not establish the full-benchmark result. Do not start the full campaign
until the smoke command exits successfully and both arm directories exist in
the newly created campaign directory under `results/`.

For a diagnostic pilot, run the same two arms against this fixed, predeclared
ten-task subset: `data-to-d3`, `earthquake-phase-association`, `edit-pdf`,
`jax-computing-basics`, `organize-messy-files`,
`pptx-reference-formatting`, `sec-financial-report`,
`spring-boot-jakarta-migration`, `travel-planning`, and `xlsx-recover-data`.

```sh
npx nx run mosaic-benchmark:docker-run -- \
  campaign skillsbench pilot --yes-paid-run
```

The pilot is a varied diagnostic sample, not a statistical benchmark result.
Its comparison is valid only when the run config contains exactly those ten
tasks. It neither replaces the 87-task primary campaign nor satisfies the
SkillsBench evidence gate required for Terminal-Bench.

If a pilot stops because one or more Direct tasks are unscored, fix the host
problem and resume the existing campaign instead of starting another one:

```sh
npx nx run mosaic-benchmark:docker-run -- \
  campaign skillsbench resume \
  --campaign benchmarks/mosaic/results/<skillsbench-pilot-campaign> \
  --yes-paid-run
```

Resume is intentionally limited to SkillsBench pilots. It validates the exact
campaign directory, recorded metadata and artifact hashes, fixed task set,
released ACP bundle, and agent manifest before the paid preflight. It also
requires Docker to expose at least 8 CPUs and 8 GiB. BenchFlow reuses each
arm's scored rollouts from the existing `jobs/` directory, reruns only unscored
tasks, and starts or resumes MOSAIC first. Direct runs only after MOSAIC exits
without errors. Resume accepts valid existing evidence from either arm, including
older campaigns that started with Direct. The
operation preserves the original `campaignId` and `pilot` action and holds an
exclusive campaign lock. Before and after each returned arm attempt, `jobs/`
retains only the newest result per task while replaced or incomplete rollout
directories remain available under `attempts/`. BenchFlow output and explicit
host stages stream live to `stderr`; the final machine-readable result remains
on `stdout`.

Resume is release-bound: it refuses a campaign whose recorded bundle or
manifest differs from the current release. After an adapter release, start a
new pilot rather than mixing agent implementations in one comparison.

For a full paid SkillsBench run, execute all 87 tasks in each arm:

```sh
npx nx run mosaic-benchmark:docker-run -- \
  campaign skillsbench run --yes-paid-run
```

After any paid action, compare the two arm directories from that same
campaign. A completed campaign command only means that both arms exited
successfully; the comparison determines whether the evidence is valid, whether
MOSAIC improved paired quality, its cost per reward, and whether it also won the
strict Pareto reading:

```sh
npx nx run mosaic-benchmark:run -- compare \
  --direct benchmarks/mosaic/results/<campaign>/mosaic-direct \
  --mosaic benchmarks/mosaic/results/<campaign>/mosaic \
  --report benchmarks/mosaic/results/<campaign>/compare.json
```

Comparison exit code `0` means valid evidence and a paired quality win, `1`
means valid evidence without a quality win, and `2` means invalid or incomplete
evidence. `paretoWin` remains an independent stronger indicator. Never
combine arms from different campaign directories. A paid Terminal-Bench run
additionally requires the valid comparison report from a full 87-task
SkillsBench campaign; a one-task smoke report is not sufficient.

`check` is free and never calls a model. `smoke`, `pilot`, `resume`, and `run`
stop before spawning anything unless `--yes-paid-run` is present, and they
repeat the free preflight before changing results. A paid Terminal-Bench
command also requires a report from a valid SkillsBench comparison. New
campaigns create a directory below `results/`; resume is the only operation
that may continue an existing directory. The two arms use separate jobs, task
manifest, run config, health summary, and non-secret metadata.

SkillsBench is pinned to
`b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af` and uses `with-skill` mode. Its
smoke task is `jax-computing-basics`; its pilot task set is fixed in `src/pilot.ts`.
Terminal-Bench 2 is pinned to
`2fd12b88aafdd04a52c298e3940bcb189f9766d6`, uses `no-skill` mode, and its
smoke task is `regex-log`. Both arms use `openrouter/deepseek/deepseek-v4-pro-0813`, low reasoning
effort, Docker, single-task/build concurrency, single-shot looping, zero
retries, and required usage tracking.

## Environment and safety

BenchFlow resolves the host's `OPENROUTER_API_KEY`, routes the explicit
`openrouter/deepseek/deepseek-v4-pro-0813` model through its LiteLLM proxy, and maps the
proxy URL, ephemeral key, and model alias into `OPENROUTER_BASE_URL`,
`OPENROUTER_API_KEY`, and `OPENROUTER_MODEL` inside the task container. Both
arms compose `createUnifiedProvider`; without a proxy URL it defaults to
`https://openrouter.ai/api/v1`. The launcher transfers only the ephemeral proxy
key through a mode-0600 temporary file, removes it from the Node environment,
and the provider unlinks the file before accepting prompts. The host credential
never enters the agent container. Terminal subprocesses also receive an
environment with credential-shaped names removed. Never place credentials in
manifests, command arguments, metadata, task artifacts, or commits.
Both manifests pin the generated bundle SHA-256 literally and verify the
official Node archive checksum for the selected architecture. The free
preflight requires the local bundle, manifest pins, and published sidecar to
agree before any paid run.
`BENCHFLOW_AGENTS_DIR` is set internally to `agents/` for each paid run. The
portable coordinator is privileged because it owns a nested Docker daemon; run
only the image built from this repository. It does not mount the host Docker
socket. Docker executes untrusted benchmark tasks, so inspect the free preflight
before approving cost.

Comparison validates identical neutral treatment metadata, task manifests and
bundle digest, exact task sets, error-free verifier results, finite rewards,
trusted positive usage/cost telemetry, and non-negative tool-call telemetry.
It reports aggregate reward, cost per reward unit, score delta, task-level wins,
regressions, and ties. Its exit code is `0` for a paired quality win, `1` for a
valid quality non-win, and `2` for invalid evidence; strict Pareto remains a
separate aspirational flag.
