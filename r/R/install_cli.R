.PLAYTEST_DEFAULT_RELEASE_URL <- "https://github.com/Archont561/playtest/releases/download"

.SHA256_K <- c(
  1116352408, 1899447441, 3049323471, 3921009573, 961987163, 1508970993, 2453635748, 2870763221,
  3624381080, 310598401, 607225278, 1426881987, 1925078388, 2162078206, 2614888103, 3248222580,
  3835390401, 4022224774, 264347078, 604807628, 770255983, 1249150122, 1555081692, 1996064986,
  2554220882, 2821834349, 2952996808, 3210313671, 3336571891, 3584528711, 113926993, 338241895,
  666307205, 773529912, 1294757372, 1396182291, 1695183700, 1986661051, 2177026350, 2456956037,
  2730485921, 2820302411, 3259730800, 3345764771, 3516065817, 3600352804, 4094571909, 275423344,
  430227734, 506948616, 659060556, 883997877, 958139571, 1322822218, 1537002063, 1747873779,
  1955562222, 2024104815, 2227730452, 2361852424, 2428436474, 2756734187, 3204031479, 3329325298
)

.SHA256_H0 <- c(
  1779033703, 3144134277, 1013904242, 2773480762,
  1359893119, 2600822924, 528734635, 1541459225
)

.u32_xor <- function(a, b) {
  ahi <- as.integer(floor(a / 65536))
  alo <- as.integer(a - ahi * 65536)
  bhi <- as.integer(floor(b / 65536))
  blo <- as.integer(b - bhi * 65536)
  as.double(bitwXor(ahi, bhi)) * 65536 + as.double(bitwXor(alo, blo))
}

.u32_and <- function(a, b) {
  ahi <- as.integer(floor(a / 65536))
  alo <- as.integer(a - ahi * 65536)
  bhi <- as.integer(floor(b / 65536))
  blo <- as.integer(b - bhi * 65536)
  as.double(bitwAnd(ahi, bhi)) * 65536 + as.double(bitwAnd(alo, blo))
}

.u32_not <- function(a) {
  ahi <- as.integer(floor(a / 65536))
  alo <- as.integer(a - ahi * 65536)
  as.double(bitwXor(ahi, 65535L)) * 65536 + as.double(bitwXor(alo, 65535L))
}

.u32_rotr <- function(a, n) {
  two_n <- 2^n
  floor(a / two_n) + (a %% two_n) * (2^(32 - n))
}

.u32_shr <- function(a, n) {
  floor(a / (2^n))
}

