source("../../R/version.R")

test_version <- function() {
  if (VERSION != "0.1.0") {
    stop(paste("Expected 0.1.0, got", VERSION))
  }
  cat("R test_version passed:", VERSION, "\n")
}

test_version()
