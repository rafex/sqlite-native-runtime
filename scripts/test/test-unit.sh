#!/usr/bin/env bash
# TT-3: tests unitarios Java (JUnit 5 / JaCoCo).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
ETHER_SQLITE_LIB="$ETHER_SQLITE_LIB" JAVA_HOME="$GRAALVM_HOME" ./mvnw test