# Pure base-R FIPS 180-4 SHA-256 fallback for R 4.3-4.4 where tools::sha256sum() is absent.
.sha256_raw <- function(bytes) {
  msg <- as.integer(bytes)
  msg_len <- length(msg)
  pad_zeros <- (55L - (msg_len %% 64L)) %% 64L
  bit_hi <- floor((as.double(msg_len) * 8) / 4294967296)
  bit_lo <- (as.double(msg_len) * 8) %% 4294967296
  len_bytes <- integer(8L)
  for (idx in 1:4) {
    shift <- 2^((4 - idx) * 8)
    len_bytes[idx] <- as.integer(floor(bit_hi / shift) %% 256)
    len_bytes[idx + 4L] <- as.integer(floor(bit_lo / shift) %% 256)
  }
  padded <- c(msg, 128L, rep.int(0L, pad_zeros), len_bytes)
  h <- .SHA256_H0
  n_blocks <- length(padded) %/% 64L
  w <- numeric(64L)

  for (blk in seq_len(n_blocks)) {
    off <- (blk - 1L) * 64L
    for (i in 1:16) {
      base <- off + (i - 1L) * 4L
      w[i] <- padded[base + 1L] * 16777216 + padded[base + 2L] * 65536 +
        padded[base + 3L] * 256 + padded[base + 4L]
    }
    for (i in 17:64) {
      s0 <- .u32_xor(.u32_xor(.u32_rotr(w[i - 15L], 7), .u32_rotr(w[i - 15L], 18)), .u32_shr(w[i - 15L], 3))
      s1 <- .u32_xor(.u32_xor(.u32_rotr(w[i - 2L], 17), .u32_rotr(w[i - 2L], 19)), .u32_shr(w[i - 2L], 10))
      w[i] <- (w[i - 16L] + s0 + w[i - 7L] + s1) %% 4294967296
    }
    a <- h[1L]; b <- h[2L]; c <- h[3L]; d <- h[4L]
    e <- h[5L]; f <- h[6L]; g <- h[7L]; hh <- h[8L]
    for (i in 1:64) {
      big_s1 <- .u32_xor(.u32_xor(.u32_rotr(e, 6), .u32_rotr(e, 11)), .u32_rotr(e, 25))
      ch <- .u32_xor(.u32_and(e, f), .u32_and(.u32_not(e), g))
      temp1 <- (hh + big_s1 + ch + .SHA256_K[i] + w[i]) %% 4294967296
      big_s0 <- .u32_xor(.u32_xor(.u32_rotr(a, 2), .u32_rotr(a, 13)), .u32_rotr(a, 22))
      maj <- .u32_xor(.u32_xor(.u32_and(a, b), .u32_and(a, c)), .u32_and(b, c))
      temp2 <- (big_s0 + maj) %% 4294967296
      hh <- g; g <- f; f <- e
      e <- (d + temp1) %% 4294967296
      d <- c; c <- b; b <- a
      a <- (temp1 + temp2) %% 4294967296
    }
    h <- (h + c(a, b, c, d, e, f, g, hh)) %% 4294967296
  }

  parts <- character(8L)
  for (idx in 1:8) {
    hi <- as.integer(floor(h[idx] / 65536))
    lo <- as.integer(h[idx] - hi * 65536)
    parts[idx] <- sprintf("%04x%04x", hi, lo)
  }
  paste0(parts, collapse = "")
}

.compute_sha256_file <- function(path, force_fallback = FALSE) {
  tools_ns <- asNamespace("tools")
  if (!isTRUE(force_fallback) && exists("sha256sum", envir = tools_ns, inherits = FALSE)) {
    fn <- get("sha256sum", envir = tools_ns)
    return(tolower(unname(fn(path))[[1L]]))
  }
  size <- file.info(path)$size
  raw_bytes <- readBin(path, what = "raw", n = size)
  .sha256_raw(raw_bytes)
}

.resolve_cli_platform <- function(sysname, machine, version, base_url) {
  sys_lower <- tolower(trimws(sysname))
  arch_lower <- tolower(trimws(machine))
  norm_arch <- if (arch_lower %in% c("x86_64", "x86-64", "amd64")) {
    "x64"
  } else if (arch_lower %in% c("aarch64", "arm64")) {
    "arm64"
  } else {
    ""
  }

  key <- paste0(sys_lower, "/", norm_arch)
  table <- list(
    "linux/x64" = list(triple = "x86_64-unknown-linux-musl", ext = ".tar.gz", binary = "playtest"),
    "linux/arm64" = list(triple = "aarch64-unknown-linux-musl", ext = ".tar.gz", binary = "playtest"),
    "darwin/x64" = list(triple = "x86_64-apple-darwin", ext = ".tar.gz", binary = "playtest"),
    "darwin/arm64" = list(triple = "aarch64-apple-darwin", ext = ".tar.gz", binary = "playtest"),
    "windows/x64" = list(triple = "x86_64-pc-windows-msvc", ext = ".zip", binary = "playtest.exe")
  )
  spec <- table[[key]]
  if (is.null(spec)) {
    sum_url <- sprintf("%s/v%s/sha256.sum", sub("/+$", "", base_url), version)
    stop(
      sprintf(
        "Unsupported platform %s/%s for prebuilt Playtest CLI v%s.\n  url: %s\n  fix: Build from source with Rust >= 1.88 via `cargo install playtest-cli --version %s --locked`.",
        sysname, machine, version, sum_url, version
      ),
      call. = FALSE
    )
  }
  spec$archive_filename <- paste0("playtest-cli-", spec$triple, spec$ext)
  spec
}

