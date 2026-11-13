# Playtest for TypeScript

Polyglot test framework with the batteries included: dependency-injected fixtures, strict
LIFO teardown, property-based testing, and code-first BDD for TypeScript.

## Installation
```bash
bun add -d @archont561/playtest
```

## Quick Start
```typescript
import { VERSION } from "@archont561/playtest";

console.log(VERSION); // 0.1.0
```

## Status

This package currently exposes the version contract and is wired into the shared CLI's
mirror and release pipeline. The fixture engine, code-first BDD, and property-based testing
land through [Milestone M5](https://github.com/Archont561/playtest/blob/main/backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md).
The README tracks the same contract as the other eight targets; it does not promise more
than has shipped.
