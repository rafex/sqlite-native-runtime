#!/usr/bin/env bash
# Cross-compila la librería nativa para Linux con cargo-zigbuild.
# Uso: cross.sh <amd64|arm64|all>
#
# Requisitos (una sola vez):
#   brew install zig
#   cargo install cargo-zigbuild
#   rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

PLATFORM="${1:-all}"

build_target() {
  local triple="$1"
  echo "→ zigbuild --workspace --target ${triple}.${GLIBC_MIN}"
  cd "$RUST_DIR"
  PATH="${CARGO_BIN}:${PATH}" \
    "$CARGO" zigbuild --workspace --release --target "${triple}.${GLIBC_MIN}"
  echo "Librerías:"
  echo "  ${RUST_DIR}/target/${triple}/release/libether_sqlite_ffm_runtime.so"
  echo "  ${RUST_DIR}/target/${triple}/release/libether_sqlite_jni_runtime.so"
}

case "$PLATFORM" in
  amd64) build_target "x86_64-unknown-linux-gnu"  ;;
  arm64) build_target "aarch64-unknown-linux-gnu" ;;
  all)
    build_target "x86_64-unknown-linux-gnu"
    build_target "aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "Uso: $0 <amd64|arm64|all>" >&2
    exit 1
    ;;
esac
