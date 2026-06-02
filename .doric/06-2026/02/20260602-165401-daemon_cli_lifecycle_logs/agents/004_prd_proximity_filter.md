# PRD Proximity Filter

- Receipt id: `prd_proximity_filter_01`
- Role: proximity filter
- Phase goal: compare three PRD seed candidates and identify unique candidates versus structural duplicates.

## Unique candidate list

1. `security_v1`
   - Self-evaluation score: 8/10
   - Generator persona: PM security/compliance advocate
   - Unique structural contribution: extends the shared lifecycle-output scope with explicit privacy, auditability, secret redaction, local data exposure, and terminal-output integrity requirements.
   - Functional differentiators: credential-bearing repository URLs and user-derived lifecycle messages must not expose secrets; line breaks and control characters must not forge extra lifecycle lines; rejected local paths should avoid unnecessary local machine disclosure.
   - User Value Hop differentiator: adds a dedicated credential and terminal-output safety hop.
   - Risk/compliance differentiator: strongest treatment of credential leakage, audit integrity, local data exposure, and over-logging risk.
   - Prompt traceability: strong. It directly traces the approved prompt defaults for lifecycle lines, UUIDv6 IDs, URL-only input, rejected local and `file://` inputs, corrected `cloning repo` spelling, failure/completion coverage, stream-start behavior, and existing listing order, then adds security-specific safeguards that are compatible with prompt constraints.

2. `lean_v1`
   - Self-evaluation score: 9/10
   - Generator persona: PM lean-UX advocate
   - Unique structural contribution: frames the feature around low-friction terminal readability, minimal cognitive load, ordinary command flows, simple recovery from invalid repository input, and no extra user input.
   - Functional differentiators: emphasizes preserving existing command flows, avoiding new CLI commands, keeping output simple, and making empty worker listings understandable without verbose worker detail.
   - User Value Hop differentiator: separates user recovery from invalid repository input and maintainer verification of consistent worker identity as product-value steps.
   - Risk/compliance differentiator: focuses on compatibility, readability, formatting consistency, identity consistency, and clear URL-only error recovery.
   - Prompt traceability: strong. It tracks all resolved product decisions and keeps architecture-specific mechanisms out of the PRD, while preserving the user-approved defaults and non-goals.

3. `scale_v1`
   - Self-evaluation score: 9/10
   - Generator persona: PM scale/performance advocate
   - Unique structural contribution: turns fast-event readability, bounded message volume, low-overhead logging, and predictable line-oriented output into explicit product requirements.
   - Functional differentiators: adds requirements for complete terminal lines under fast event rates, concise bounded lifecycle messages, visible UUIDv6 IDs across serialized and event-output surfaces, and low-noise failure/completion coverage.
   - User Value Hop differentiator: adds fast event readability and low-overhead lifecycle completion as separate hops.
   - Risk/compliance differentiator: strongest NFR treatment for event bursts, terminal noise, low-overhead output, and avoiding terminal-specific rich rendering dependencies.
   - Prompt traceability: strong. It explicitly traces the resolved prompt defaults and active-listening edge case about fast event rendering, while preserving the non-goals against JSON, rich TUI rendering, nested logs, and lifecycle-semantic changes.

## Similarity estimates

| Pair | Similarity estimate | Duplicate decision | Rationale |
| ---- | ------------------- | ------------------ | --------- |
| `security_v1` vs `lean_v1` | 78% | Keep both | They share the mandatory lifecycle, UUIDv6, URL-only validation, listing, stream, persona, and non-goal structure. They diverge materially in risk/compliance treatment, with `security_v1` adding redaction and terminal-output safety while `lean_v1` emphasizes low-friction recovery, simple output, and no added user inputs. |
| `security_v1` vs `scale_v1` | 80% | Keep both | They share broad daemon/CLI lifecycle coverage, URL-only enforcement, UUIDv6 identity, event-stream rendering, and failure/completion coverage. They diverge materially because `security_v1` prioritizes privacy/audit safeguards, while `scale_v1` prioritizes fast-event readability, bounded output, and low-overhead lifecycle behavior. |
| `lean_v1` vs `scale_v1` | 84% | Keep both | This is the closest pair. Both have 9/10 self-evaluation scores and strong prompt traceability around the same core workflows. The overlap remains below the strict more-than-85% discard threshold because `lean_v1` is structurally centered on low-friction user workflows and recovery, while `scale_v1` adds distinct NFR structure for fast event bursts, bounded messages, low-noise completion/failure lines, and predictable stream behavior. |

## Discarded candidates

None.

No candidate is more than 85% structurally similar to a higher-scored candidate. The score tie between `lean_v1` and `scale_v1` does not require a discard because the pair remains under the strict duplicate threshold. If a future evaluator treated that pair as above threshold, the tie-breaker would need to compare prompt traceability closely; in this pass, both are traceable enough and structurally distinct enough to remain unique.

## Tournament recommendation

Tournament is required.

All three seed candidates remain unique after proximity filtering, so the PRD phase should proceed to pairwise Elo tournament evaluation. The tournament should compare:

- `security_v1` for privacy, auditability, URL-only enforcement, and safe terminal rendering.
- `lean_v1` for low-friction CLI UX, concise workflows, recovery clarity, and ordinary terminal readability.
- `scale_v1` for fast-event readability, low-overhead lifecycle output, bounded message volume, and NFR feasibility.
