#!/usr/bin/env bash
# Cobertura Rust con cargo-llvm-cov.
# Requiere: cargo install cargo-llvm-cov
# Reporte HTML: sources/rust/target/llvm-cov/html/index.html
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$RUST_DIR"
PATH="${CARGO_BIN}:${PATH}" RUSTC="$RUSTC" "$CARGO" llvm-cov --lib --html

echo "Reporte de cobertura Rust: ${RUST_DIR}/target/llvm-cov/html/index.html"
