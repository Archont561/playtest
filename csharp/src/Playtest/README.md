# Playtest for C# / .NET

Playwright-style testing framework for .NET with dependency-injected fixtures, strict LIFO teardown, property-based testing, and fluent BDD scenarios.

## Installation
```bash
dotnet add package Playtest
```

## Quick Start
```csharp
using Playtest;

Assert.Equal("0.1.0", Playtest.Version);
```
