score_required_columns <- c(
  "schemaVersion", "runId", "attempt", "studyId", "phase", "caseId",
  "familyId", "conditionId", "repetition", "pairedBlock", "provider", "model", "effort",
  "freezeHash", "traceRootHash", "traceDerivedHash", "evidenceHash",
  "worldHash", "modelCallBudget", "domain", "compositionClass", "adaptive",
  "success", "primaryEligible", "infrastructure"
)

config_string <- function(config, name) {
  value <- config[[name]]
  if (is.null(value) || length(value) != 1L || !is.character(value) || !nzchar(value)) {
    stop(sprintf("analysis config requires one non-empty %s", name))
  }
  value
}

exact_object <- function(value, expected_names, name) {
  if (
    is.null(value) || !is.list(value) ||
      !identical(sort(names(value)), sort(expected_names))
  ) {
    stop(sprintf("%s must contain exactly: %s", name, paste(expected_names, collapse = ", ")))
  }
  value
}

scalar_number <- function(value, name) {
  result <- suppressWarnings(as.numeric(value))
  if (length(result) != 1L || !is.finite(result)) {
    stop(sprintf("%s must be one finite number", name))
  }
  result
}

positive_integer <- function(value, name) {
  result <- scalar_number(value, name)
  if (result <= 0 || result %% 1 != 0) {
    stop(sprintf("%s must be one positive integer", name))
  }
  as.integer(result)
}

adaptive_classes <- function(value, name) {
  result <- as.character(unlist(value, use.names = FALSE))
  if (
    length(result) == 0L || any(!result %in% LETTERS[1:6]) ||
      anyDuplicated(result)
  ) {
    stop(sprintf("%s must contain unique composition classes A-F", name))
  }
  result
}

validate_freeze_projection <- function(config) {
  freeze <- exact_object(
    config$freeze,
    c(
      "manifestHash", "nPower", "nFinal", "repetitions", "minimumEffect",
      "modelCallBudgetP95", "artifactHashes"
    ),
    "config.freeze"
  )
  artifacts <- exact_object(
    freeze$artifactHashes,
    c("analysis", "powerConfig", "powerResult"),
    "config.freeze.artifactHashes"
  )
  result <- list(
    manifestHash = validate_hash(freeze$manifestHash, "freeze.manifestHash"),
    nPower = positive_integer(freeze$nPower, "freeze.nPower"),
    nFinal = positive_integer(freeze$nFinal, "freeze.nFinal"),
    repetitions = positive_integer(freeze$repetitions, "freeze.repetitions"),
    minimumEffect = scalar_number(freeze$minimumEffect, "freeze.minimumEffect"),
    modelCallBudgetP95 = positive_integer(
      freeze$modelCallBudgetP95,
      "freeze.modelCallBudgetP95"
    ),
    analysisHash = validate_hash(
      artifacts$analysis,
      "freeze.artifactHashes.analysis"
    ),
    powerConfigHash = validate_hash(
      artifacts$powerConfig,
      "freeze.artifactHashes.powerConfig"
    ),
    powerResultHash = validate_hash(
      artifacts$powerResult,
      "freeze.artifactHashes.powerResult"
    )
  )
  if (result$repetitions != 5L) stop("freeze.repetitions must equal five")
  if (result$minimumEffect != 0.1) stop("freeze.minimumEffect must equal 0.1")
  if (result$nPower %% 24L != 0L) stop("freeze.nPower must be a multiple of 24")
  expected_final <- round_up(max(240L, result$nPower), 120L)
  if (result$nFinal != expected_final) {
    stop("freeze.nFinal violates the registered minimum and rounding rule")
  }
  result
}

