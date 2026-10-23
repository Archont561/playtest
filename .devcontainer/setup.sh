#!/usr/bin/env bash
# Devcontainer post-create setup for playtest
set -euo pipefail

pixi --version

# Baseline CLI tooling
pixi global install git gh

# The pixi-sandbox CLI, from the Archont561 prefix.dev channel
pixi global install --channel https://prefix.dev/archont561/archont561 --channel conda-forge pixi-sandbox

# The project's own environment, pinned to the lockfile
pixi install --locked

# Setup git hooks
pixi run install-hooks
