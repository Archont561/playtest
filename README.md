# Offline sandbox (orphan branch)

Built 2026-10-23T14:23:15Z from commit `9962f08` for platform `linux-64`.
`pixi.lock` sha256 `b404e7d00c3f217bbe90855ef196ee1ba0d4a3f0eaf2017da51189543a35841f`.

The verified self-bootstrap binary is stored at `.pixi-sandbox/tools/linux-64/pixi-sandbox`. The branch root intentionally contains documentation only.

| env | platform | packed | unpacked | files |
| --- | --- | ---: | ---: | ---: |
| `default` | linux-64 | 657.6 MiB | 2586.3 MiB | 126 |

Cargo dependencies: **22 crates**, 22.6 MiB (loose) from `Cargo.lock` sha256 `2f74546f2f17…`; restore materialises them to `.pixi-sandbox/vendor/`. Built with cargo 1.99.0 (5f94df478 2026-08-27); rustc 1.99.0 (b940084d7 2026-09-28).


## Restore on the disconnected machine

```bash
./.pixi-sandbox/tools/linux-64/pixi-sandbox doctor --branch-location . --verify
./.pixi-sandbox/tools/linux-64/pixi-sandbox restore --branch-location . --output-path <project> --force
# then, from <project> with no network, use pixi as the sole entrypoint:
pixi install --frozen --offline
pixi run --frozen -- cargo build --offline
```

Every manifest blob is verified before it is written into the working tree.