validate_power_parameters <- function(config) {
  parameters <- exact_object(
    config$powerParameters,
    c(
      "seed", "numericSeed", "baselineProbability", "randomInterceptSd",
      "adaptiveClasses"
    ),
    "config.powerParameters"
  )
  seed <- as.character(parameters$seed)
  numeric_seed <- scalar_number(parameters$numericSeed, "powerParameters.numericSeed")
  if (length(seed) != 1L || !nzchar(seed) || numeric_seed %% 1 != 0) {
    stop("powerParameters requires a textual seed and an integer numericSeed")
  }
  if (!identical(seed, sprintf("L'Ecuyer-CMRG:%d", as.integer(numeric_seed)))) {
    stop("powerParameters.seed must identify numericSeed")
  }
  baseline <- scalar_number(
    parameters$baselineProbability,
    "powerParameters.baselineProbability"
  )
  random_sd <- scalar_number(
    parameters$randomInterceptSd,
    "powerParameters.randomInterceptSd"
  )
  if (baseline <= 0 || baseline >= 1) {
    stop("powerParameters.baselineProbability must lie strictly between zero and one")
  }
  if (random_sd < 0) stop("powerParameters.randomInterceptSd cannot be negative")
  list(
    seed = seed,
    numericSeed = as.integer(numeric_seed),
    baselineProbability = baseline,
    randomInterceptSd = random_sd,
    adaptiveClasses = adaptive_classes(
      parameters$adaptiveClasses,
      "powerParameters.adaptiveClasses"
    )
  )
}

validate_analysis_options <- function(config) {
  parameters <- validate_power_parameters(config)
  if (positive_integer(
    config$bootstrapRepetitions,
    "bootstrapRepetitions"
  ) != 10000L) {
    stop("bootstrapRepetitions must equal 10000")
  }
  bootstrap_seed <- scalar_number(config$bootstrapSeed, "bootstrapSeed")
  if (
    bootstrap_seed %% 1 != 0 ||
      as.integer(bootstrap_seed) != parameters$numericSeed
  ) {
    stop("bootstrapSeed must equal powerParameters.numericSeed")
  }
  config_string(config, "powerResultPath")
  family <- config_string(config, "family")
  if (!is.null(config$contrasts) && family != "secondary") {
    stop("only a registered secondary family may define custom contrasts")
  }
  invisible(parameters)
}

validate_power_result <- function(value, config) {
  power <- exact_object(
    value,
    c(
      "simulations", "seed", "numericSeed", "configHash",
      "baselineProbability", "randomInterceptSd", "adaptiveClasses",
      "nPower", "nFinal", "power", "minimumEffect"
    ),
    "power result"
  )
  freeze <- validate_freeze_projection(config)
  parameters <- validate_power_parameters(config)
  if (positive_integer(power$simulations, "power.simulations") != 10000L) {
    stop("power.simulations must equal 10000")
  }
  if (!identical(as.character(power$seed), parameters$seed)) {
    stop("power.seed differs from the registered power config")
  }
  if (scalar_number(power$numericSeed, "power.numericSeed") != parameters$numericSeed) {
    stop("power.numericSeed differs from the registered power config")
  }
  if (
    validate_hash(power$configHash, "power.configHash") !=
      freeze$powerConfigHash
  ) {
    stop("power.configHash differs from freeze.artifactHashes.powerConfig")
  }
  if (
    scalar_number(power$baselineProbability, "power.baselineProbability") !=
      parameters$baselineProbability ||
      scalar_number(power$randomInterceptSd, "power.randomInterceptSd") !=
        parameters$randomInterceptSd ||
      !identical(
        adaptive_classes(power$adaptiveClasses, "power.adaptiveClasses"),
        parameters$adaptiveClasses
      )
  ) {
    stop("power simulation parameters differ from the registered config")
  }
  if (
    positive_integer(power$nPower, "power.nPower") != freeze$nPower ||
      positive_integer(power$nFinal, "power.nFinal") != freeze$nFinal ||
      scalar_number(power$minimumEffect, "power.minimumEffect") !=
        freeze$minimumEffect
  ) {
    stop("power sample size or minimum effect differs from the freeze")
  }
  estimated <- scalar_number(power$power, "power.power")
  if (estimated < 0.8 || estimated > 1) {
    stop("power estimate must reach the registered 80 percent target")
  }
  power
}

