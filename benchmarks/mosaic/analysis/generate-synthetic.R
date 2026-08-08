#!/usr/bin/env Rscript

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) stop("usage: generate-synthetic.R <scores.csv>")

cells <- expand.grid(
  compositionClass = LETTERS[1:6],
  domain = c("documents-finance", "software", "artifacts", "communication"),
  stringsAsFactors = FALSE
)
n_final <- 360L
balanced_cells <- cells[rep(seq_len(nrow(cells)), each = n_final / nrow(cells)), , drop = FALSE]
cases <- transform(
  balanced_cells,
  caseId = sprintf("case-%03d", seq_len(nrow(balanced_cells))),
  adaptive = compositionClass %in% c("E", "F")
)
rows <- do.call(rbind, lapply(seq_len(nrow(cases)), function(index) {
  item <- cases[index, ]
  do.call(rbind, lapply(c("B2", "M1"), function(condition) {
    repetitions <- seq_len(5L)
    condition_offset <- if (condition == "M1") 50000000L else 0L
    successes <- repetitions <= if (condition == "M1" && item$adaptive) 3L else 2L
    data.frame(
      schemaVersion = 1L,
      runId = sprintf("run-%02d-%s-%d", index, condition, repetitions),
      attempt = 1L,
      studyId = "synthetic-study",
      phase = "confirmatory",
      caseId = item$caseId,
      familyId = sprintf("family-%02d", index),
      conditionId = condition,
      repetition = repetitions,
      pairedBlock = sprintf("block-%02d-%d", index, repetitions),
      provider = "openai",
      model = "openai/gpt-5.6-luna",
      effort = "medium",
      freezeHash = paste0("sha256:", strrep("a", 64L)),
      traceRootHash = sprintf(
        "sha256:%064x",
        as.integer(index * 100L) + repetitions + condition_offset
      ),
      traceDerivedHash = sprintf(
        "sha256:%064x",
        as.integer(index * 1000L) + repetitions + condition_offset
      ),
      evidenceHash = sprintf(
        "sha256:%064x",
        as.integer(index * 10000L) + repetitions + condition_offset
      ),
      worldHash = sprintf(
        "sha256:%064x",
        as.integer(index * 100000L) + repetitions + condition_offset
      ),
      modelCallBudget = NA_integer_,
      domain = item$domain,
      compositionClass = item$compositionClass,
      adaptive = item$adaptive,
      success = as.integer(successes),
      primaryEligible = TRUE,
      infrastructure = FALSE,
      failureCode = ifelse(successes, "", "deterministic_assertion_failed"),
      inputTokens = 100L,
      outputTokens = 20L,
      modelCalls = 2L,
      toolCalls = 1L,
      costUsd = 0.01,
      durationMs = 100L,
      stringsAsFactors = FALSE
    )
  }))
}))
utils::write.csv(rows, args[[1]], row.names = FALSE, na = "")
