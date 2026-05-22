#!/usr/bin/env bash
# Compila fuentes principales y de test (necesario para el smoke test manual).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
JAVA_HOME="$GRAALVM_HOME" ./mvnw test-compile -q
