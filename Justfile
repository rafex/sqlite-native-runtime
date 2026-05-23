# ─────────────────────────────────────────────────────────────────────────────
# Justfile — sqlite-native-runtime
#
# Solo define el grafo de dependencias entre recetas y delega toda la lógica
# a los scripts en scripts/*/.
#
# Configuración (toolchain, rutas, motor de contenedor):
#   → scripts/commons/commons.just
#
# Para sobreescribir una variable:
#   just test-unit GRAALVM_HOME=/otra/ruta
#   just container-test CONTAINER_ENGINE=docker
# ─────────────────────────────────────────────────────────────────────────────

import 'scripts/commons/commons.just'
import 'scripts/build/build.just'
import 'scripts/test/test.just'
import 'scripts/container/container.just'
import 'scripts/dist/dist.just'
import 'scripts/release/release.just'

default: build

build: build-rust build-java

test: build smoke
