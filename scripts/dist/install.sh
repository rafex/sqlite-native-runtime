#!/usr/bin/env bash
# Genera el JAR e instala en el repositorio Maven local (~/.m2).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
JAVA_HOME="$GRAALVM_HOME" ./mvnw install -q
