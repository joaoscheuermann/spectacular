valid_fit <- function(fit) {
  coefficients <- tryCatch(lme4::fixef(fit), error = function(error) numeric())
  messages <- tryCatch(fit@optinfo$conv$lme4$messages, error = function(error) "invalid")
  length(coefficients) > 0L && all(is.finite(coefficients)) && is.null(messages)
}

glmer_attempt <- function(data, optimizer) {
  fit <- tryCatch(
    lme4::glmer(
      stats::as.formula(formula_text),
      data = data,
      family = stats::binomial(),
      control = lme4::glmerControl(
        optimizer = optimizer,
        optCtrl = list(maxfun = 200000)
      )
    ),
    error = function(error) NULL
  )
  if (is.null(fit) || !valid_fit(fit)) return(NULL)
  list(
    fit = fit,
    coefficients = lme4::fixef(fit),
    covariance = as.matrix(stats::vcov(fit)),
    estimator = paste0("glmer-", optimizer),
    validFitRate = 1,
    warnings = if (lme4::isSingular(fit, tol = 1e-5)) "singular_fit" else character()
  )
}

glm_hc2_attempt <- function(data) {
  fit <- tryCatch(
    stats::glm(
      success ~ condition * composition_class,
      data = data,
      family = stats::binomial()
    ),
    error = function(error) NULL
  )
  if (is.null(fit) || any(!is.finite(stats::coef(fit)))) return(NULL)
  covariance <- tryCatch(
    sandwich::vcovCL(fit, cluster = data$case_id, type = "HC2"),
    error = function(error) NULL
  )
  if (is.null(covariance) || any(!is.finite(covariance))) return(NULL)
  list(
    fit = fit,
    coefficients = stats::coef(fit),
    covariance = as.matrix(covariance),
    estimator = "glm-hc2-cluster",
    validFitRate = 1,
    warnings = character()
  )
}

bootstrap_coefficients <- function(data, repetitions, seed) {
  case_ids <- unique(data$case_id)
  set.seed(seed)
  estimates <- replicate(repetitions, {
    sampled <- sample(case_ids, length(case_ids), replace = TRUE)
    bootstrap <- do.call(rbind, lapply(seq_along(sampled), function(index) {
      rows <- data[data$case_id == sampled[[index]], , drop = FALSE]
      rows$case_id <- paste0(rows$case_id, "-", index)
      rows
    }))
    fit <- tryCatch(
      stats::glm(
        success ~ condition * composition_class,
        data = bootstrap,
        family = stats::binomial()
      ),
      error = function(error) NULL
    )
    if (is.null(fit)) return(rep(NA_real_, ncol(stats::model.matrix(
      success ~ condition * composition_class,
      data = data
    ))))
    stats::coef(fit)
  })
  t(estimates)
}

bootstrap_attempt <- function(data, repetitions = 10000L, seed = 104729L) {
  fit <- tryCatch(
    stats::glm(
      success ~ condition * composition_class,
      data = data,
      family = stats::binomial()
    ),
    error = function(error) NULL
  )
  if (is.null(fit)) return(NULL)
  estimates <- bootstrap_coefficients(data, repetitions, seed)
  valid <- apply(estimates, 1L, function(row) all(is.finite(row)))
  rate <- mean(valid)
  if (rate < 0.95) {
    return(list(estimator = "not-estimable", validFitRate = rate))
  }
  list(
    fit = fit,
    coefficients = colMeans(estimates[valid, , drop = FALSE]),
    covariance = stats::cov(estimates[valid, , drop = FALSE]),
    estimator = "case-bootstrap",
    validFitRate = rate,
    warnings = character()
  )
}

default_fitters <- function(repetitions = 10000L, seed = 104729L) {
  list(
    bobyqa = function(data) glmer_attempt(data, "bobyqa"),
    nloptwrap = function(data) glmer_attempt(data, "nloptwrap"),
    hc2 = glm_hc2_attempt,
    bootstrap = function(data) bootstrap_attempt(data, repetitions, seed)
  )
}

fit_with_fallbacks <- function(data, fitters = default_fitters()) {
  order <- c("bobyqa", "nloptwrap", "hc2", "bootstrap")
  failed <- character()
  for (name in order) {
    result <- fitters[[name]](data)
    if (!is.null(result)) {
      result$warnings <- unique(c(result$warnings %||% character(), failed))
      return(result)
    }
    failed <- c(failed, paste0(name, "_failed"))
  }
  list(
    estimator = "not-estimable",
    validFitRate = 0,
    warnings = unique(c(failed, "all_estimators_failed"))
  )
}

