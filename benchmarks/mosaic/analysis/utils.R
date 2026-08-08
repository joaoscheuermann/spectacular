options(stringsAsFactors = FALSE)

formula_text <- "success ~ condition * composition_class + (1 | case_id)"

require_analysis_packages <- function() {
  packages <- c("digest", "jsonlite", "lme4", "sandwich")
  missing <- packages[!vapply(packages, requireNamespace, logical(1), quietly = TRUE)]
  if (length(missing) > 0L) {
    stop(sprintf("missing frozen R packages: %s", paste(missing, collapse = ", ")))
  }
}

read_json <- function(path) {
  jsonlite::fromJSON(path, simplifyVector = FALSE)
}

write_json <- function(value, path) {
  json <- jsonlite::toJSON(
    value,
    auto_unbox = TRUE,
    digits = 16,
    null = "null",
    pretty = TRUE
  )
  writeLines(enc2utf8(json), path, useBytes = TRUE)
}

sha256_file <- function(path) {
  paste0("sha256:", digest::digest(file = path, algo = "sha256", serialize = FALSE))
}

sha256_value <- function(value) {
  json <- jsonlite::toJSON(value, auto_unbox = TRUE, digits = 16, null = "null")
  paste0("sha256:", digest::digest(json, algo = "sha256", serialize = FALSE))
}

round_up <- function(value, multiple) {
  as.integer(ceiling(value / multiple) * multiple)
}

holm <- function(values) {
  as.numeric(p.adjust(values, method = "holm"))
}

validate_hash <- function(value, name) {
  if (
    length(value) != 1L || !is.character(value) || is.na(value) ||
      !grepl("^sha256:[a-f0-9]{64}$", value)
  ) {
    stop(sprintf("%s must be a SHA-256 identifier", name))
  }
  value
}
