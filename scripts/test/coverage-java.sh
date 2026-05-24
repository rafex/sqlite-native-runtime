#!/usr/bin/env bash
# Ejecuta tests + verifica cobertura JaCoCo (LINE >= 0.99).
# Reporte HTML: sources/java/ether-sqlite-ffm-runtime/target/site/jacoco/index.html
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
ETHER_SQLITE_LIB="$ETHER_SQLITE_LIB" JAVA_HOME="$GRAALVM_HOME" ./mvnw verify

echo "Reporte de cobertura: ${JAVA_DIR}/target/site/jacoco/index.html"
