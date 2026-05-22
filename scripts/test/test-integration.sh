#!/usr/bin/env bash
# TT-3i: tests de integración Java (@Tag("integration")).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
SNR_LIB="$SNR_LIB" JAVA_HOME="$GRAALVM_HOME" ./mvnw test -Pintegration
