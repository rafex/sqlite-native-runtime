#!/usr/bin/env bash
# scripts/commons/env.sh — variables comunes para todos los shell scripts.
#
# Uso: source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../../scripts/commons/env.sh"
# O desde cualquier script en scripts/*/:
#   source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"
#
# Las variables exportadas por Make tienen prioridad sobre los defaults aquí:
# la sintaxis ${VAR:-default} deja intacto el valor si la variable ya existe.

# Raíz del repositorio (absoluta, independientemente de dónde se llame el script)
_COMMONS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${_COMMONS_DIR}/../.." && pwd)"

# ── Toolchains ────────────────────────────────────────────────────────────────
# Cuando los scripts se invocan desde Make, GRAALVM_HOME y CARGO_TOOLCHAIN ya
# vienen exportados con el valor correcto (`:=` en commons.mk).
# Cuando se invocan directamente desde el terminal, usamos el default del proyecto.
# IMPORTANTE: usamos el default explícito del proyecto, no el GRAALVM_HOME del
# entorno del usuario (que puede apuntar a otra versión de GraalVM).
SNR_GRAALVM_HOME="${SNR_GRAALVM_HOME:-/Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home}"
GRAALVM_HOME="${GRAALVM_HOME:-${SNR_GRAALVM_HOME}}"

SNR_CARGO_TOOLCHAIN="${SNR_CARGO_TOOLCHAIN:-stable-aarch64-apple-darwin}"
CARGO_TOOLCHAIN="${CARGO_TOOLCHAIN:-${SNR_CARGO_TOOLCHAIN}}"

CONTAINER_ENGINE="${CONTAINER_ENGINE:-podman}"
GLIBC_MIN="${GLIBC_MIN:-2.17}"

# ── Versiones de librerías nativas (desde VERSIONS en la raíz del repo) ───────
# Las variables de entorno tienen prioridad sobre el archivo (permiten override).
if [[ -f "${ROOT}/VERSIONS" ]]; then
  _FFM_FILE="$(grep '^FFM_VERSION=' "${ROOT}/VERSIONS" | cut -d= -f2 | tr -d '[:space:]')"
  _JNI_FILE="$(grep '^JNI_VERSION=' "${ROOT}/VERSIONS" | cut -d= -f2 | tr -d '[:space:]')"
  FFM_VERSION="${FFM_VERSION:-${_FFM_FILE}}"
  JNI_VERSION="${JNI_VERSION:-${_JNI_FILE}}"
fi

# ── Rutas derivadas ───────────────────────────────────────────────────────────
JAVA_DIR="${ROOT}/sources/java/ether-sqlite-ffm-runtime"
RUST_DIR="${ROOT}/sources/rust"
CONTAINERS_DIR="${ROOT}/containers"

CARGO="${HOME}/.rustup/toolchains/${CARGO_TOOLCHAIN}/bin/cargo"
RUSTC="${HOME}/.rustup/toolchains/${CARGO_TOOLCHAIN}/bin/rustc"
CARGO_BIN="${HOME}/.rustup/toolchains/${CARGO_TOOLCHAIN}/bin"

# Librerías nativas compiladas por el workspace Rust (target compartido en sources/rust/target/)
ETHER_SQLITE_FFM_LIB="${ROOT}/sources/rust/target/release/libether_sqlite_ffm_runtime.dylib"
ETHER_SQLITE_JNI_LIB="${ROOT}/sources/rust/target/release/libether_sqlite_jni_runtime.dylib"
# Alias de compatibilidad para scripts que usen ETHER_SQLITE_LIB (apunta a FFM por defecto)
ETHER_SQLITE_LIB="${ETHER_SQLITE_FFM_LIB}"

# ── Exportar todo ─────────────────────────────────────────────────────────────
export ROOT
export GRAALVM_HOME CARGO_TOOLCHAIN CONTAINER_ENGINE GLIBC_MIN
export JAVA_DIR RUST_DIR CONTAINERS_DIR
export CARGO RUSTC CARGO_BIN
export ETHER_SQLITE_FFM_LIB ETHER_SQLITE_JNI_LIB ETHER_SQLITE_LIB
export FFM_VERSION JNI_VERSION
