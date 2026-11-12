# Curated presets

This directory is the distribution. Each tool keeps its native config format, and each preset is standalone.

```text
presets/
├── alint/{recommended,strict,pedantic}.yml
├── vale/{recommended,strict,pedantic}.ini
├── vale/styles/<preset>/PCC/
├── ast-grep/<preset>/{sgconfig.yml,rules/,rule-tests/}
└── semgrep/{recommended,strict,pedantic}.yml
└── semgrep/rules/ and semgrep/fixtures/<preset>/
```

There is no `catalogue.yml`. The reviewed rule register lives in `backlog/tasks/`; see [`docs/concepts/rule-register.md`](../docs/concepts/rule-register.md). Change a native config and its backlog task in the same review. No generator writes these files.

Use one preset per tool:

```sh
alint check . -c presets/alint/strict.yml
vale --no-global --config=presets/vale/strict.ini docs/
ast-grep --config presets/ast-grep/strict/sgconfig.yml scan .
semgrep scan --config presets/semgrep/strict.yml .
```

The `fixtures/` and `rule-tests/` trees are part of the tested distribution. Keep them when copying a whole preset. A consumer that needs a smaller subset can copy one config and add local overrides.

## Consumer installation

A consumer repository vendors only `presets/`. Keep the checkout at `vendor/clean-code-presets` and point each native CLI at one config; see [`docs/guides/adopting-presets.md`](../docs/guides/adopting-presets.md). Pin a release tag after `PCC-12` or a full commit SHA today. The development harness in this repository is not part of the distribution.
