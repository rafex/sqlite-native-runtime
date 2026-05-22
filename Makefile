# ─────────────────────────────────────────────────────────────────────────────
# Makefile — sqlite-native-runtime
#
# Solo define el grafo de dependencias entre targets y delega toda la lógica
# a los scripts en scripts/*/.
#
# Configuración (toolchain, rutas, motor de contenedor):
#   → scripts/commons/commons.mk
#
# Para sobreescribir una variable:
#   make test-unit GRAALVM_HOME=/otra/ruta
#   make container-test CONTAINER_ENGINE=docker
# ─────────────────────────────────────────────────────────────────────────────

include scripts/commons/commons.mk
include scripts/build/build.mk
include scripts/test/test.mk
include scripts/container/container.mk
include scripts/dist/dist.mk

.PHONY: all build test

all: build

build: build-rust build-java

test: build smoke
