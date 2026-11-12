description_candidates <- c("DESCRIPTION", "../../DESCRIPTION")
existing <- description_candidates[file.exists(description_candidates)]
expected_version <- if (length(existing) > 0L) {
  unname(read.dcf(existing[[1L]])[1L, "Version"])
} else {
  as.character(utils::packageVersion("playtest"))
}
if (!identical(VERSION, expected_version)) {
  stop(paste("Expected", expected_version, "got", VERSION))
}
cat("R test_version passed:", VERSION, "\n")
