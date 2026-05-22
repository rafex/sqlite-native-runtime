#!/usr/bin/env bash
# Empaqueta el JAR sin instalar en ~/.m2.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

cd "$JAVA_DIR"
JAVA_HOME="$GRAALVM_HOME" ./mvnw package -q
