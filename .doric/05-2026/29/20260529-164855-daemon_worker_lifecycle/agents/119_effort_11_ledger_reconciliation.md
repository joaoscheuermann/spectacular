# Coordinator Receipt: effort 11 ledger reconciliation

## Role

Coordinator reconciliation after the first effort 11 reviewer found a stale historical ledger status from effort 07.

## Evidence

- `agents/081_effort_07_reviewer.md` rejected the first effort 07 review.
- `agents/085_effort_07_reviewer_retry.md` later accepted effort 07 after the required repair and validation.
- `STATE.md` still listed `agents/081_effort_07_reviewer.md` as `rejected` in both the Required agents and Agent receipts tables.
- `agents/118_effort_11_reviewer.md` rejected effort 11 only because that stale historical rejected row was still active in the ledger.

## Changes

- Marked `agents/081_effort_07_reviewer.md` as `superseded` in `STATE.md`.
- Added a supersession note to `agents/081_effort_07_reviewer.md` linking the accepted retry receipt.
- Marked `agents/118_effort_11_reviewer.md` as `rejected` in `STATE.md`.
- Registered `agents/120_effort_11_reviewer_retry.md` as the replacement effort 11 reviewer gate.

## Coordinator decision

accepted
