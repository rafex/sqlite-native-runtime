#!/usr/bin/env bash
# Construye la imagen y ejecuta los tests en contenedor.
# Uso: container-test.sh <rust|java|all>
#
# Podman (local): CONTAINER_ENGINE=podman  (default)
# Docker (CI):    CONTAINER_ENGINE=docker
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

TARGET="${1:-all}"

run_rust() {
  echo "▶ ${CONTAINER_ENGINE} build → snr-rust-test"
  "$CONTAINER_ENGINE" build \
    -f "${CONTAINERS_DIR}/Dockerfile.rust-test" \
    -t snr-rust-test \
    "$ROOT"
  echo "▶ ${CONTAINER_ENGINE} run → snr-rust-test"
  "$CONTAINER_ENGINE" run --rm snr-rust-test
}

run_java() {
  echo "▶ ${CONTAINER_ENGINE} build → snr-java-test"
  "$CONTAINER_ENGINE" build \
    -f "${CONTAINERS_DIR}/Dockerfile.java-test" \
    -t snr-java-test \
    "$ROOT"
  echo "▶ ${CONTAINER_ENGINE} run → snr-java-test"
  "$CONTAINER_ENGINE" run --rm snr-java-test
}

case "$TARGET" in
  rust) run_rust ;;
  java) run_java ;;
  all)  run_rust; run_java ;;
  *)
    echo "Uso: $0 <rust|java|all>" >&2
    exit 1
    ;;
esac
