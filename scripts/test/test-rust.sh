#!/usr/bin/env bash
# TT-1: tests unitarios Rust (#[cfg(test)] en cada módulo).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$RUST_DIR"
PATH="${CARGO_BIN}:${PATH}" RUSTC="$RUSTC" "$CARGO" test --lib
