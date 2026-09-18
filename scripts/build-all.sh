#!/bin/bash
# RIVAREN build-all: compila UNA VEZ para Linux + Windows + Mac en paralelo.
# Uso: ./scripts/build-all.sh [--release|--dist|--mobile]
set -euo pipefail
PROFILE_ARGS="--release"
if [[ "${1:-}" == "--dist" ]]; then PROFILE_ARGS="--profile dist"; fi
if [[ "${1:-}" == "--mobile" ]]; then PROFILE_ARGS="--profile mobile"; fi
echo "◆ RIVAREN build-all $PROFILE_ARGS (paralelo, todos los hilos)"
cargo xtask build-all "$PROFILE_ARGS"