.default_cli_downloader <- function(url, destfile) {
  status <- tryCatch(
    suppressWarnings(utils::download.file(url, destfile = destfile, mode = "wb", quiet = TRUE)),
    error = function(err) conditionMessage(err)
  )
  if (is.character(status)) {
    return(list(ok = FALSE, detail = status))
  }
  if (!identical(as.integer(status), 0L) || !file.exists(destfile)) {
    return(list(ok = FALSE, detail = sprintf("download.file exited with status %s", as.character(status))))
  }
  list(ok = TRUE, detail = "")
}

.fetch_release_file <- function(url, destfile, version, downloader) {
  dl_fn <- if (is.null(downloader)) .default_cli_downloader else downloader
  result <- tryCatch(
    dl_fn(url, destfile),
    error = function(err) list(ok = FALSE, detail = conditionMessage(err))
  )
  if (!isTRUE(result$ok) || !file.exists(destfile)) {
    detail <- if (!is.null(result$detail) && nzchar(result$detail)) result$detail else "download failed"
    stop(
      sprintf(
        "Failed to download Playtest CLI v%s release asset (%s).\n  url: %s\n  fix: Verify GitHub Release v%s is published and check network access to github.com (install_cli() never falls back to another version).",
        version, detail, url, version
      ),
      call. = FALSE
    )
  }
}

.lookup_sha256_entry <- function(sum_file, archive_filename, sum_url, version) {
  lines <- readLines(sum_file, warn = FALSE, encoding = "UTF-8")
  for (raw_line in lines) {
    trimmed <- trimws(raw_line)
    if (!nzchar(trimmed)) next
    parts <- strsplit(trimmed, "[[:space:]]+")[[1L]]
    if (length(parts) < 2L) next
    candidate <- sub("^\\*", "", parts[[2L]])
    if (identical(candidate, archive_filename) || identical(basename(candidate), archive_filename)) {
      digest <- tolower(parts[[1L]])
      if (grepl("^[0-9a-f]{64}$", digest)) {
        return(digest)
      }
    }
  }
  stop(
    sprintf(
      "Checksum entry for %s is missing from sha256.sum in release v%s.\n  url: %s\n  fix: Verify GitHub Release v%s includes %s and sha256.sum.",
      archive_filename, version, sum_url, version, archive_filename
    ),
    call. = FALSE
  )
}

