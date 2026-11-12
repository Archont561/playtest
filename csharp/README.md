# Playtest for C# / .NET

Polyglot test framework with the batteries included: dependency-injected fixtures, strict LIFO teardown, property-based testing, and code-first BDD for .NET.

## Installation
```bash
dotnet add package Playtest
```

## Quick Start
```csharp
using Playtest;

Assert.Equal("0.1.0", Playtest.Version);
```
