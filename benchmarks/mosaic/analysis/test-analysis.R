#!/usr/bin/env Rscript

script_arg <- grep("^--file=", commandArgs(), value = TRUE)
script_path <- if (length(script_arg) == 1L) sub("^--file=", "", script_arg) else "."
analysis_dir <- dirname(normalizePath(script_path, mustWork = FALSE))
source(file.path(analysis_dir, "utils.R"), local = TRUE)
source(file.path(analysis_dir, "fit.R"), local = TRUE)
source(file.path(analysis_dir, "validate.R"), local = TRUE)

holm_golden <- read_json(file.path(analysis_dir, "goldens", "holm.json"))
power_golden <- read_json(file.path(analysis_dir, "goldens", "power.json"))
fallback_golden <- read_json(file.path(analysis_dir, "goldens", "fallbacks.json"))

fake_success <- function(estimator) list(
  estimator = estimator,
  validFitRate = 1,
  warnings = character()
)
unused_data <- data.frame(success = c(0L, 1L))

nlopt <- fit_with_fallbacks(unused_data, list(
  bobyqa = function(data) NULL,
  nloptwrap = function(data) fake_success("glmer-nloptwrap"),
  hc2 = function(data) stop("unexpected HC2 call"),
  bootstrap = function(data) stop("unexpected bootstrap call")
))
stopifnot(identical(nlopt$estimator, "glmer-nloptwrap"))

hc2 <- fit_with_fallbacks(unused_data, list(
  bobyqa = function(data) NULL,
  nloptwrap = function(data) NULL,
  hc2 = function(data) fake_success("glm-hc2-cluster"),
  bootstrap = function(data) stop("unexpected bootstrap call")
))
stopifnot(identical(hc2$estimator, "glm-hc2-cluster"))

bootstrap <- fit_with_fallbacks(unused_data, list(
  bobyqa = function(data) NULL,
  nloptwrap = function(data) NULL,
  hc2 = function(data) NULL,
  bootstrap = function(data) list(
    estimator = "not-estimable",
    validFitRate = 0.949,
    warnings = "bootstrap_fit_rate_below_95pct"
  )
))
stopifnot(identical(bootstrap$estimator, "not-estimable"))
stopifnot(bootstrap$validFitRate < 0.95)

stopifnot(isTRUE(all.equal(
  holm(unlist(holm_golden$input)),
  unlist(holm_golden$expected)
)))
stopifnot(round_up(max(240L, power_golden$nPower), 120L) == power_golden$nFinal)
stopifnot(fallback_golden$minimumValidFitRate == 0.95)

validation_cells <- expand.grid(
  compositionClass = LETTERS[1:6],
  domain = c("documents-finance", "software", "artifacts", "communication"),
  stringsAsFactors = FALSE
)
validation_cases <- validation_cells[
  rep(seq_len(nrow(validation_cells)), each = 10L),
  ,
  drop = FALSE
]
validation_rows <- do.call(rbind, lapply(seq_len(nrow(validation_cases)), function(index) {
  item <- validation_cases[index, ]
  do.call(rbind, lapply(c("B2", "M1"), function(condition) {
    repetitions <- seq_len(5L)
    data.frame(
      schemaVersion = 1L,
      runId = sprintf("run-%d-%s-%d", index, condition, repetitions),
      attempt = 1L,
      studyId = "synthetic-study",
      phase = "confirmatory",
      caseId = sprintf("case-%d", index),
      familyId = sprintf("family-%d", index),
      conditionId = condition,
      repetition = repetitions,
      pairedBlock = sprintf("block-%d-%d", index, repetitions),
      provider = "openai",
      model = "openai/gpt-5.6-luna",
      effort = "medium",
      freezeHash = paste0("sha256:", strrep("a", 64L)),
      traceRootHash = paste0("sha256:", strrep("b", 64L)),
      traceDerivedHash = paste0("sha256:", strrep("c", 64L)),
      evidenceHash = paste0("sha256:", strrep("d", 64L)),
      worldHash = paste0("sha256:", strrep("e", 64L)),
      modelCallBudget = NA_integer_,
      domain = item$domain,
      compositionClass = item$compositionClass,
      adaptive = item$compositionClass %in% c("E", "F"),
      success = as.integer(repetitions %% 2L == 0L),
      primaryEligible = TRUE,
      infrastructure = FALSE,
      stringsAsFactors = FALSE
    )
  }))
}))
validation_config <- list(
  studyId = "synthetic-study",
  family = "primary",
  baselineCondition = "B2",
  provider = "openai",
  model = "openai/gpt-5.6-luna",
  effort = "medium",
  powerResultPath = "registered-power.json",
  bootstrapRepetitions = 10000L,
  bootstrapSeed = 104729L,
  implementationHash = paste0("sha256:", strrep("f", 64L)),
  powerParameters = list(
    seed = "L'Ecuyer-CMRG:104729",
    numericSeed = 104729L,
    baselineProbability = 0.45,
    randomInterceptSd = 0.7,
    adaptiveClasses = as.list(c("E", "F"))
  ),
  freeze = list(
    manifestHash = paste0("sha256:", strrep("a", 64L)),
    nPower = 240L,
    nFinal = 240L,
    repetitions = 5L,
    minimumEffect = 0.1,
    modelCallBudgetP95 = 7L,
    artifactHashes = list(
      analysis = paste0("sha256:", strrep("f", 64L)),
      powerConfig = paste0("sha256:", strrep("9", 64L)),
      powerResult = paste0("sha256:", strrep("8", 64L))
    )
  )
)
stopifnot(nrow(validate_analysis_scores(validation_rows, validation_config)) == 2400L)

