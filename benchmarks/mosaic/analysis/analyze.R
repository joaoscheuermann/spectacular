#!/usr/bin/env Rscript

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) {
  stop("usage: analyze.R <scores.csv> <config.json> <result.json>")
}

script_arg <- grep("^--file=", commandArgs(), value = TRUE)
script_path <- if (length(script_arg) == 1L) sub("^--file=", "", script_arg) else "."
analysis_dir <- dirname(normalizePath(script_path, mustWork = FALSE))
source(file.path(analysis_dir, "utils.R"), local = TRUE)
source(file.path(analysis_dir, "fit.R"), local = TRUE)
source(file.path(analysis_dir, "validate.R"), local = TRUE)
require_analysis_packages()

scores_path <- args[[1]]
config <- read_json(args[[2]])
output_path <- args[[3]]
scores <- utils::read.csv(scores_path, check.names = FALSE)
eligible <- validate_analysis_scores(scores, config)
freeze <- validate_freeze_projection(config)
power_parameters <- validate_power_parameters(config)
baseline <- config$baselineCondition
conditions <- unique(as.character(eligible$conditionId))

power_family <- config$family %in% c("primary", "replication")
power_path <- config_string(config, "powerResultPath")
if (!grepl("^/", power_path)) {
  config_dir <- dirname(normalizePath(args[[2]], mustWork = TRUE))
  power_path <- file.path(config_dir, power_path)
}
validated_power <- validate_power_result_file(power_path, config)
power <- if (power_family) validated_power else NULL

data <- data.frame(
  success = as.integer(eligible$success),
  condition = factor(eligible$conditionId, levels = c(baseline, sort(setdiff(conditions, baseline)))),
  composition_class = factor(eligible$compositionClass, levels = LETTERS[1:6]),
  case_id = factor(eligible$caseId),
  adaptive = as.logical(eligible$adaptive)
)

bootstrap_repetitions <- as.integer(config$bootstrapRepetitions)
bootstrap_seed <- as.integer(config$bootstrapSeed)
default_contrast <- list(
  id = "M1-vs-baseline-primary",
  condition = "M1",
  reference = baseline,
  classes = as.list(power_parameters$adaptiveClasses),
  adaptive = TRUE
)
specifications <- if (is.null(config$contrasts)) list(default_contrast) else config$contrasts
fit <- fit_contrasts_with_fallbacks(
  data,
  specifications,
  default_fitters(bootstrap_repetitions, bootstrap_seed)
)
estimable <- fit$estimator != "not-estimable" && fit$validFitRate >= 0.95
contrasts <- if (estimable) fit$contrasts else list()
if (length(contrasts) > 0L) {
  adjusted <- holm(vapply(contrasts, function(contrast) contrast$pValue, numeric(1)))
  contrasts <- Map(function(contrast, value) {
    contrast$holmPValue <- value
    contrast
  }, contrasts, adjusted)
}

result <- list(
  schemaVersion = 1L,
  studyId = config$studyId,
  family = config$family,
  datasetHash = sha256_file(scores_path),
  freezeHash = freeze$manifestHash,
  implementationHash = validate_hash(config$implementationHash, "implementationHash"),
  formula = formula_text,
  estimator = fit$estimator,
  estimable = estimable,
  validFitRate = fit$validFitRate,
  observations = nrow(data),
  cases = length(unique(data$case_id)),
  contrasts = contrasts,
  power = power,
  warnings = unique(fit$warnings %||% character())
)
result$resultHash <- sha256_value(result)
write_json(result, output_path)
