# Prompt

## Feature summary

Build Doric's first daemon/worker lifecycle slice. A raw CLI can dispatch a feature or debug job with a prompt and repo, a daemon manages worker process liveness and routing, and a worker owns the prompt-agent execution loop for that job.

## User value

Doric becomes a strict lifecycle orchestrator instead of a simple coding-agent chat. Users can start long-running development or debug jobs, observe worker status, stream worker events, and later answer human-in-the-loop workflow prompts through daemon-mediated commands.

## Business or commercial driver

Create a model- and harness-independent multi-agent software development lifecycle tool that reflects Doric's workflow philosophy instead of copying single-agent coding tools tied to a specific vendor runtime.

## Personas

- Developer/operator running `doric` locally.
- Feature/debug requester who submits a prompt and target repo.
- Future coordinator/user who must answer human-in-the-loop questions during workflow execution.

## Constraints

- Initial UI is raw CLI, not TUI.
- Daemon owns worker liveness and routing, not worker internals.
- Worker owns prompt execution, agent/tool loop, repo clone/setup, and internal state.
- CLI communicates through the daemon, not directly with workers.
- Docker sandboxing is a later target; the first slice may run workers on the host OS under a configured root folder.
- First workflow agent is the prompt/requirements agent only.
- Agents and tools should live under the worker package.

## Product requirements

1. Provide a raw CLI surface for the daemon/worker lifecycle.
2. Support dispatching jobs, for example `doric feature --prompt="<prompt>" --repo="<remote_to_clone>"` and `doric debug --prompt="<prompt>" --repo="<remote_to_clone>"`.
3. Support listing live workers and statuses via a command like `doric list`.
4. Support streaming readable worker events via a command like `doric worker <id>`.
5. Preserve daemon-mediated observation: the CLI should observe worker state through the daemon, not by talking directly to workers. This is duplicated in architecture requirements because it defines both product UX and process boundaries.
6. Start with only the prompt/requirements agent workflow until the architecture stabilizes.
7. Include a path for human-in-the-loop interaction when a workflow step needs user input.
8. Expose enough state for users to understand worker liveness, current activity, and job identity.

## Architecture requirements

1. Add a daemon binary/package responsible for spawning and managing workers.
2. Add a worker binary/package responsible for prompt execution, repo preparation, tools, agents, transcript/internal state, and the tool/agent loop.
3. Use daemon-to-worker and CLI-to-daemon communication over gRPC; evaluate QUIC only as a preference, not a blocker.
4. Keep daemon responsibility limited to worker liveness, worker registry/status, and event/state routing.
5. Keep worker responsibility over internal execution state, transcript management, tool calls, agent orchestration, repo clone/setup, and job CWD.
6. The CLI must route through the daemon for worker state and events. This is duplicated in product requirements because it shapes both UX and service topology.
7. Initial sandboxing may run without Docker by configuring a root folder and spawning the worker binary on the host OS. Docker remains a later target.
8. Future Docker model: each worker session runs in an image with env/config passed in, clones the target repo, and communicates with daemon over gRPC.
9. Worker package should contain an `agents` folder, with each agent as its own module/folder.
10. Existing tools should move under a `tools` folder in the worker package.
11. The first implemented agent should be the prompt/requirements agent only. This is duplicated in product requirements because it limits user-facing scope and technical blast radius.

## Active listening notes

Final reflection: Doric should shift from interactive coding-agent chat to a daemon-managed lifecycle system where a user submits one prompt/repo job and observes a managed worker. The first deliverable is not the full SDLC agent graph; it is the lifecycle foundation plus the prompt agent.

Ambiguity check: PRD can proceed with assumptions. Missing details such as exact worker status names, command flag syntax, persistence depth, and QUIC availability can be refined in PRD/TDD without changing the feature definition.

Edge-case probe: If a worker needs a human answer while the user is only observing through `doric worker <id>`, the product must define how the daemon surfaces the prompt and how the CLI sends the answer back without bypassing the daemon.

## Resolved decisions

- Initial UI is raw CLI, not TUI.
- Daemon owns worker liveness and routing, not worker internals.
- Worker owns prompt execution, agent/tool loop, repo clone/setup, and internal state.
- CLI communicates through daemon, not directly with workers.
- Start without Docker if needed by using a configured root folder.
- First workflow agent is the prompt/requirements agent only.
- Agents and tools should live under the worker package.

## Open questions

- architecture: Should worker lifecycle state be persisted across daemon restarts in the first slice, or can v1 be in-memory?
- shared: What is the exact human-in-the-loop command flow when a worker pauses for user input?
- architecture: What root folder convention should non-Docker workers use for cloned repos and run artifacts?
- product: Should `feature` and `debug` differ in v1 behavior, or are they separate command labels over the same worker lifecycle?

These do not block PRD generation; they can be carried forward as assumptions and open questions.

## Non-goals

- Full multi-step Doric SDLC execution beyond the prompt agent.
- Docker sandboxing as a hard v1 requirement.
- Permission handling.
- Direct CLI-to-worker communication.
- Rebuilding the TUI for the initial slice.
- Full agent tournament/evaluator implementation in this first lifecycle slice.
