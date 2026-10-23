# Spec 0004: Isolation, Determinism, Snapshots, and VCR Fixtures

## Scope

This specification defines portable behavior for the isolation and determinism helpers. Implementations may use language-native libraries and APIs, but their observable behavior must match across all nine Playtest targets as required by [Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md). The Rust types and libraries named below are implementation mappings, not cross-language API requirements.

## 1. `tmpdir` — per-test filesystem isolation

- Create a unique temporary directory for each test or generated property sample.
- Reject path traversal and resolved paths that escape the fixture root.
- Clean up automatically after success or failure; cleanup participates in the test's LIFO teardown contract.
- Do not use the checkout or a real user home as test scratch space.
- Rust mapping: `tempfile::TempDir` with traversal guards.

## 2. `snap` — deterministic snapshots

- Associate snapshots with a stable test/scenario identity.
- Apply configured masking for nondeterministic values, including UUIDs and timestamps; additional masks are explicit.
- A mismatch reports a useful diff. Updating or recording a new expected snapshot must be an explicit developer action, never an implicit CI side effect.
- Rust mapping: `insta::Settings` and filters.

## 3. `cassette` — HTTP and callback record/replay

- Record and replay HTTP requests/responses and explicitly registered callbacks using deterministic cassette keys.
- Persist cassettes under the test project's cassette directory with a documented, stable format.
- Support `Auto` (record when missing, replay when present), `Record` (force live record), `Replay` (fail if no matching cassette), and `Passthrough` (bypass cassette handling).
- Save changed cassettes through fixture teardown; a failed test must not silently lose a completed recording.
- CI uses strict `Replay` mode unless a test explicitly opts out.

## 4. `network_guard` — default-deny external network access

- Block outbound network access by default during guarded tests.
- Allow narrowly scoped, explicit destinations (for example, a local test server); an allowlist must not disable the guard globally.
- Fail the test with the attempted destination when an unapproved connection is made.
- Guard cleanup and state restoration follow the fixture teardown contract.

## 5. `clock` and `seed` — deterministic time and randomness

- `clock` can freeze time and advance it deterministically without changing unrelated tests' clock state.
- `seed` provides deterministic random input for framework generators and reports a replayable seed/case on failure.
- Each test or property sample has isolated deterministic state unless a broader scope is explicitly requested.

## 6. Conformance

The cross-language suite must verify isolation boundaries, traversal rejection, cleanup after failures, stable snapshot masking, cassette replay/miss behavior, default-deny network behavior, deterministic clock advancement, and seed replay for every target. A target does not claim full core parity if one of these required helpers is missing or behaves differently.
