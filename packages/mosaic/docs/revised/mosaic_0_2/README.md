# MOSAIC 0.2 architecture paper

This directory contains the editable MOSAIC 0.2 architecture source, native
LaTeX/TikZ replacements for versioned or semantically revised figures, retained
source figures, and the rebuilt PDF.

Build from this directory with the inspected Island of TeX image pinned by
digest:

```powershell
docker run --rm --volume "$($PWD.Path):/work" --workdir /work registry.gitlab.com/islandoftex/images/texlive@sha256:8957c916b8160049f89c24d362a6d86c09d8a04095acde37e88404c4afed85b4 sh -lc "xelatex -interaction=nonstopmode -halt-on-error mosaic_0_2.tex && xelatex -interaction=nonstopmode -halt-on-error mosaic_0_2.tex"
```

The two XeLaTeX passes produce `mosaic_0_2.pdf`. See the
[Island of TeX Docker build guidance](https://gitlab.com/islandoftex/images/texlive/-/wikis/Building-LaTeX-documents-locally-using-Docker)
for the underlying workflow.
