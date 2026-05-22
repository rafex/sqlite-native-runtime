# scripts/commons/commons.mk — variables compartidas para todos los sub-makefiles.
# Incluido automáticamente desde el Makefile raíz con: include scripts/commons/commons.mk

# ── Toolchains ────────────────────────────────────────────────────────────────
# := ignora la variable de entorno del shell y siempre usa el valor del proyecto.
# Para sobreescribir desde la línea de comandos:
#   make test-unit GRAALVM_HOME=/otra/ruta/graalvm
GRAALVM_HOME    := /Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home
CARGO_TOOLCHAIN := stable-aarch64-apple-darwin

# ?= permite sobreescribir desde el entorno o la línea de comandos
CONTAINER_ENGINE ?= podman
GLIBC_MIN        ?= 2.17

# ── Rutas del proyecto ────────────────────────────────────────────────────────
JAVA_DIR      := sqlite-native-runtime/java
RUST_DIR      := sqlite-native-runtime/rust
CONTAINERS_DIR := containers
SCRIPTS_DIR   := scripts

# ── Exportar a los scripts de shell invocados desde los targets ───────────────
export GRAALVM_HOME
export CARGO_TOOLCHAIN
export CONTAINER_ENGINE
export GLIBC_MIN
export JAVA_DIR
export RUST_DIR
export CONTAINERS_DIR
export SCRIPTS_DIR
