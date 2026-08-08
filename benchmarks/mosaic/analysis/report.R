#!/usr/bin/env Rscript

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) stop("usage: report.R <result.json> <scores.csv> <report.md>")

script_arg <- grep("^--file=", commandArgs(), value = TRUE)
script_path <- if (length(script_arg) == 1L) sub("^--file=", "", script_arg) else "."
analysis_dir <- dirname(normalizePath(script_path, mustWork = FALSE))
source(file.path(analysis_dir, "utils.R"), local = TRUE)
if (!requireNamespace("jsonlite", quietly = TRUE)) stop("jsonlite is required")

result <- read_json(args[[1]])
scores <- utils::read.csv(args[[2]], check.names = FALSE)
if (!all(c("conditionId", "costUsd") %in% names(scores))) {
  stop("score dataset is missing cost-reporting columns")
}
costs <- stats::aggregate(
  scores$costUsd,
  list(condition = scores$conditionId),
  function(values) c(mean = mean(values), total = sum(values), runs = length(values))
)
cost_lines <- vapply(seq_len(nrow(costs)), function(index) {
  values <- costs$x[index, ]
  sprintf(
    "- %s: mean USD %.6f; total USD %.6f across %d runs (failures included).",
    costs$condition[[index]], values[["mean"]], values[["total"]], values[["runs"]]
  )
}, character(1))
contrast_lines <- if (length(result$contrasts) == 0L) {
  "No contrast was estimable."
} else {
  vapply(result$contrasts, function(contrast) {
    sprintf(
      "- %s: OR %.4f (95%% CI %.4f to %.4f); absolute difference %.4f (95%% CI %.4f to %.4f); Holm p %.4g.",
      contrast$id,
      contrast$oddsRatio,
      contrast$oddsRatioConfidence95$lower,
      contrast$oddsRatioConfidence95$upper,
      contrast$absoluteDifference,
      contrast$absoluteDifferenceConfidence95$lower,
      contrast$absoluteDifferenceConfidence95$upper,
      contrast$holmPValue
    )
  }, character(1))
}
power_lines <- if (is.null(result$power)) {
  "No power result is attached to this analysis family."
} else {
  c(
    sprintf("- Simulations: %d", result$power$simulations),
    sprintf(
      "- Seed: `%s` (numeric seed %d)",
      result$power$seed,
      result$power$numericSeed
    ),
    sprintf("- Config hash: `%s`", result$power$configHash),
    sprintf("- Baseline probability: %.6f", result$power$baselineProbability),
    sprintf("- Random-intercept SD: %.6f", result$power$randomInterceptSd),
    sprintf(
      "- Adaptive classes: %s",
      paste(unlist(result$power$adaptiveClasses), collapse = ", ")
    ),
    sprintf("- Minimum effect: %.6f", result$power$minimumEffect),
    sprintf(
      "- N_power/N_final: %d/%d at estimated power %.6f",
      result$power$nPower,
      result$power$nFinal,
      result$power$power
    )
  )
}

lines <- c(
  "# MOSAIC benchmark analysis",
  "",
  sprintf("- Study: `%s`", result$studyId),
  sprintf("- Family: `%s`", result$family),
  sprintf("- Estimator: `%s`", result$estimator),
  sprintf("- Estimable: `%s`", tolower(as.character(result$estimable))),
  sprintf("- Valid fit rate: `%.4f`", result$validFitRate),
  sprintf("- Observations/cases: `%d/%d`", result$observations, result$cases),
  sprintf("- Dataset hash: `%s`", result$datasetHash),
  sprintf("- Result hash: `%s`", result$resultHash),
  "",
  "## Power provenance",
  "",
  power_lines,
  "",
  "## Contrasts",
  "",
  contrast_lines,
  "",
  "## Cost (descriptive)",
  "",
  cost_lines,
  "",
  "Costs are descriptive and are not an eligibility or success gate."
)
writeLines(enc2utf8(lines), args[[3]], useBytes = TRUE)