fit_contrasts_with_fallbacks <- function(data, specifications, fitters = default_fitters()) {
  order <- c("bobyqa", "nloptwrap", "hc2", "bootstrap")
  failed <- character()
  last_rate <- 0
  for (name in order) {
    result <- fitters[[name]](data)
    if (is.null(result)) {
      failed <- c(failed, paste0(name, "_failed"))
      next
    }
    last_rate <- result$validFitRate
    if (result$estimator == "not-estimable") {
      result$contrasts <- list()
      result$warnings <- unique(c(result$warnings %||% character(), failed))
      return(result)
    }
    contrasts <- tryCatch(
      lapply(specifications, function(specification) contrast_for(result, data, specification)),
      error = function(error) NULL
    )
    if (!is.null(contrasts)) {
      result$contrasts <- contrasts
      result$warnings <- unique(c(result$warnings %||% character(), failed))
      return(result)
    }
    failed <- c(failed, paste0(name, "_contrast_invalid"))
  }
  list(
    estimator = "not-estimable",
    validFitRate = last_rate,
    contrasts = list(),
    warnings = unique(c(failed, "all_estimators_failed"))
  )
}

`%||%` <- function(left, right) if (is.null(left)) right else left

fixed_formula <- function(fit) {
  if (inherits(fit, "merMod")) lme4::nobars(stats::formula(fit)) else stats::formula(fit)
}

design_matrix <- function(fit, data) {
  terms <- stats::delete.response(stats::terms(fixed_formula(fit)))
  stats::model.matrix(terms, data = data)
}

contrast_for <- function(result, data, specification) {
  target <- data
  class_match <- if (is.null(specification$classes)) {
    rep(FALSE, nrow(target))
  } else {
    target$composition_class %in% unlist(specification$classes)
  }
  if (!is.null(specification$classes) || isTRUE(specification$adaptive)) {
    target <- target[class_match | (isTRUE(specification$adaptive) & target$adaptive), , drop = FALSE]
  }
  if (nrow(target) == 0L) stop("contrast target contains no observations")
  reference <- target
  treatment <- target
  reference$condition <- factor(specification$reference, levels = levels(data$condition))
  treatment$condition <- factor(specification$condition, levels = levels(data$condition))
  x0 <- design_matrix(result$fit, reference)
  x1 <- design_matrix(result$fit, treatment)
  beta <- result$coefficients[colnames(x0)]
  covariance <- result$covariance[colnames(x0), colnames(x0), drop = FALSE]
  eta0 <- as.vector(x0 %*% beta)
  eta1 <- as.vector(x1 %*% beta)
  probability0 <- stats::plogis(eta0)
  probability1 <- stats::plogis(eta1)
  log_odds <- mean(eta1 - eta0)
  absolute <- mean(probability1 - probability0)
  log_gradient <- colMeans(x1 - x0)
  probability_gradient <- colMeans(
    probability1 * (1 - probability1) * x1 -
      probability0 * (1 - probability0) * x0
  )
  log_se <- sqrt(drop(t(log_gradient) %*% covariance %*% log_gradient))
  absolute_se <- sqrt(drop(t(probability_gradient) %*% covariance %*% probability_gradient))
  values <- c(log_odds, absolute, log_se, absolute_se)
  if (any(!is.finite(values)) || log_se <= 0 || absolute_se <= 0) {
    stop("contrast variance is zero or non-finite")
  }
  p_value <- 2 * stats::pnorm(-abs(log_odds / log_se))
  odds_ratio <- exp(log_odds)
  odds_interval <- exp(log_odds + c(-1.96, 1.96) * log_se)
  if (
    !is.finite(p_value) ||
      !is.finite(odds_ratio) ||
      any(!is.finite(odds_interval)) ||
      odds_ratio <= 0 ||
      any(odds_interval <= 0)
  ) {
    stop("contrast result is non-finite")
  }
  list(
    id = specification$id,
    estimate = unname(log_odds),
    oddsRatio = unname(odds_ratio),
    oddsRatioConfidence95 = list(
      lower = unname(odds_interval[[1]]),
      upper = unname(odds_interval[[2]])
    ),
    absoluteDifference = unname(absolute),
    absoluteDifferenceConfidence95 = list(
      lower = unname(max(-1, absolute - 1.96 * absolute_se)),
      upper = unname(min(1, absolute + 1.96 * absolute_se))
    ),
    pValue = unname(p_value),
    holmPValue = unname(p_value)
  )
}
