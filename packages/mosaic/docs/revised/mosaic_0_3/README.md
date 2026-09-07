# MOSAIC 0.3 architecture paper

This directory contains the editable MOSAIC 0.3 architecture source, native
LaTeX/TikZ replacements for versioned or semantically revised figures, retained
source figures, and the rebuilt PDF.

Version 0.3 replaces model-extracted planning hints with:

```text
catalog-independent P0 used upstream for retrieval and gating
-> per-goal high-recall retrieval
-> full-body reranking
-> conservative keep/drop planning gate
-> one P1 synthesis from the request and unchanged retained bodies,
   without exposing P0 to that model call
-> independent final routing per P1 goal
```

When the gate retains no body, the runtime may materialize validated P0 as
revision 1 without making an empty second planning call. Whenever P1 is
model-generated from selected skills, P0 is omitted from its context.

The paper records diagnostic evidence from `scripts/full-skill-vs-hints`,
`scripts/direct-skill-planning`, `scripts/skill-retrieval-gating`, and
`scripts/p0-planning-ablation`. Those results motivate the architecture change
but do not establish end-to-end MOSAIC performance. The current package
implementation still targets MOSAIC 0.2 until the 0.3 planning gate is
implemented.

The latest selection audit is run
`89ba6c1e-470c-43ea-809b-a34a90f59540` under
`scripts/skill-retrieval-gating/output/`. Across 30 exhaustively labeled
cases, hybrid lexical and vector retrieval plus semantic gating recovered all
65 expected skill occurrences, removed 300 of 310 recovered noise
occurrences, and retained 78 of 79 recovered relevant occurrences. The paper
reports these as retrieval and selection evidence only.

The oracle final-synthesis ablation supplies the same ordered
`expected + useful` skill bodies to every condition. Its historical campaign
found no benefit from exposing P0 in four judge/batch cells. A later
confirmation used a neutral system prompt and reused the same frozen direct
control across three P0 fixtures:

| Exposed P0                 | Without P0 | With P0 | Both | Neither | Inconsistent | Decisive rate without P0 | One-sided p |
| -------------------------- | ---------: | ------: | ---: | ------: | -----------: | -----------------------: | ----------: |
| Catalog-independent        |         16 |       9 |    0 |       1 |            4 |                    64.0% |      0.1148 |
| Qwen, identical to control |          3 |       3 |   17 |       4 |            3 |                    50.0% |      0.6563 |
| Gemini                     |          9 |      13 |    2 |       1 |            5 |                    40.9% |      0.8569 |

Qwen generated every fresh P1; “Gemini” identifies the draft source, not the
final planner. None of the three cells supported the predeclared directional
hypothesis that P0 causes harm. The opposite Gemini direction was also not
significant in a post-hoc test. Together, the campaigns show neither universal
harm nor robust benefit: exposure acts as a draft-dependent anchor.

MOSAIC therefore uses the conservative, less coupled contract
`P1 = request + selected skill bodies`. P0 remains provisionally available
upstream for retrieval and gating, but its text is omitted from
skill-conditioned P1 synthesis. This avoids making final planning depend on
another model generation when no stable gain offsets that dependency. The
ablation does not test whether P0 improves retrieval or downstream task
execution. The next validation compares filtered and unfiltered bodies while
omitting P0 from both P1 synthesis prompts; a separate ablation must compare
request-only and P0-guided retrieval.

The P0 treatment jointly changes draft content, structural scaffolding, and
context length. It also uses a simpler list-of-goals contract than the full
normative DAG. The paper records both limits; confirmation on the complete
`Plan` contract remains future work.

Build from this directory with the inspected Island of TeX image pinned by
digest:

```sh
docker run --rm --volume "$PWD:/work" --workdir /work registry.gitlab.com/islandoftex/images/texlive@sha256:8957c916b8160049f89c24d362a6d86c09d8a04095acde37e88404c4afed85b4 sh -lc 'xelatex -interaction=nonstopmode -halt-on-error mosaic_0_3.tex && xelatex -interaction=nonstopmode -halt-on-error mosaic_0_3.tex'
```

The two XeLaTeX passes produce `mosaic_0_3.pdf`. See the
[Island of TeX Docker build guidance](https://gitlab.com/islandoftex/images/texlive/-/wikis/Building-LaTeX-documents-locally-using-Docker)
for the underlying workflow.
