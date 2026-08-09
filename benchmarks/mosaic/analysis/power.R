#!/usr/bin/env Rscript

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) {
  stop("usage: power.R <config.json> <result.json> <checkpoint.json>")
}

script_arg <- grep("^--file=", commandArgs(), value = TRUE)
script_path <- if (length(script_arg) == 1L) sub("^--file=", "", script_arg) else "."
analysis_dir <- dirname(normalizePath(script_path, mustWork = FALSE))
source(file.path(analysis_dir, "utils.R"), local = TRUE)
source(file.path(analysis_dir, "fit.R"), local = TRUE)
require_analysis_packages()

config <- read_json(args[[1]])
simulations <- as.integer(config$simulations)
if (simulations != 10000L) stop("power analysis requires exactly 10000 simulations")
seed <- as.character(config$seed)
numeric_seed <- as.numeric(config$numericSeed)
if (
  length(seed) != 1L || !nzchar(seed) || length(numeric_seed) != 1L ||
    !is.finite(numeric_seed) || numeric_seed %% 1 != 0
) {
  stop("power analysis requires a textual seed and one integer numericSeed")
}
if (!identical(seed, sprintf("L'Ecuyer-CMRG:%d", as.integer(numeric_seed)))) {
  stop("power seed must identify its L'Ecuyer-CMRG numericSeed")
}
minimum_effect <- as.numeric(config$minimumEffect)
if (
  length(minimum_effect) != 1L || !is.finite(minimum_effect) ||
    minimum_effect != 0.1
) {
  stop("power analysis requires the frozen minimumEffect of 0.1")
}
minimum <- as.integer(config$minimumCases)
maximum <- as.integer(config$maximumCases)
if (minimum < 24L || minimum %% 24L != 0L || maximum %% 24L != 0L || maximum < minimum) {
  stop("power candidate bounds must be ordered positive multiples of 24")
}
baseline_probability <- as.numeric(config$baselineProbability)
random_intercept_sd <- as.numeric(config$randomInterceptSd)
if (
  length(baseline_probability) != 1L || !is.finite(baseline_probability) ||
    baseline_probability <= 0 || baseline_probability >= 1
) {
  stop("baselineProbability must lie strictly between zero and one")
}
if (
  length(random_intercept_sd) != 1L || !is.finite(random_intercept_sd) ||
    random_intercept_sd < 0
) {
  stop("randomInterceptSd must be finite and non-negative")
}
adaptive_classes <- as.character(unlist(config$adaptiveClasses, use.names = FALSE))
if (
  length(adaptive_classes) == 0L || any(!adaptive_classes %in% LETTERS[1:6]) ||
    anyDuplicated(adaptive_classes)
) {
  stop("adaptiveClasses must contain unique composition classes A-F")
}

RNGkind("L'Ecuyer-CMRG")
set.seed(as.integer(numeric_seed))
config_hash <- sha256_file(args[[1]])
checkpoint_path <- args[[3]]

treatment_shift <- function(intercepts, baseline_probability, minimum_effect) {
  baseline_logit <- stats::qlogis(baseline_probability)
  objective <- function(shift) {
    mean(
      stats::plogis(baseline_logit + intercepts + shift) -
        stats::plogis(baseline_logit + intercepts)
    ) - minimum_effect
  }
  stats::uniroot(objective, interval = c(0, 20), tol = 1e-12)$root
}

simulate_dataset <- function(
  cases,
  baseline_probability,
  minimum_effect,
  random_sd,
  adaptive_classes
) {
  cells <- expand.grid(
    composition_class = LETTERS[1:6],
    domain = c("documents-finance", "software", "artifacts", "communication"),
    stringsAsFactors = FALSE
  )
  cell_rows <- cells[rep(seq_len(nrow(cells)), each = cases / 24L), , drop = FALSE]
  case_table <- data.frame(
    case_id = paste0("case-", seq_len(cases)),
    composition_class = cell_rows$composition_class,
    domain = cell_rows$domain,
    adaptive = cell_rows$composition_class %in% adaptive_classes,
    intercept = stats::rnorm(cases, 0, random_sd)
  )
  shift <- treatment_shift(
    case_table$intercept[case_table$adaptive],
    baseline_probability,
    minimum_effect
  )
  baseline_logit <- stats::qlogis(baseline_probability) + case_table$intercept
  case_table$baseline_probability <- stats::plogis(baseline_logit)
  case_table$treatment_probability <- stats::plogis(baseline_logit + shift)
  rows <- case_table[rep(seq_len(cases), each = 10L), , drop = FALSE]
  rows$condition <- factor(rep(rep(c("baseline", "M1"), each = 5L), cases))
  probability <- ifelse(
    rows$condition == "M1" & rows$adaptive,
    rows$treatment_probability,
    rows$baseline_probability
  )
  rows$success <- stats::rbinom(nrow(rows), 1L, probability)
  rows$composition_class <- factor(rows$composition_class, levels = LETTERS[1:6])
  rows$case_id <- factor(rows$case_id)
  rows
}

