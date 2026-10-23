description_candidates <- c("DESCRIPTION", "../../DESCRIPTION")
description_path <- description_candidates[file.exists(description_candidates)][1]
if (is.na(description_path)) {
  stop("Could not locate the package DESCRIPTION file")
}
expected_version <- read.dcf(description_path)[1, "Version"]
if (!identical(VERSION, expected_version)) {
  stop(paste("Expected", expected_version, "got", VERSION))
}
cat("R test_version passed:", VERSION, "\n")