wrong_implementation <- validation_config
wrong_implementation$implementationHash <- paste0("sha256:", strrep("0", 64L))
stopifnot(inherits(try(
  validate_analysis_scores(validation_rows, wrong_implementation),
  silent = TRUE
), "try-error"))

wrong_bootstrap_seed <- validation_config
wrong_bootstrap_seed$bootstrapSeed <- 104730L
stopifnot(inherits(try(
  validate_analysis_scores(validation_rows, wrong_bootstrap_seed),
  silent = TRUE
), "try-error"))

wrong_bootstrap_repetitions <- validation_config
wrong_bootstrap_repetitions$bootstrapRepetitions <- 9999L
stopifnot(inherits(try(
  validate_analysis_scores(validation_rows, wrong_bootstrap_repetitions),
  silent = TRUE
), "try-error"))

missing_power_path <- validation_config
missing_power_path$powerResultPath <- NULL
stopifnot(inherits(try(
  validate_analysis_scores(validation_rows, missing_power_path),
  silent = TRUE
), "try-error"))

custom_primary <- validation_config
custom_primary$contrasts <- list(list(id = "post-hoc"))
stopifnot(inherits(try(
  validate_analysis_scores(validation_rows, custom_primary),
  silent = TRUE
), "try-error"))

custom_secondary <- validation_config
custom_secondary$family <- "secondary"
custom_secondary$contrasts <- list(list(id = "registered-secondary"))
stopifnot(nrow(validate_analysis_scores(
  validation_rows,
  custom_secondary
)) == 2400L)

duplicate_run <- validation_rows
duplicate_run$runId[[2]] <- duplicate_run$runId[[1]]
stopifnot(inherits(try(
  validate_analysis_scores(duplicate_run, validation_config),
  silent = TRUE
), "try-error"))

broken_pair <- validation_rows
broken_pair$pairedBlock[[2]] <- "wrong-block"
stopifnot(inherits(try(
  validate_analysis_scores(broken_pair, validation_config),
  silent = TRUE
), "try-error"))

mixed_model <- validation_rows
mixed_model$model[[1]] <- "openai/gpt-5.6-sol"
stopifnot(inherits(try(
  validate_analysis_scores(mixed_model, validation_config),
  silent = TRUE
), "try-error"))

sensitivity <- validation_rows
sensitivity$modelCallBudget <- 7L
sensitivity_config <- validation_config
sensitivity_config$family <- "sensitivity"
stopifnot(nrow(validate_analysis_scores(sensitivity, sensitivity_config)) == 2400L)

golden_power_config <- validation_config
golden_power_config$freeze$nPower <- power_golden$nPower
golden_power_config$freeze$nFinal <- power_golden$nFinal
golden_power_config$freeze$artifactHashes$powerConfig <- power_golden$configHash
golden_power_config$freeze$artifactHashes$powerResult <- sha256_file(
  file.path(analysis_dir, "goldens", "power.json")
)
stopifnot(identical(
  validate_power_result_file(
    file.path(analysis_dir, "goldens", "power.json"),
    golden_power_config
  )$configHash,
  power_golden$configHash
))

wrong_power_hash <- power_golden
wrong_power_hash$configHash <- paste0("sha256:", strrep("0", 64L))
stopifnot(inherits(try(
  validate_power_result(wrong_power_hash, golden_power_config),
  silent = TRUE
), "try-error"))

wrong_final <- validation_config
wrong_final$freeze$nPower <- 264L
wrong_final$freeze$nFinal <- 360L
stopifnot(inherits(try(
  validate_analysis_scores(validation_rows, wrong_final),
  silent = TRUE
), "try-error"))

wrong_power_parameters <- power_golden
wrong_power_parameters$adaptiveClasses <- as.list(c("D", "F"))
stopifnot(inherits(try(
  validate_power_result(wrong_power_parameters, golden_power_config),
  silent = TRUE
), "try-error"))
