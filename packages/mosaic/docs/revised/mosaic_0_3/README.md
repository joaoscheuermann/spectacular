# MOSAIC 0.3 architecture paper

This directory contains the editable MOSAIC 0.3 architecture source, native
LaTeX/TikZ replacements for versioned or semantically revised figures, retained
source figures, and the rebuilt PDF.

Version 0.3 replaces model-extracted planning hints with:

```text
catalog-independent P0
-> per-goal high-recall retrieval
-> full-body reranking
-> conservative keep/drop planning gate
-> one P1 revision with unchanged retained bodies
-> independent final routing per P1 goal
```

The paper records diagnostic evidence from `scripts/full-skill-vs-hints`,
`scripts/direct-skill-planning`, and `scripts/direct-skill-planning-gated`.
Those results motivate the architecture change but do not establish end-to-end
MOSAIC performance. The current package implementation still targets MOSAIC
0.2 until the 0.3 planning gate is implemented.

Build from this directory with the inspected Island of TeX image pinned by
digest:

```sh
docker run --rm --volume "$PWD:/work" --workdir /work registry.gitlab.com/islandoftex/images/texlive@sha256:8957c916b8160049f89c24d362a6d86c09d8a04095acde37e88404c4afed85b4 sh -lc 'xelatex -interaction=nonstopmode -halt-on-error mosaic_0_3.tex && xelatex -interaction=nonstopmode -halt-on-error mosaic_0_3.tex'
```

The two XeLaTeX passes produce `mosaic_0_3.pdf`. See the
[Island of TeX Docker build guidance](https://gitlab.com/islandoftex/images/texlive/-/wikis/Building-LaTeX-documents-locally-using-Docker)
for the underlying workflow.
