# ADR 0006: Crates.io Trusted Publishing via GitHub Actions OIDC

## Context
Long-lived API tokens stored in GitHub Actions secrets are prone to leakage and require manual secret rotation. Crates.io supports OpenID Connect (OIDC) Trusted Publishing.

## Decision
1. Reserve crate names on crates.io via `reserve_crate.sh` at `v0.0.0`.
2. Configure Trusted Publishing on crates.io pointing to:
   - Repository: `Archont561/playtest`
   - Workflow: `release.yml`
   - Environment: `crates`
3. Configure `.github/workflows/release.yml` with `permissions: id-token: write` and `environment: crates`.
4. Publish releases via tag pushes (`v*`) using native Cargo OIDC negotiation.

## Consequences
- Zero long-lived tokens required in repository secrets.
- Secure, auditable, and automated releases directly from Git tags.
