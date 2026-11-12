# Specification 0006: Two-Phase Release and Trusted Publishing

## Overview
Specifies the packaging standards, artifact storage schemas, OpenID Connect (OIDC) authentication flows, and registry publication rules implemented in `.github/workflows/release.yml` across the CLI-first five-crate workspace and multi-channel distribution architecture ([ADR 0006](../adr/0006-crates-io-trusted-publishing.md), [ADR 0008](../adr/0008-two-phase-artifact-release-pipeline.md), [ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md), [ADR 0016](../adr/0016-cli-binary-distribution-channels.md), [Spec 0012](./0012-workspace-crates-and-cli-distribution.md)).

## Two-Phase Pipeline Architecture

### Phase 1: Package Generation and Staging

Phase 1 build jobs execute isolated from registry credentials and stage immutable workflow artifacts from the unified `vX.Y.Z` tag after `gate` verifies version parity across every manifest, `VERSION` constant, and embedded SDK:

| Target | Job(s) | Artifact Name(s) | Output Path | Format |
| :--- | :--- | :--- | :--- | :--- |
| **CLI Binaries** | `build-cli` (5-platform matrix) + `stage-cli-release-assets` | `cli-bin-<target>`, `cli-release-archives` | `release-assets/cli/*` | `.tar.gz`, `.zip`, `.sha256`, `sha256.sum` |
| **Rust Crates** | `build-rust` (`pixi run --frozen package-rs`) | `package-rust` | `target/package/*.crate` | `.crate` (all 5 crates) |
| **Python** | `build-python` (5-platform `maturin` matrix + sdist) | `package-python-<target>` | `python/dist/*` | `.whl`, `.tar.gz` |
| **TypeScript / npm** | `build-typescript` (`automation.npm_packages` + `cargo-npm`) | `package-typescript` | `npm-dist/*.tgz` | 5 platform `.tgz` + wrapper `.tgz` |
| **conda** | `build-conda` (5-platform `pixi build --path conda` matrix) | `package-conda-<subdir>` | `conda-dist/*.conda` | `.conda` |
| **C++** | `build-cpp` (`pixi run --frozen -e cpp sdk-release`) | `package-cpp` | `cpp/dist/*` | `.tar.gz` |
| **C** | `build-c` (`pixi run --frozen -e c sdk-release`) | `package-c` | `c/dist/*` | `.tar.gz` |
| **C#** | `build-csharp` (`pixi run --frozen -e csharp sdk-release`) | `package-csharp` | `csharp/dist/*` | `.nupkg` |
| **Go** | `build-go` (`pixi run --frozen -e go sdk-release`) | `package-go` | `go/dist/*` | `.tar.gz` |
| **Java** | `build-java` (`pixi run --frozen -e java sdk-release`) | `package-java` | `java/dist/*` | `.jar` |
| **R** | `build-r` (`pixi run --frozen -e r sdk-release`) | `package-r` | `r/dist/*` | `playtest_*.tar.gz` |

Each job uses `actions/upload-artifact` with `if-no-files-found: error`.

### Phase 2: Registry Publishing and Release Publication

Downstream jobs upload validated Phase 1 artifacts using scoped OIDC Trusted Publishing environments or environment secrets:

1. **crates.io (`publish-crates`)**:
   - Trusted Publishing binding on crates.io for all five reserved crates (`playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, `playtest-cli`): repository `Archont561/playtest`, workflow `release.yml`, environment `crates`.
   - Environment: `crates`
   - Permissions: `id-token: write`, `contents: read`
   - Uses `rust-lang/crates-io-auth-action@v1` (`id: auth`) to exchange the GitHub OIDC JWT for a temporary token and passes `CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}` to `pixi run --frozen publish-rs`.
   - Publishes in topological dependency order: `playtest-macros` $\rightarrow$ `playtest-core` $\rightarrow$ `playtest-assets` $\rightarrow$ `playtest` $\rightarrow$ `playtest-cli`.
2. **PyPI (`publish-pypi`)**:
   - Environment: `pypi`
   - Permissions: `id-token: write`, `contents: read`
   - Downloads `package-python-*` into `python/dist/` and uploads the 5 platform wheels and sdist via `pypa/gh-action-pypi-publish@release/v1`.
3. **npm (`publish-npm`)**:
   - Environment: `npm`
   - Permissions: `id-token: write`, `contents: read`
   - Downloads `package-typescript` and runs `python3 -m automation.publish_registries npm` (`npm publish --provenance --access public`), publishing all five `@archont561/playtest-<os>-<cpu>` platform packages strictly before `@archont561/playtest`.
4. **conda (`publish-conda`)**:
   - Environment: `conda`
   - Downloads `package-conda-*` and uploads `.conda` archives to `https://prefix.dev/archont561/archont561` via `python3 -m automation.publish_registries conda` when `PREFIX_DEV_TOKEN` is configured.
5. **Conan (`publish-conan`)**:
   - Environment: `conan`
   - Runs `conan create` and `conan upload` for `playtest-cpp` and `playtest-c` when `CONAN_REMOTE_URL`, `CONAN_USERNAME`, and `CONAN_PASSWORD` are configured.
6. **NuGet (`publish-nuget`)**:
   - Environment: `nuget`
   - Pushes `csharp/dist/*.nupkg` from `package-csharp` with `dotnet nuget push --skip-duplicate` when `NUGET_API_KEY` is configured.
7. **Maven Central (`publish-maven-central`)**:
   - Environment: `maven`
   - Stamps the release version and runs a signed `mvn deploy` when the `MAVEN_CENTRAL_*` and `MAVEN_GPG_*` secrets are configured.
8. **CRAN check (`prepare-cran`), GitHub Releases (`github-release`), Go (`publish-go`), and R-universe (`publish-runiverse`)**:
   - `prepare-cran` runs `R CMD check --as-cran --no-manual` on `package-r`. Manual CRAN submission via `.github/workflows/cran-publish.yml` verifies `vX.Y.Z/sha256.sum` exists before submitting.
   - `github-release` publishes the immutable GitHub Release `vX.Y.Z` with all CLI binary archives, per-archive `.sha256` files, combined `sha256.sum`, and SDK artifacts attached in one publication step via `softprops/action-gh-release@v2`.
   - `publish-go` runs **after** `github-release`, verifies over HTTPS that `vX.Y.Z/sha256.sum` is live, pushes `go/vX.Y.Z` at the release commit via `python3 -m automation.publish_go`, and triggers `proxy.golang.org` indexing.
   - `publish-runiverse` runs after `github-release` when `RUNIVERSE_ENABLED` is `true` and waits until R-universe reports the released version.

