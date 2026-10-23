# ADR 0008: Two-Phase Artifact-Driven Release Pipeline

## Status
Accepted

## Context
Deploying multi-language packages directly from compilation steps within single monolithic CI jobs creates vulnerability to partial failures, network timeouts, and lack of reproducible audit trails. Furthermore, modern package registries (crates.io, PyPI, npm) encourage or require OpenIDC (OIDC) trusted publishing environments where credentials should only be exposed to jobs that upload already-built and validated artifacts.

## Decision
We decoupled package generation from registry publishing in `.github/workflows/release.yml` into a two-phase architecture:

1. **Phase 1: Build & Stage Artifacts (Parallel Matrix Jobs)**:
   - Separate build jobs (`build-rust`, `build-python`, `build-typescript`, `build-cpp`, `build-go`, `build-java`, `build-r`) compile source trees, run lints/tests, package `.crate`, `.whl`, `.tgz`, `.jar`, and `.tar.gz` distributions, and upload them to GitHub Actions Artifacts using `actions/upload-artifact@v4`.
2. **Phase 2: Publish from Verified Artifacts**:
   - Independent downstream publish jobs (`publish-crates`, `publish-pypi`, `publish-npm`, `publish-go`, `publish-java`) execute in isolated GitHub environments (`crates`, `pypi`, `npm`, `maven`) with minimal permissions (`id-token: write`).
   - Downstream jobs download only the specific built artifact using `actions/download-artifact@v4` and dispatch to the remote registry via trusted OIDC tokens or provenance-enabled tools.
   - The final `github-release` job downloads all artifacts, runs `convco` to extract conventional changelog release notes, and attaches all package archives to the GitHub Release.

## Consequences
### Positive
- Strict separation of build logic from publishing credentials.
- Zero risk of releasing partially built packages: publishing only proceeds if all required build matrix jobs pass.
- Artifacts can be inspected or re-published without re-running compilation steps.

### Negative
- Requires GitHub Actions artifact storage and additional job coordination steps.
