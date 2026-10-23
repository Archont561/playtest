# Specification 0006: Two-Phase Release and Trusted Publishing

## Overview
Specifies the packaging standards, artifact storage schemas, OpenID Connect (OIDC) authentication flows, and registry publication rules implemented in `.github/workflows/release.yml`.

## Two-Phase Pipeline Architecture

### Phase 1: Package Generation and Staging

Matrix build jobs execute package creation tasks isolated from registry secrets:

| Target | Build Command | Artifact Name | Output Path | Format |
| :--- | :--- | :--- | :--- | :--- |
| **Rust** | `pixi run package-rs` | `package-rust` | `target/package/*.crate` | `.crate` |
| **Python** | `pixi run release-py` | `package-python` | `python/dist/*` | `.whl`, `.tar.gz` |
| **TypeScript** | `pixi run release-ts` | `package-typescript` | `typescript/*.tgz` | `.tgz` |
| **C++** | `pixi run release-cpp` | `package-cpp` | `cpp/dist/*` | `.tar.gz` |
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
   - Pings `https://proxy.golang.org/github.com/Archont561/playtest/go/@v/${TAG}.info` to trigger indexing.
5. **GitHub Releases (`github-release`)**:
   - Downloads all staged artifacts from Phase 1.
   - Generates release notes from Conventional Commits using `convco changelog`.
   - Attaches all package archives to the GitHub Release via `softprops/action-gh-release@v2`.
