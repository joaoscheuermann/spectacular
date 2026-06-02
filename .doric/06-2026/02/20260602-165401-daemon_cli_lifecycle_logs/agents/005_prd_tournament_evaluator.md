# PRD Tournament Evaluator

- Receipt id: `prd_tournament_evaluator_01`
- Role: tournament evaluator
- Phase goal: run pairwise Elo tournament comparisons across unique PRD candidates.

## Inputs

- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/PROMPT.md`
- `.agents/skills/doric/references/02-prd-generation.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/001_prd_pm_security_candidate.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/002_prd_pm_lean_candidate.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/003_prd_pm_scale_candidate.md`
- `.doric/06-2026/02/20260602-165401-daemon_cli_lifecycle_logs/agents/004_prd_proximity_filter.md`

## Method

All three seed candidates remain unique after proximity filtering, so the tournament evaluated every pair:

- `security_v1` vs `lean_v1`
- `security_v1` vs `scale_v1`
- `lean_v1` vs `scale_v1`

Initial Elo was `1000` for each candidate. K-factor was `32`.

The combined tournament score is the sum of:

- Security/compliance score in `[0, 10]`.
- UX score in `[0, 10]`, where higher means lower product friction.
- Complexity score in `[0, 10]`, computed as `10 - product_complexity`, where lower product complexity is better.
- NFR compliance score in `[0, 10]`.
- Rule Adherence Score using the Step 02 weighted rules:
  - Rule 1: Direct Coverage, weight `1.0`
  - Rule 2: Scope Containment, weight `1.0`
  - Rule 3: Constraint Contradiction, weight `1.5`
  - Rule 4: NFR Feasibility, weight `1.0`
  - Rule 5: Implicit Dependency, weight `0.8`

No pair required the product-complexity tie-breaker because every combined score was distinct.

## Candidate Scores

| Candidate     | Security / compliance | UX  | Product complexity | Complexity score | NFR | Rule adherence | Combined |
| ------------- | --------------------- | --- | ------------------ | ---------------- | --- | -------------- | -------- |
| `scale_v1`    | 8.0                   | 8.6 | 4.6                | 5.4              | 9.7 | 5.190          | 38.890   |
| `security_v1` | 9.8                   | 8.2 | 4.8                | 5.2              | 8.8 | 5.050          | 37.050   |
| `lean_v1`     | 7.2                   | 9.5 | 3.2                | 6.8              | 8.0 | 5.124          | 36.624   |

## Rule Adherence Detail

| Candidate     | Rule 1 Direct Coverage | Rule 2 Scope Containment | Rule 3 Constraint Contradiction | Rule 4 NFR Feasibility | Rule 5 Implicit Dependency | Weighted score |
| ------------- | ---------------------- | ------------------------ | ------------------------------- | ---------------------- | -------------------------- | -------------- |
| `security_v1` | 0.98                   | 0.92                     | 1.00                            | 0.93                   | 0.90                       | 5.050          |
| `lean_v1`     | 1.00                   | 0.98                     | 1.00                            | 0.90                   | 0.93                       | 5.124          |
| `scale_v1`    | 0.99                   | 0.96                     | 1.00                            | 0.98                   | 0.95                       | 5.190          |

## Pairwise Outcomes

### `security_v1` defeats `lean_v1`

- Winner: `security_v1`
- Combined scores: `security_v1` `37.050`, `lean_v1` `36.624`
- Elo update: `security_v1` `1000.000 -> 1016.000`; `lean_v1` `1000.000 -> 984.000`
- Rationale: `lean_v1` has the lowest product friction and simplest product surface, but it does not explicitly cover secret redaction or terminal-output integrity even though the prompt requires daemon terminal logs for user-message activity and repository lifecycle lines. `security_v1` adds bounded privacy and output-forgery safeguards without contradicting the prompt, so it wins the security/compliance dimension strongly enough to overcome higher product complexity.

### `scale_v1` defeats `security_v1`

- Winner: `scale_v1`
- Combined scores: `scale_v1` `38.890`, `security_v1` `37.050`
- Elo update: `security_v1` `1016.000 -> 999.264`; `scale_v1` `1000.000 -> 1016.736`
- Rationale: `security_v1` is the strongest privacy and audit candidate, but `scale_v1` better covers fast-event readability, bounded message volume, low-overhead lifecycle output, failure/completion visibility, and ordered line-oriented stream behavior. Those NFR strengths are central to the prompt's active-listening edge case and terminal readability goal.

### `scale_v1` defeats `lean_v1`

- Winner: `scale_v1`
- Combined scores: `scale_v1` `38.890`, `lean_v1` `36.624`
- Elo update: `lean_v1` `984.000 -> 969.503`; `scale_v1` `1016.736 -> 1031.233`
- Rationale: `lean_v1` is the best low-friction workflow draft, but `scale_v1` preserves the simple terminal experience while making fast event bursts, bounded output, stream startup, failure/completion lines, and low-noise lifecycle behavior explicit product requirements. Its stronger NFR feasibility and nearly equal rule adherence outweigh the extra complexity.

## Final Rankings

| Rank | Candidate     | Final Elo | Match record | Summary                                                                                                                                                                                    |
| ---- | ------------- | --------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1    | `scale_v1`    | 1031.233  | 2-0          | Best tournament fit for fast-event readability, low-overhead terminal lifecycle output, bounded messages, URL-only validation, UUIDv6 identity, and lifecycle completion/failure coverage. |
| 2    | `security_v1` | 999.264   | 1-1          | Best source for privacy, redaction, audit integrity, and safe one-line rendering concepts that should be grafted into the champion.                                                        |
| 3    | `lean_v1`     | 969.503   | 0-2          | Best source for low-friction wording and clear recovery, but it under-specifies terminal safety and high-rate lifecycle behavior relative to the other candidates.                         |

## Top Two

The top two candidate ids for concept grafting are:

1. `scale_v1`
2. `security_v1`

## Blockers

None. All tournament inputs were present, all three candidates were unique, every pair was judged, and the Elo ledger was written to `logs/prd_tournament_evals.json`.