#' Download, Verify, and Cache the Playtest CLI Binary
#'
#' Downloads the release archive matching `playtest::VERSION` and the host
#' platform from GitHub Releases, verifies its SHA-256 digest before unpacking
#' (using `tools::sha256sum()` on R >= 4.5 or a pure base-R FIPS 180-4 SHA-256
#' implementation on R 4.3--4.4), and stores the binary under
#' `tools::R_user_dir("playtest", which = "cache")`. Nothing is downloaded when
#' the package is installed or loaded.
#'
#' @param cache_dir Per-user cache root directory. Defaults to
#'   `tools::R_user_dir("playtest", which = "cache")`.
#' @param base_url Release download base URL.
#' @param sysname Operating system name (defaults to `Sys.info()[["sysname"]]`).
#' @param machine CPU architecture name (defaults to `Sys.info()[["machine"]]`).
#' @param downloader Optional `function(url, destfile)` returning
#'   `list(ok = TRUE/FALSE, detail = "...")`, used for offline testing.
#' @return Normalized path to the verified cached `playtest` executable.
#' @export
install_cli <- function(
  cache_dir = tools::R_user_dir("playtest", which = "cache"),
  base_url = Sys.getenv(
    "PLAYTEST_RELEASE_BASE_URL",
    unset = "https://github.com/Archont561/playtest/releases/download"
  ),
  sysname = Sys.info()[["sysname"]],
  machine = Sys.info()[["machine"]],
  downloader = NULL
) {
  clean_url <- sub("/+$", "", if (nzchar(base_url)) base_url else .PLAYTEST_DEFAULT_RELEASE_URL)
  spec <- .resolve_cli_platform(sysname, machine, VERSION, clean_url)
  cached_bin <- file.path(cache_dir, "cli", paste0("v", VERSION), spec$triple, spec$binary)
  if (file.exists(cached_bin)) {
    return(normalizePath(cached_bin, winslash = "/", mustWork = TRUE))
  }

  work_dir <- tempfile(pattern = "playtest-cli-dl-")
  dir.create(work_dir, recursive = TRUE, showWarnings = FALSE)
  on.exit(unlink(work_dir, recursive = TRUE, force = TRUE), add = TRUE)

  sum_url <- sprintf("%s/v%s/sha256.sum", clean_url, VERSION)
  sum_path <- file.path(work_dir, "sha256.sum")
  .fetch_release_file(sum_url, sum_path, VERSION, downloader)
  expected_hex <- .lookup_sha256_entry(sum_path, spec$archive_filename, sum_url, VERSION)

  archive_url <- sprintf("%s/v%s/%s", clean_url, VERSION, spec$archive_filename)
  archive_path <- file.path(work_dir, spec$archive_filename)
  .fetch_release_file(archive_url, archive_path, VERSION, downloader)

  actual_hex <- .compute_sha256_file(archive_path)
  if (!identical(actual_hex, expected_hex)) {
    stop(
      sprintf(
        "SHA-256 checksum mismatch for %s (expected %s, got %s).\n  url: %s\n  fix: Retry `playtest::install_cli()` over a clean connection or inspect GitHub Release v%s.",
        spec$archive_filename, expected_hex, actual_hex, archive_url, VERSION
      ),
      call. = FALSE
    )
  }

  unpack_dir <- file.path(work_dir, "unpacked")
  dir.create(unpack_dir, recursive = TRUE, showWarnings = FALSE)
  if (identical(spec$ext, ".zip")) {
    utils::unzip(archive_path, exdir = unpack_dir)
  } else {
    rc <- utils::untar(archive_path, exdir = unpack_dir)
    if (!identical(as.integer(rc), 0L)) {
      stop(
        sprintf(
          "Failed to unpack archive %s.\n  url: %s\n  fix: Retry `playtest::install_cli()`.",
          spec$archive_filename, archive_url
        ),
        call. = FALSE
      )
    }
  }

  candidates <- list.files(unpack_dir, pattern = paste0("^", gsub("\\.", "\\\\.", spec$binary), "$"), recursive = TRUE, full.names = TRUE)
  if (length(candidates) < 1L) {
    stop(
      sprintf(
        "Executable %s was not found in release archive %s.\n  url: %s\n  fix: Verify GitHub Release v%s archive layout matches Spec 0012.",
        spec$binary, spec$archive_filename, archive_url, VERSION
      ),
      call. = FALSE
    )
  }

  target_dir <- dirname(cached_bin)
  dir.create(target_dir, recursive = TRUE, showWarnings = FALSE)
  staged_bin <- tempfile(pattern = ".playtest-bin-", tmpdir = target_dir)
  if (!file.copy(candidates[[1L]], staged_bin, overwrite = TRUE)) {
    stop(
      sprintf(
        "Could not copy verified CLI binary into cache directory %s.\n  url: %s\n  fix: Ensure %s is writable.",
        target_dir, archive_url, target_dir
      ),
      call. = FALSE
    )
  }
  if (!identical(spec$ext, ".zip")) {
    Sys.chmod(staged_bin, mode = "0755")
  }
  if (!file.rename(staged_bin, cached_bin)) {
    file.copy(staged_bin, cached_bin, overwrite = TRUE)
    unlink(staged_bin, force = TRUE)
  }
  if (!identical(spec$ext, ".zip")) {
    Sys.chmod(cached_bin, mode = "0755")
  }
  normalizePath(cached_bin, winslash = "/", mustWork = TRUE)
}
