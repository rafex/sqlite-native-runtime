#!/usr/bin/env bash
# TT-2: FFI contract tests (tests/ffi_contract.rs).
# Requiere crate-type = ["cdylib", "staticlib", "rlib"] para linkear el test binary.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$RUST_DIR"
PATH="${CARGO_BIN}:${PATH}" RUSTC="$RUSTC" "$CARGO" test --test ffi_contract
