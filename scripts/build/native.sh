#!/usr/bin/env bash
# Compila SmokeTest como ejecutable GraalVM Native Image.
# Requiere: make build primero (genera la .dylib y las clases Java).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
ETHER_SQLITE_LIB="$ETHER_SQLITE_LIB" \
  JAVA_HOME="$GRAALVM_HOME" \
  ./mvnw -Pnative package native:compile -q

echo "Binario generado: ${JAVA_DIR}/target/ether-sqlite"
echo "Ejecutar con: ETHER_SQLITE_LIB=${ETHER_SQLITE_LIB} ${JAVA_DIR}/target/ether-sqlite"