validate_power_result_file <- function(path, config) {
  freeze <- validate_freeze_projection(config)
  if (sha256_file(path) != freeze$powerResultHash) {
    stop("power result bytes differ from freeze.artifactHashes.powerResult")
  }
  validate_power_result(read_json(path), config)
}

strict_logical <- function(value, name) {
  if (is.logical(value) && !anyNA(value)) return(value)
  if (is.numeric(value) && !anyNA(value) && all(value %in% c(0, 1))) {
    return(as.logical(value))
  }
  normalized <- tolower(as.character(value))
  if (anyNA(normalized) || !all(normalized %in% c("true", "false"))) {
    stop(sprintf("%s must contain only booleans", name))
  }
  normalized == "true"
}

validate_hash_column <- function(value, name) {
  if (
    anyNA(value) ||
      !all(grepl("^sha256:[a-f0-9]{64}$", as.character(value)))
  ) {
    stop(sprintf("%s must contain only SHA-256 identifiers", name))
  }
}

validate_case_metadata <- function(scores) {
  columns <- c("familyId", "domain", "compositionClass", "adaptive")
  grouped <- split(scores, as.character(scores$caseId))
  stable <- vapply(grouped, function(rows) {
    all(vapply(columns, function(column) {
      length(unique(as.character(rows[[column]]))) == 1L
    }, logical(1)))
  }, logical(1))
  if (!all(stable)) stop("case metadata differs across paired observations")

  cases <- scores[!duplicated(scores$caseId), , drop = FALSE]
  classes <- LETTERS[1:6]
  domains <- c("documents-finance", "software", "artifacts", "communication")
  if (
    !all(as.character(cases$compositionClass) %in% classes) ||
      !all(as.character(cases$domain) %in% domains)
  ) {
    stop("case metadata contains an unknown composition class or domain")
  }
  matrix <- table(
    factor(cases$compositionClass, levels = classes),
    factor(cases$domain, levels = domains)
  )
  if (any(matrix == 0L) || length(unique(as.integer(matrix))) != 1L) {
    stop("case matrix must balance all six classes across all four domains")
  }
}

validate_pairing <- function(scores, baseline, registered_repetitions) {
  repetitions <- suppressWarnings(as.integer(scores$repetition))
  if (
    anyNA(repetitions) ||
      !all(repetitions %in% seq_len(registered_repetitions))
  ) {
    stop("analysis repetitions differ from the freeze")
  }
  keys <- paste(scores$caseId, scores$conditionId, repetitions, sep = "\r")
  if (anyDuplicated(keys)) {
    stop("analysis contains duplicate case-condition-repetition rows")
  }
  expected <- expand.grid(
    caseId = unique(as.character(scores$caseId)),
    conditionId = c(baseline, "M1"),
    repetition = seq_len(registered_repetitions),
    stringsAsFactors = FALSE
  )
  expected_keys <- paste(
    expected$caseId,
    expected$conditionId,
    expected$repetition,
    sep = "\r"
  )
  if (length(keys) != length(expected_keys) || !setequal(keys, expected_keys)) {
    stop("analysis requires five paired repetitions for both frozen conditions")
  }
  paired_blocks <- as.character(scores$pairedBlock)
  if (anyNA(paired_blocks) || any(!nzchar(paired_blocks))) {
    stop("pairedBlock must be present on every observation")
  }
  pair_keys <- paste(scores$caseId, repetitions, sep = "\r")
  blocks_by_pair <- split(paired_blocks, pair_keys)
  if (!all(vapply(blocks_by_pair, function(values) {
    length(values) == 2L && length(unique(values)) == 1L
  }, logical(1)))) {
    stop("baseline and M1 must share one pairedBlock per case and repetition")
  }
  pairs_by_block <- split(pair_keys, paired_blocks)
  if (!all(vapply(pairs_by_block, function(values) {
    length(unique(values)) == 1L
  }, logical(1)))) {
    stop("pairedBlock cannot aggregate multiple case-repetition pairs")
  }
}

