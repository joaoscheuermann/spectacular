# MOSAIC Core Profile 0.1 — LaTeX reconstruction

Main file: `mosaic_core_latex.tex`

Figures and tables from the supplied PDF are stored in `figures/` and referenced with `\includegraphics`. Compile with XeLaTeX:

```sh
xelatex mosaic_core_latex.tex
```

The prose is editable LaTeX; complex diagrams/tables are preserved as raster crops from the supplied PDF.
