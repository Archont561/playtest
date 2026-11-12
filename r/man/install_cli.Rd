\name{install_cli}
\alias{install_cli}
\title{Download, Verify, and Cache the Playtest CLI Binary}
\description{
Downloads the release archive matching \code{playtest::VERSION} and the host
platform from GitHub Releases, verifies its SHA-256 digest before unpacking,
and stores the executable under \code{tools::R_user_dir("playtest", which = "cache")}.
Nothing is downloaded when the package is installed or loaded. On R 4.5 and
newer \code{tools::sha256sum()} is used; on R 4.3 and 4.4 a pure base-R
FIPS 180-4 SHA-256 implementation verifies the archive before extraction.
}
\usage{
install_cli(
  cache_dir = tools::R_user_dir("playtest", which = "cache"),
  base_url = Sys.getenv(
    "PLAYTEST_RELEASE_BASE_URL",
    unset = "https://github.com/Archont561/playtest/releases/download"
  ),
  sysname = Sys.info()[["sysname"]],
  machine = Sys.info()[["machine"]],
  downloader = NULL
)
}
\arguments{
  \item{cache_dir}{Per-user cache root directory.}
  \item{base_url}{GitHub Releases download prefix.}
  \item{sysname}{Operating system name.}
  \item{machine}{CPU architecture identifier.}
  \item{downloader}{Optional custom download callback used by offline tests.}
}
\value{
A character scalar giving the normalized path to the verified cached
\code{playtest} executable.
}
\examples{
\dontrun{
cli_path <- install_cli()
}
}
