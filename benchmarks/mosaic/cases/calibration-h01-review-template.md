# H-01 calibration corpus review record

> **Status: APPROVED — REVIEWED — H-01 ACCEPTED**
>
> João Vitor Scheuermann attested that the complete human semantic review
> recorded below was performed before paid calibration results were observed.

## Bound artifacts

- Draft: `calibration-draft.v1.json`
  - Canonical artifact hash:
    `sha256:1dc33a0c5541d03a8417ecc40e278d043bf98877e1a1f4647c85d4aefac22908`
  - Byte SHA-256:
    `b7201f89b88f15e72e74d9f8895e5d408bde8f614313527513049a664dbdb487`
- Compiled corpus: `calibration-cases.v1.json`
  - Canonical artifact hash:
    `sha256:937d781ee6512adb3a7bd2af58550e672e29719d306aa476b4761ed9efb986c6`
  - Byte SHA-256:
    `40b2cf465cb594b2247073ef393203fe2d1510cc7bc44a01a65690a1c7f774b9`

Any byte or canonical-hash change invalidates this template and requires a new
automatic report followed by a fresh human review.

## Automatic checks completed

These checks establish structural and deterministic validity only.

- [x] Provider-free compilation completed with zero issues.
- [x] Standalone calibration validation completed with zero issues.
- [x] Exactly 60 unique case IDs and 60 unique family IDs.
- [x] All cases use `phase: "calibration"`.
- [x] Exactly 15 cases per domain.
- [x] Exactly 10 cases per composition class A–F.
- [x] Every tool execution, evidence hash, final World hash, required effect,
      canonical delivery, and content hash is reproducible.
- [x] Exact pilot request overlap: 0.
- [x] Exact pilot title overlap: 0.
- [x] Completion/success marker findings: 0.
- [x] All 24 deterministic benchmark tools appear in at least one required
      tool sequence.
- [x] 35 distinct catalog skills appear as required procedural references.
- [x] 15 cases are tagged adaptive by the authored design.
- [x] Five cases require deterministic isolated-world effects.

The automatic checks do **not** establish semantic neutrality, appropriate
difficulty, procedural equivalence, absence of semantic near-duplicates,
correct distractor/conflict labeling, or freedom from answer leakage.

## Corpus-level human review

All checks below were attested by the identified human reviewer.

- [x] Every request and expected answer is semantically correct.
- [x] Every answer can be derived from visible task facts or deterministic
      observations; no arbitrary password, marker, or unexplained value is
      required.
- [x] The canonical JSON reporting contract does not favor B0–B3 or M1.
- [x] Required skills are procedurally relevant but are not success gates.
- [x] Relevant skills, overlaps, equivalents, distractors, and conflicts have
      been reviewed against the current catalog.
- [x] Tool choices and ordered inputs measure the stated task rather than an
      architecture-specific implementation detail.
- [x] Difficulty is comparable across the intended model families.
- [x] Class A–F assignments accurately reflect skill/tool composition.
- [x] Domain assignments accurately reflect task substance.
- [x] No case is a semantic near-duplicate of a pilot case.
- [x] No pair within this calibration corpus is a semantic duplicate.
- [x] Wording does not reveal hidden gold, tool hashes, World hashes, or
      content hashes.
- [x] The 15 adaptive labels are scientifically justified and do not silently
      claim required localized revision.
- [x] Mutating cases use the intended artifact/message/file destination,
      content, value, and final state.
- [x] English wording is clear, grammatical, and unambiguous.

## Per-case human review

For each case, reviewers must inspect the request, expected answer, required
skills, tool sequence, canonical document, and criterion evidence bindings.

| Case                   | Correct semantics | No answer leak | Architecture-neutral | No semantic duplicate | Status   |
| ---------------------- | ----------------- | -------------- | -------------------- | --------------------- | -------- |
| `calibration.case.a01` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a02` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a03` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a04` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a05` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a06` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a07` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a08` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a09` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.a10` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b01` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b02` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b03` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b04` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b05` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b06` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b07` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b08` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b09` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.b10` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c01` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c02` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c03` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c04` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c05` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c06` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c07` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c08` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c09` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.c10` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d01` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d02` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d03` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d04` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d05` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d06` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d07` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d08` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d09` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.d10` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e01` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e02` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e03` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e04` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e05` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e06` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e07` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e08` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e09` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.e10` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f01` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f02` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f03` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f04` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f05` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f06` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f07` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f08` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f09` | ✓                 | ✓              | ✓                    | ✓                     | approved |
| `calibration.case.f10` | ✓                 | ✓              | ✓                    | ✓                     | approved |

## Review decision

- Reviewer: **João Vitor Scheuermann <joao.s@wonderful.ai>**
- Review date: **2026-08-09T15:52:27.000Z**
- Reviewed artifact hashes match the bound hashes: ✓
- Required corrections: **none**
- Reviewer attestation: **The complete corpus-level and per-case review was
  performed before observing paid calibration results.**
- Decision: **APPROVED**
- Approved for H-01: **true**

This approval is valid only for the exact canonical and byte hashes bound
above. Any artifact change invalidates H-01 and requires a fresh human review.
