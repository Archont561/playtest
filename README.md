# Offline sandbox (orphan branch)

Built 2026-11-13T23:38:45Z from commit `6b2f46b` for platform `linux-64`.
`pixi.lock` sha256 `a58b6f768f76bd68aadfdf3c4d89d864226a24b02e3c71d7e02ec1fc92f2ec27`.

The verified self-bootstrap binary is stored at `.pixi-sandbox/tools/linux-64/pixi-sandbox`. The branch root intentionally contains documentation only.

| env | platform | packed | unpacked | files |
| --- | --- | ---: | ---: | ---: |
| `default` | linux-64 | 647.3 MiB | 2534.8 MiB | 152 |

Cargo dependencies: **305 crates**, 308.7 MiB (loose) from `Cargo.lock` sha256 `dc4c3eec58e0…`; restore materialises them to `.pixi-sandbox/vendor/`. Built with cargo 1.99.1 (5f94df478 2026-08-27); rustc 1.99.1 (58cf8f9c3 2026-10-13).


## Restore on the disconnected machine

```bash
./.pixi-sandbox/tools/linux-64/pixi-sandbox doctor --branch-location . --verify
./.pixi-sandbox/tools/linux-64/pixi-sandbox restore --branch-location . --output-path <project> --force
# then, from <project> with no network, use pixi as the sole entrypoint:
pixi install --frozen --offline
pixi run --frozen -- cargo build --offline
```

Every manifest blob is verified before it is written into the working tree.
