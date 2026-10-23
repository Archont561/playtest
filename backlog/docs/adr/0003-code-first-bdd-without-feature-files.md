# ADR 0003: Code-First Fluent BDD Without `.feature` Files

## Context
Cucumber-style BDD introduces significant friction for software engineers:
- Separate `.feature` plain text files.
- Regex-matched string step definitions.
- Untyped world state maps without compile-time guarantees.
- Inability to feed property-based arbitrary test generators into scenario steps.

## Decision
Implement a code-first, type-safe fluent scenario pipeline:
- `test_scenario!` macro using method chaining: `.given().when().then().and()`.
- Steps pass typed state forward: `given` returns `StateA`, `when` consumes `StateA` and returns `StateB`.
- Directly compatible with `#[prop(...)]` attribute macros.

## Consequences
- 100% compile-time type safety with IDE autocomplete and refactoring.
- Zero runtime regex overhead or glue code maintenance.
