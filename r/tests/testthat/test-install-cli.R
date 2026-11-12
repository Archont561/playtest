.get_internal <- function(name) {
  if (exists(name, mode = "function", inherits = TRUE)) {
    return(get(name, mode = "function", inherits = TRUE))
  }
  get(name, envir = asNamespace("playtest"), inherits = FALSE)
}

.assert_raises <- function(expr_fn, expected_substrings) {
  caught <- tryCatch(
    {
      expr_fn()
      NULL
    },
    error = function(err) conditionMessage(err)
  )
  if (is.null(caught)) {
    stop("Expected error, but expression succeeded", call. = FALSE)
  }
  for (sub_str in expected_substrings) {
    if (!grepl(sub_str, caught, fixed = TRUE)) {
      stop(
        sprintf("Expected error message to contain '%s', got:\n%s", sub_str, caught),
        call. = FALSE
      )
    }
  }
}

.run_install_cli_suite <- function() {
  sha256_raw_fn <- .get_internal(".sha256_raw")
  compute_sha256_fn <- .get_internal(".compute_sha256_file")

  stopifnot(identical(
    sha256_raw_fn(raw(0L)),
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  ))
  stopifnot(identical(
    sha256_raw_fn(charToRaw("abc")),
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  ))
  stopifnot(identical(
    sha256_raw_fn(charToRaw("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
    "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
  ))

  fixture_dir <- tempfile("playtest-r-fixture-")
  dir.create(fixture_dir, recursive = TRUE)
  on.exit(unlink(fixture_dir, recursive = TRUE, force = TRUE), add = TRUE)

  pkg_sub <- file.path(fixture_dir, "playtest-cli-x86_64-unknown-linux-musl")
  dir.create(pkg_sub, recursive = TRUE)
  fake_bin_src <- file.path(pkg_sub, "playtest")
  writeLines(c("#!/usr/bin/env sh", paste("echo playtest", VERSION)), fake_bin_src)
  Sys.chmod(fake_bin_src, mode = "0755")

  archive_name <- "playtest-cli-x86_64-unknown-linux-musl.tar.gz"
  archive_fixture <- file.path(fixture_dir, archive_name)
  old_wd <- setwd(fixture_dir)
  tar_rc <- utils::tar(
    archive_fixture,
    files = "playtest-cli-x86_64-unknown-linux-musl",
    compression = "gzip"
  )
  setwd(old_wd)
  stopifnot(identical(as.integer(tar_rc), 0L))

  archive_sha <- compute_sha256_fn(archive_fixture, force_fallback = TRUE)
  stopifnot(identical(archive_sha, compute_sha256_fn(archive_fixture, force_fallback = FALSE)))
  sum_fixture <- file.path(fixture_dir, "sha256.sum")
  writeLines(sprintf("%s  %s", archive_sha, archive_name), sum_fixture)

  state <- new.env(parent = emptyenv())
  state$calls <- character(0L)
  mock_ok_downloader <- function(url, destfile) {
    state$calls <- c(state$calls, url)
    if (grepl("sha256\\.sum$", url)) {
      file.copy(sum_fixture, destfile, overwrite = TRUE)
      return(list(ok = TRUE, detail = ""))
    }
    if (grepl(archive_name, url, fixed = TRUE)) {
      file.copy(archive_fixture, destfile, overwrite = TRUE)
      return(list(ok = TRUE, detail = ""))
    }
    list(ok = FALSE, detail = "HTTP 404 Not Found")
  }

  cache_root <- tempfile("playtest-r-cache-")
  on.exit(unlink(cache_root, recursive = TRUE, force = TRUE), add = TRUE)
  base_test_url <- "https://releases.example.invalid/download"
  installed_path <- install_cli(
    cache_dir = cache_root,
    base_url = base_test_url,
    sysname = "Linux",
    machine = "x86_64",
    downloader = mock_ok_downloader
  )
  stopifnot(file.exists(installed_path))
  stopifnot(identical(length(state$calls), 2L))

  failing_downloader <- function(url, destfile) {
    list(ok = FALSE, detail = "offline")
  }
  cached_again <- install_cli(
    cache_dir = cache_root,
    base_url = base_test_url,
    sysname = "Linux",
    machine = "x86_64",
    downloader = failing_downloader
  )
  stopifnot(identical(cached_again, installed_path))

  .assert_raises(
    function() {
      install_cli(
        cache_dir = tempfile("cache-unsup-"),
        base_url = base_test_url,
        sysname = "SunOS",
        machine = "sparc64",
        downloader = failing_downloader
      )
    },
    c("SunOS/sparc64", paste0(base_test_url, "/v", VERSION, "/sha256.sum"), "fix:")
  )

  state$calls_404 <- character(0L)
  .assert_raises(
    function() {
      install_cli(
        cache_dir = tempfile("cache-404-"),
        base_url = base_test_url,
        sysname = "Linux",
        machine = "x86_64",
        downloader = function(url, destfile) {
          state$calls_404 <- c(state$calls_404, url)
          list(ok = FALSE, detail = "HTTP status was '404 Not Found'")
        }
      )
    },
    c("404", paste0(base_test_url, "/v", VERSION, "/sha256.sum"), "fix:")
  )
  stopifnot(identical(length(state$calls_404), 1L))

  tampered_sum <- file.path(fixture_dir, "bad-sha256.sum")
  writeLines(sprintf("%s  %s", paste(rep("0", 64L), collapse = ""), archive_name), tampered_sum)
  .assert_raises(
    function() {
      install_cli(
        cache_dir = tempfile("cache-badhash-"),
        base_url = base_test_url,
        sysname = "Linux",
        machine = "x86_64",
        downloader = function(url, destfile) {
          if (grepl("sha256\\.sum$", url)) {
            file.copy(tampered_sum, destfile, overwrite = TRUE)
          } else {
            file.copy(archive_fixture, destfile, overwrite = TRUE)
          }
          list(ok = TRUE, detail = "")
        }
      )
    },
    c("SHA-256 checksum mismatch", paste0(base_test_url, "/v", VERSION, "/", archive_name), "fix:")
  )

  .assert_raises(
    function() {
      install_cli(
        cache_dir = tempfile("cache-netfail-"),
        base_url = base_test_url,
        sysname = "Linux",
        machine = "x86_64",
        downloader = function(url, destfile) {
          stop("Connection timed out")
        }
      )
    },
    c("Connection timed out", paste0(base_test_url, "/v", VERSION, "/sha256.sum"), "fix:")
  )
  cat("R test-install-cli passed\n")
}

if ("package:testthat" %in% search()) {
  testthat::test_that("install_cli verifies checksums, caches binaries, and reports errors", {
    .run_install_cli_suite()
    testthat::expect_true(TRUE)
  })
} else {
  .run_install_cli_suite()
}
