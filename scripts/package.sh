#!/bin/bash
# Empaqueta binarios dist/ por plataforma.
set -euo pipefail
mkdir -p dist
for T in x86_64-unknown-linux-gnu x86_64-pc-windows-gnu x86_64-apple-darwin aarch64-apple-darwin; do
  BIN="target/$T/dist/rivaren"
  [[ "$T" == *windows* ]] && BIN="$BIN.exe"
  if [[ -f "$BIN" ]]; then
    echo "◆ $T"
    tar -czf "dist/rivaren-$T.tar.gz" -C "target/$T/dist" "$(basename "$BIN")"
  else
    echo "  (skip $T: sin binario, compila primero con cargo xtask build-all --dist)"
  fi
done
ls -la dist/ || true
