# Spec 0004: Isolation and VCR Fixtures

## 1. `tmpdir`
- Wraps `tempfile::TempDir`.
- Path traversal protection: rejects paths attempting to resolve outside the sandbox directory (`..`).
- Cleans up directory automatically on drop.

## 2. `snap`
- Built on `insta::Settings`.
- Automatically names snapshots after test title.
- Preconfigured regex filters replacing dynamic UUIDs (`[UUID]`) and ISO timestamps (`[TIMESTAMP]`).

## 3. `cassette`
- VCR record and replay system for HTTP and callbacks.
- Stores recorded payloads in `tests/cassettes/<test_name>.json`.
- Modes: `Auto` (record if missing, replay if present), `Record` (force live write), `Replay` (strict CI mode), `Passthrough` (bypass cache).
- Saves dirty entries automatically on fixture teardown.

## 4. `network_guard`
- Intercepts outbound TCP/socket calls.
- Enforces explicit allowlists: `network_guard.allow("http://localhost:*")`.
- Panics immediately on accidental external internet egress during unit/integration tests.