validate_budget_family <- function(scores, config, family) {
  freeze <- validate_freeze_projection(config)
  budget <- suppressWarnings(as.numeric(scores$modelCallBudget))
  if (family == "sensitivity") {
    if (anyNA(budget) || !all(budget == freeze$modelCallBudgetP95)) {
      stop("sensitivity rows must use the single frozen model-call cap")
    }
    return(invisible(NULL))
  }
  if (any(!is.na(budget))) {
    stop("primary and replication analyses must use uncapped rows")
  }
}

validate_analysis_scores <- function(scores, config) {
  if (!all(score_required_columns %in% names(scores))) {
    stop("score dataset is missing canonical provenance columns")
  }
  family <- config_string(config, "family")
  if (!family %in% c("primary", "secondary", "replication", "sensitivity")) {
    stop("analysis family is invalid")
  }
  baseline <- config_string(config, "baselineCondition")
  if (!baseline %in% c("B0", "B1", "B2", "B3")) {
    stop("baselineCondition must be the frozen B0-B3 selection")
  }
  expected_conditions <- sort(c(baseline, "M1"))
  actual_conditions <- sort(unique(as.character(scores$conditionId)))
  if (!identical(actual_conditions, expected_conditions)) {
    stop("analysis requires exactly M1 and the frozen baseline")
  }
  if (anyDuplicated(as.character(scores$runId))) {
    stop("runId must be unique after technical-attempt resolution")
  }
  attempt <- suppressWarnings(as.integer(scores$attempt))
  if (anyNA(attempt) || any(attempt <= 0L)) stop("attempt must be positive")

  expected_study <- config_string(config, "studyId")
  expected_provider <- config_string(config, "provider")
  expected_model <- config_string(config, "model")
  expected_effort <- config_string(config, "effort")
  freeze <- validate_freeze_projection(config)
  validate_analysis_options(config)
  expected_freeze <- freeze$manifestHash
  frozen_analysis <- freeze$analysisHash
  implementation <- validate_hash(
    config_string(config, "implementationHash"),
    "implementationHash"
  )
  if (!identical(implementation, frozen_analysis)) {
    stop("implementationHash differs from freeze.artifactHashes.analysis")
  }
  homogeneous <- list(
    studyId = expected_study,
    provider = expected_provider,
    model = expected_model,
    effort = expected_effort,
    freezeHash = expected_freeze
  )
  for (name in names(homogeneous)) {
    values <- unique(as.character(scores[[name]]))
    if (length(values) != 1L || !identical(values[[1]], homogeneous[[name]])) {
      stop(sprintf("score dataset %s differs from the frozen analysis config", name))
    }
  }
  expected_phase <- if (family == "replication") "replication" else "confirmatory"
  if (!all(as.character(scores$phase) == expected_phase)) {
    stop("score dataset phase differs from its separate analysis family")
  }
  eligible <- strict_logical(scores$primaryEligible, "primaryEligible")
  if (!all(eligible)) {
    stop("primaryEligible differs from the frozen family and budget policy")
  }
  success <- suppressWarnings(as.integer(scores$success))
  if (anyNA(success) || !all(success %in% c(0L, 1L))) {
    stop("success must contain only zero or one")
  }
  scores$adaptive <- strict_logical(scores$adaptive, "adaptive")
  scores$primaryEligible <- eligible
  scores$success <- success
  for (name in c(
    "freezeHash", "traceRootHash", "traceDerivedHash", "evidenceHash", "worldHash"
  )) {
    validate_hash_column(scores[[name]], name)
  }
  case_count <- length(unique(as.character(scores$caseId)))
  if (case_count != freeze$nFinal) {
    stop("score dataset case count differs from freeze.nFinal")
  }
  expected_rows <- freeze$nFinal * 2L * freeze$repetitions
  if (nrow(scores) != expected_rows) {
    stop("score dataset row count differs from freeze.nFinal times ten")
  }
  validate_budget_family(scores, config, family)
  validate_pairing(scores, baseline, freeze$repetitions)
  validate_case_metadata(scores)
  scores
}
