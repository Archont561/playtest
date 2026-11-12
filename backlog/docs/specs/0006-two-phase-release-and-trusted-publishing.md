# Specification 0006: Two-Phase Release and Trusted Publishing

## Overview
Specifies the packaging standards, artifact storage schemas, OpenID Connect (OIDC) authentication flows, and registry publication rules implemented in `.github/workflows/release.yml`.

This specification describes the workflow as it is implemented. The CLI-first distribution model of [ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md) and [ADR 0016](../adr/0016-cli-binary-distribution-channels.md) changes several jobs; those changes are not implemented yet and are listed under [Planned Changes](#planned-changes).

## Two-Phase Pipeline Architecture

### Phase 1: Package Generation and Staging

Matrix build jobs execute package creation tasks isolated from registry secrets:

| Target | Build Command | Artifact Name | Output Path | Format |
| :--- | :--- | :--- | :--- | :--- |
| **Rust** | `pixi run package-rs` | `package-rust` | `target/package/*.crate` | `.crate` |
| **Python** | `pixi run release-py` | `package-python` | `python/dist/*` | `.whl`, `.tar.gz` |
| **TypeScript** | `pixi run release-ts` | `package-typescript` | `typescript/*.tgz` | `.tgz` |
| **C++** | `pixi run release-cpp` | `package-cpp` | `cpp/dist/*` | `.tar.gz` |
| **C** | `pixi run release-c` | `package-c` | `c/dist/*` | `.tar.gz` |
| **C#** | `pixi run release-csharp` | `package-csharp` | `csharp/dist/*` | `.nupkg` |
| **Go** | `pixi run release-go` | `package-go` | `go/dist/*` | `.tar.gz` |
| **Java** | `pixi run release-java` | `package-java` | `java/dist/*` | `.jar` |
| **R** | `pixi run release-r` | `package-r` | `r/dist/*` | `playtest_*.tar.gz` |

Each job uses `actions/upload-artifact@v4` with `if-no-files-found: error`.

### Phase 2: Registry Publishing from Artifacts

Downstream publishing jobs download only their respective artifact via `actions/download-artifact@v4` and execute registry uploads:

1. **crates.io (`publish-crates`)**:
   - Environment: `crates`
   - Permissions: `id-token: write`
   - Uses `rust-lang/crates-io-auth-action@v1` for OIDC authentication.
   - Runs `pixi run release-rs`.
2. **PyPI (`publish-pypi`)**:
   - Environment: `pypi`
   - Permissions: `id-token: write`
   - Uses `pypa/gh-action-pypi-publish@release/v1` targeting `python/dist/`.
3. **npm (`publish-npm`)**:
   - Environment: `npm`
   - Permissions: `id-token: write`
   - Runs `npm publish *.tgz --provenance --access public` with `NODE_AUTH_TOKEN`.
4. **Go (`publish-go`)**:
   - Permissions: `contents: write`
   - Runs `python3 -m automation.publish_go "$RELEASE_TAG"` from a checkout of the release tag.
   - Ensures the tag `go/vX.Y.Z` exists, creating it at the release commit for a general `vX.Y.Z` release. Go resolves a module in a repository subdirectory only from tags named `<subdirectory>/vX.Y.Z`. An existing `go/vX.Y.Z` on a different commit is accepted when its `go/` directory is identical (an earlier Go-only release already published that version) and fails the job otherwise; the tag is never moved.
   - Requests `https://proxy.golang.org/github.com/!archont561/playtest/go/@v/vX.Y.Z.info` to trigger indexing. Proxy URLs case-encode the module path (`A` becomes `!a`). A proxy that does not confirm the version produces a warning, not a failure.
5. **Conan (`publish-conan`)**:
   - Environment: `conan`
   - Runs `conan create` and `conan upload` for `playtest-cpp` and `playtest-c` from a checkout of the release tag.
   - Skipped with a warning unless `CONAN_REMOTE_URL`, `CONAN_USERNAME`, and `CONAN_PASSWORD` are configured.
6. **NuGet (`publish-nuget`)**:
   - Environment: `nuget`
   - Pushes `csharp/dist/*.nupkg` from the `package-csharp` artifact with `dotnet nuget push --skip-duplicate`.
   - Skipped with a warning unless `NUGET_API_KEY` is configured.
7. **Maven Central (`publish-maven-central`)**:
   - Environment: `maven`
   - Stamps the release version and runs a signed `mvn deploy` from a checkout of the release tag.
   - Skipped with a warning unless the four `MAVEN_CENTRAL_*` and `MAVEN_GPG_*` secrets are configured.
8. **CRAN check (`prepare-cran`) and R-universe (`publish-runiverse`)**:
   - `prepare-cran` runs `R CMD check --as-cran --no-manual` on the `package-r` artifact. Submission to CRAN is the separate, manually confirmed `.github/workflows/cran-publish.yml`.
   - `publish-runiverse` runs after the GitHub Release when `RUNIVERSE_ENABLED` is `true` and waits until R-universe reports the released version.
9. **GitHub Releases (`github-release`)**:
   - Runs after every build and publish job above, including `publish-go`.
   - Downloads all staged artifacts from Phase 1.
   - Generates release notes from Conventional Commits using `convco changelog`.
   - Attaches all package archives to the GitHub Release via `softprops/action-gh-release@v2`.

## Planned Changes

Not implemented. Each item is tracked by the task named, and [Specification 0012](./0012-workspace-crates-and-cli-distribution.md) is the target contract.

| Area | Today | Target | Task |
| :--- | :--- | :--- | :--- |
| Rust crates | `package-rs` and `release-rs` handle `playtest-macros` and `playtest` | Five crates, published in dependency order | `task_050` |
| CLI binaries | None are built | cargo-dist builds one archive per platform, with SHA-256 files, attached to an immutable GitHub Release | `task_054` |
| PyPI | A pure-Python wheel and a source distribution built with setuptools | Platform wheels built by maturin that hold the binary and the Python SDK | `task_055` |
| npm | `release-ts` runs `bun pm pack --dry-run`, which writes no tarball, while `build-typescript` requires `typescript/*.tgz`; `publish-npm` publishes tarballs in alphabetical order | Platform packages and a wrapper generated by cargo-npm, platform packages published first | `task_056` |
| conda | Not published | A package built with `pixi-build-rust` | `task_057` |
| Go | `publish-go` pushes `go/vX.Y.Z` before `github-release` runs | The Go tag is pushed after the release assets exist, because the Go wrapper downloads them | `task_058`, `task_059` |
| Versions | Scoped tags release one target at its own version | One version per release across the CLI and all SDKs | `task_059` |
| Platforms | Every job runs on `ubuntu-latest`; the Pixi workspace declares `linux-64` | A build matrix that covers every supported platform | `task_054` |
