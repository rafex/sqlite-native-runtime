#!/usr/bin/env bash
# Ejecuta SmokeTest manualmente con la .dylib compilada.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
SNR_LIB="$SNR_LIB" \
  "${GRAALVM_HOME}/bin/java" \
    --enable-native-access=ALL-UNNAMED \
    -cp target/classes:target/test-classes \
    -ea \
    mx.rafex.sqlite.SmokeTest