simulation_significant <- function(data) {
  fit <- glmer_attempt(data, "bobyqa")
  if (is.null(fit)) return(NA)
  contrast <- tryCatch(
    contrast_for(fit, data, list(
      id = "power",
      condition = "M1",
      reference = "baseline",
      classes = as.list(adaptive_classes),
      adaptive = TRUE
    )),
    error = function(error) NULL
  )
  if (is.null(contrast)) NA else contrast$pValue < 0.05
}

estimate_power <- function(cases) {
  outcomes <- replicate(simulations, {
    data <- simulate_dataset(
      cases,
      baseline_probability,
      minimum_effect,
      random_intercept_sd,
      adaptive_classes
    )
    simulation_significant(data)
  })
  valid <- !is.na(outcomes)
  if (mean(valid) < 0.95) return(NA_real_)
  mean(outcomes[valid])
}

candidates <- seq.int(minimum, maximum, by = 24L)
n_evaluated <- 0L
evaluated <- list()
if (file.exists(checkpoint_path)) {
  checkpoint <- read_json(checkpoint_path)
  if (
    !identical(as.integer(checkpoint$schemaVersion), 1L) ||
      !identical(as.character(checkpoint$configHash), config_hash)
  ) {
    stop("power checkpoint does not match the frozen configuration")
  }
  evaluated <- checkpoint$evaluated
  n_evaluated <- length(evaluated)
  if (n_evaluated > length(candidates)) {
    stop("power checkpoint contains too many candidates")
  }
  if (n_evaluated > 0L) {
    recorded <- vapply(
      evaluated,
      function(value) as.integer(value$cases),
      integer(1)
    )
    if (!identical(recorded, candidates[seq_len(n_evaluated)])) {
      stop("power checkpoint candidate sequence changed")
    }
  }
  random_seed <- as.integer(unlist(checkpoint$randomSeed, use.names = FALSE))
  if (length(random_seed) == 0L || any(is.na(random_seed))) {
    stop("power checkpoint has no valid RNG state")
  }
  assign(".Random.seed", random_seed, envir = .GlobalEnv)
}

write_checkpoint <- function() {
  value <- list(
    schemaVersion = 1L,
    configHash = config_hash,
    evaluated = evaluated,
    randomSeed = as.list(as.integer(.Random.seed))
  )
  temporary <- paste0(checkpoint_path, ".tmp")
  write_json(value, temporary)
  if (!file.rename(temporary, checkpoint_path)) {
    stop("could not atomically publish the power checkpoint")
  }
}

n_power <- NA_integer_
selected_power <- NA_real_
if (n_evaluated > 0L) {
  for (entry in evaluated) {
    if (isTRUE(entry$estimable) && as.numeric(entry$power) >= 0.8) {
      n_power <- as.integer(entry$cases)
      selected_power <- as.numeric(entry$power)
      break
    }
  }
}
if (is.na(n_power) && n_evaluated < length(candidates)) {
  for (candidate in candidates[(n_evaluated + 1L):length(candidates)]) {
    estimated <- estimate_power(candidate)
    evaluated[[length(evaluated) + 1L]] <- list(
      cases = as.integer(candidate),
      estimable = !is.na(estimated),
      power = if (is.na(estimated)) 0 else unname(estimated)
    )
    write_checkpoint()
    if (!is.na(estimated) && estimated >= 0.8) {
      n_power <- candidate
      selected_power <- estimated
      break
    }
  }
}
if (!is.na(n_power)) {
  matching <- Filter(
    function(entry) as.integer(entry$cases) == n_power,
    evaluated
  )
  if (length(matching) != 1L || !isTRUE(matching[[1]]$estimable)) {
    stop("power checkpoint selected result is inconsistent")
  }
  selected_power <- as.numeric(matching[[1]]$power)
}
if (is.na(n_power)) {
  if (length(evaluated) < length(candidates)) {
    stop("power checkpoint stopped before all candidates were evaluated")
  } else {
    stop("no candidate achieved 80% power")
  }
}
n_final <- round_up(max(240L, n_power), 120L)

result <- list(
  simulations = 10000L,
  seed = seed,
  numericSeed = as.integer(numeric_seed),
  configHash = config_hash,
  baselineProbability = baseline_probability,
  randomInterceptSd = random_intercept_sd,
  adaptiveClasses = as.list(adaptive_classes),
  nPower = as.integer(n_power),
  nFinal = as.integer(n_final),
  power = unname(selected_power),
  minimumEffect = minimum_effect
)
write_json(result, args[[2]])
