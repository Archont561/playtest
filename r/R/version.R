.discover_playtest_version <- function() {
  candidates <- c("DESCRIPTION", "../../DESCRIPTION", "../DESCRIPTION")
  existing <- candidates[file.exists(candidates)]
  if (length(existing) > 0L) {
    return(unname(read.dcf(existing[[1L]], fields = "Version")[1L, 1L]))
  }
  as.character(utils::packageVersion("playtest"))
}

#' Package version constant discovered from DESCRIPTION
#' @export
VERSION <- .discover_playtest_version()
