#!/usr/bin/env bash
# Limpia artefactos de Rust y Java.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

echo "→ cargo clean"
cd "$RUST_DIR"
RUSTC="$RUSTC" "$CARGO" clean

echo "→ mvn clean"
cd "$JAVA_DIR"
JAVA_HOME="$GRAALVM_HOME" ./mvnw clean -q
