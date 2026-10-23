# ADR 0004: Drop Browser Automation and Focus on Backend Testing

## Context
`bun-test-utils` includes Playwright headless browser automation. In JavaScript, browser automation is essential for frontend frameworks. However, in Rust:
- 90%+ of use cases are backends, APIs, microservices, CLIs, and system utilities.
- Chromium binaries require 300MB+ downloads, system graphics libraries, and heavy CI maintenance.
- In-memory web server testing (`tower::ServiceExt::oneshot`) provides instant, deterministic API testing without network sockets.

## Decision
Drop browser automation completely from `playtest` core.

## Consequences
- Blazing-fast compilation times.
- Zero external binary requirements on CI runners or developer machines.
- Focus on bulletproof backend and integration testing.
