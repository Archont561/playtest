# Development Workflow

## Tooling Commands

All operations run through Pixi to ensure 100% reproducible environments across developer machines, airgapped sandboxes, and CI runners:

| Goal | Command | Description |
|:--|:--|:--|
| **Task Claim** | `pixi run bunx backlog claim <task-id>` | Claim assigned task before making commits |
| **Lint Gate** | `pixi run lint` | Runs `fmt-check`, `clippy -D warnings`, `lint-toml`, and `actionlint` |
| **Fast Tests** | `pixi run test` | Executes unit and integration suites via `cargo-nextest` |
| **Documentation Tests** | `pixi run test-doc` | Executes code snippets in documentation and docstrings |
| **Code Coverage** | `pixi run coverage` | Runs `cargo-llvm-cov nextest` and outputs `coverage/lcov.info` |
| **Security Audit** | `pixi run audit` | Verifies `Cargo.lock` against the RustSec database |
| **Task Completion**| `pixi run bunx backlog task move <id> done` | Marks finished task as complete |

## The Working Loop

1. **Pick & Claim Task**:
   - Check open tasks: `pixi run bunx backlog task list`
   - Claim task: `pixi run bunx backlog claim <task-id>`
2. **Red (TDD)**:
   - Add a failing integration test in the crate's `tests/` directory mirroring `src/`.
   - Run `pixi run test` and verify failure.
3. **Green**:
   - Implement the minimum required code in `src/`.
   - Verify `pixi run test` passes.
4. **Refactor**:
   - Clean up code according to Clean Code Principles while keeping tests green.
   - Run `pixi run lint` (clippy + formatting).
5. **Verify & Commit**:
   - Run `pixi run coverage` and verify no drop below 90% threshold.
   - Commit with Conventional Commit message: `feat(scope): message`.
   - Pre-commit and commit-msg hooks run automatically via `lefthook`.

## Definition of Done

1. All tests passing under `cargo-nextest`.
2. Clean `pixi run lint` with 0 warnings across all targets and features.
3. Code coverage at or above 90% (`coverage/lcov.info`).
4. Any public API change reflected in `README.md` and matching specification under `backlog/docs/specs/`.
5. Any architectural change captured in a new ADR under `backlog/docs/adr/`.
