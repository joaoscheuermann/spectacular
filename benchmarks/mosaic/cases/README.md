# Case families

`src/study/cases.ts` materializes the frozen pilot instrument: 60 independent
families, 10 for each composition class A–F and 15 for each domain. Every case
is parsed by `CaseV1` and includes a canonical content hash.

Delivery success is structural. `gold.expectedDelivery.document` is the exact
JSON document requested from every condition; key order and Markdown `json`
fencing do not matter. Each entry in `expectedDelivery.fields` gives a stable
evidence ID and path into that document. Every `gold.criteria` entry names one
or more delivery-field or tool-evidence IDs in `evidenceRefs`. Substrings,
completion markers, selected skills, bundle shape, and revision behavior do not
substitute for substantive task evidence.

## Versioned calibration corpus

- `calibration-draft.v1.json` is the independently authored semantic source.
- `calibration-cases.v1.json` is the canonical CaseV1 array produced by the
  provider-free compiler.
- `calibration-h01-review-template.md` records the completed human semantic
  review and binds both artifacts to exact hashes.
- `calibration-audit.v1.json` is the strict machine-readable H-01 approval
  consumed by study readiness.

The corpus contains 60 English cases with unique calibration IDs and families:
15 for each domain and 10 for each composition class A–F. Automatic standalone
validation is clean. Automatic validation alone establishes schema, balance,
evidence, deterministic execution, and hash validity only. H-01 is satisfied
separately by the human review record and its hash-bound approval artifact.

To reproduce the checked-in artifact, compile to a new path because the writer
intentionally refuses to overwrite existing files, then compare the JSON:

```sh
node benchmarks/mosaic/scripts/cases.mjs compile \
  --draft benchmarks/mosaic/cases/calibration-draft.v1.json \
  --output /tmp/mosaic-calibration-cases.v1.json
node benchmarks/mosaic/scripts/cases.mjs validate \
  --calibration-cases benchmarks/mosaic/cases/calibration-cases.v1.json
```

## Calibration case compiler

Author calibration cases in one strict draft document:

```json
{
  "schemaVersion": 1,
  "cases": [
    {
      "schemaVersion": 1,
      "id": "calibration.case.001",
      "familyId": "calibration.family.001",
      "phase": "calibration",
      "title": "Reconcile one invoice",
      "domain": "documents-finance",
      "compositionClass": "B",
      "focusGoalRole": "goal.primary",
      "adaptive": false,
      "request": "Read the fixture and return exactly one JSON object with fixtureId, answer, and observations.",
      "fixtureIds": ["world-v1"],
      "tags": ["calibration", "domain.documents-finance", "class.B"],
      "criteria": [
        {
          "id": "task-evidence",
          "description": "The fixture was read and reported correctly.",
          "evidenceRefs": ["tool.read", "delivery.answer"]
        }
      ],
      "requiredSkills": [],
      "relevantSkills": [],
      "forbiddenSkills": [],
      "tools": [
        {
          "id": "tool.read",
          "name": "read",
          "input": { "path": "notes/request.md" }
        }
      ],
      "expectedAnswer": { "source": "notes/request.md" },
      "requiresRevision": false
    }
  ]
}
```

The compiler derives the composition signature, required and forbidden tools,
tool evidence hashes, final World hash, required effects, the canonical
delivery envelope, and `contentHash`. It executes only deterministic benchmark
tools over a fresh `World`; it makes no model or network calls.

After building `mosaic-benchmark`, compile and validate with:

```sh
node benchmarks/mosaic/scripts/cases.mjs compile \
  --draft /path/to/calibration-draft.json \
  --output /path/to/calibration-cases.json

node benchmarks/mosaic/scripts/cases.mjs validate \
  --calibration-cases /path/to/calibration-cases.json
```

Both commands return every detected issue as JSON. Compilation opens the
output exclusively and refuses to overwrite an existing file. Pass
`--confirmatory-cases` when that corpus is available to add cross-phase
isolation checks.

Confirmatory and calibration families must use new family identifiers and
independently authored content. `validateFamilyIsolation` rejects reused
families and exact non-identity content clones even when IDs or tags change;
the compiler explicitly leaves semantic neutrality, difficulty, procedural
equivalence, and semantic cross-phase independence to the required human
pre-outcome audit.
