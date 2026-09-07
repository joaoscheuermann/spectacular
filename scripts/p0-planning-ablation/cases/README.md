# Case Data

This directory is a frozen copy of the 30 authored cases and 37-skill catalog
used by the sibling `skill-retrieval-gating` diagnostic. Keeping a physical
copy lets each run bind its manifest to the exact inputs used by this ablation.
The eight focused skills created with the lab are stored beside the vendored
catalog in `skills/`.

Each case classifies the complete local catalog:

- `skills.expected` contains required useful skills whose absence lowers
  recall;
- `skills.useful` contains helpful supplementary skills whose absence is not a
  failure and whose selection counts toward precision;
- every remaining skill appears in `skills.noise` with one type:
  - `irrelevant`: no application to the objective;
  - `no_operational_value`: related or generic, but adds no material procedure,
    constraint, verification, tool, or data access;
  - `conflicting`: normal application violates an explicit objective
    constraint.

This experiment uses the ordered union `skills.expected + skills.useful` as its
oracle gold bundle. Only the resolved skill bodies are passed to planners and
the judge; the category labels themselves never enter a model prompt.
`skills.noise` is retained for corpus provenance but is not read by the runtime
after case validation and has no metric or treatment role.

## Case provenance

Twenty-four cases were added on 2026-08-30 from distinct active tasks in
SkillsBench v1.1 at commit
`b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af`. Each objective is a compact,
constraint-preserving extraction of the corresponding `task.md`; the cases are
not paraphrased variants of one another. The upstream categories of the new
cases are office/white-collar (11), finance/economics (8), software engineering
(3), and natural science (2).

Across all 30 cases, the normalized operational-domain distribution is
office/document work (12), finance/spreadsheets (8), software/release
engineering (6), natural science (2), and travel planning (2). The closest
pairs remain independent: seismic picking emits phase indices while phase
association groups multi-station picks into events; the two Georgia shock cases
use different demand-accounting and supply-side Cobb-Douglas/HP-filter models.

| Case                           | Upstream category    | SkillsBench source                                                                                                                                                                   |
| ------------------------------ | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `court-form-filling`           | office/white-collar  | [`tasks/court-form-filling/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/court-form-filling/task.md)                     |
| `data-to-d3`                   | software engineering | [`tasks/data-to-d3/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/data-to-d3/task.md)                                     |
| `earthquake-phase-association` | natural science      | [`tasks/earthquake-phase-association/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/earthquake-phase-association/task.md) |
| `edit-pdf`                     | office/white-collar  | [`tasks/edit-pdf/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/edit-pdf/task.md)                                         |
| `exceltable-in-ppt`            | office/white-collar  | [`tasks/exceltable-in-ppt/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/exceltable-in-ppt/task.md)                       |
| `financial-modeling-qa`        | finance/economics    | [`tasks/financial-modeling-qa/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/financial-modeling-qa/task.md)               |
| `flink-query`                  | software engineering | [`tasks/flink-query/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/flink-query/task.md)                                   |
| `invoice-fraud-detection`      | finance/economics    | [`tasks/invoice-fraud-detection/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/invoice-fraud-detection/task.md)           |
| `jax-computing-basics`         | software engineering | [`tasks/jax-computing-basics/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/jax-computing-basics/task.md)                 |
| `latex-formula-extraction`     | office/white-collar  | [`tasks/latex-formula-extraction/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/latex-formula-extraction/task.md)         |
| `offer-letter-generator`       | office/white-collar  | [`tasks/offer-letter-generator/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/offer-letter-generator/task.md)             |
| `organize-messy-files`         | office/white-collar  | [`tasks/organize-messy-files/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/organize-messy-files/task.md)                 |
| `paper-anonymizer`             | office/white-collar  | [`tasks/paper-anonymizer/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/paper-anonymizer/task.md)                         |
| `pdf-excel-diff`               | office/white-collar  | [`tasks/pdf-excel-diff/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/pdf-excel-diff/task.md)                             |
| `powerlifting-coef-calc`       | office/white-collar  | [`tasks/powerlifting-coef-calc/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/powerlifting-coef-calc/task.md)             |
| `pptx-reference-formatting`    | office/white-collar  | [`tasks/pptx-reference-formatting/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/pptx-reference-formatting/task.md)       |
| `reserves-at-risk-calc`        | finance/economics    | [`tasks/reserves-at-risk-calc/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/reserves-at-risk-calc/task.md)               |
| `sales-pivot-analysis`         | office/white-collar  | [`tasks/sales-pivot-analysis/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/sales-pivot-analysis/task.md)                 |
| `sec-financial-report`         | finance/economics    | [`tasks/sec-financial-report/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/sec-financial-report/task.md)                 |
| `seismic-phase-picking`        | natural science      | [`tasks/seismic-phase-picking/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/seismic-phase-picking/task.md)               |
| `shock-analysis-demand`        | finance/economics    | [`tasks/shock-analysis-demand/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/shock-analysis-demand/task.md)               |
| `shock-analysis-supply`        | finance/economics    | [`tasks/shock-analysis-supply/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/shock-analysis-supply/task.md)               |
| `weighted-gdp-calc`            | finance/economics    | [`tasks/weighted-gdp-calc/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/weighted-gdp-calc/task.md)                       |
| `xlsx-recover-data`            | finance/economics    | [`tasks/xlsx-recover-data/task.md`](https://github.com/benchflow-ai/skillsbench/blob/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks/xlsx-recover-data/task.md)                       |

The other 29 skill files were materialized on 2026-08-30 from SkillsBench v1.1
commit `b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af`. Their source files were under
`tasks/<task>/environment/skills/<skill>/SKILL.md` in
`benchflow-ai/skillsbench`. Frontmatter names and descriptions were converted
to the local Markdown heading and introductory paragraph; the skill bodies
were preserved. Runtime execution reads only these local files and performs no
network download.
