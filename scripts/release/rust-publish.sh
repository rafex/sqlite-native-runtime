#!/usr/bin/env bash
# rust-publish — Lee VERSIONS y lanza build-rust.yml vía workflow_dispatch.
#
# Compila y publica en GHCR las librerías nativas Rust (.so) para ambas
# arquitecturas (amd64, arm64). Debe ejecutarse ANTES de just release-dispatch.
#
# Uso:
#   just rust-publish
#   scripts/release/rust-publish.sh
#
# Variables opcionales (sobrescriben VERSIONS):
#   FFM_VERSION=0.2.0 just rust-publish    → solo publica FFM en esa versión
#   JNI_VERSION=0.2.0 just rust-publish    → solo publica JNI en esa versión
#
# Requisitos:
#   gh CLI autenticado con permisos packages:write y actions:write.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

VERSIONS_FILE="${ROOT}/VERSIONS"

# ── Leer versiones desde VERSIONS (las variables de entorno tienen prioridad) ─
if [[ ! -f "$VERSIONS_FILE" ]]; then
  echo "Error: archivo VERSIONS no encontrado en ${ROOT}" >&2
  exit 1
fi

_FFM="$(grep '^FFM_VERSION=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"
_JNI="$(grep '^JNI_VERSION=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"

FFM_VERSION="${FFM_VERSION:-${_FFM}}"
JNI_VERSION="${JNI_VERSION:-${_JNI}}"

if [[ -z "$FFM_VERSION" && -z "$JNI_VERSION" ]]; then
  echo "Error: FFM_VERSION y JNI_VERSION están vacíos en VERSIONS." >&2
  exit 1
fi

echo ""
echo "┌──────────────────────────────────────────────┐"
echo "│  rust-publish — publicar librerías en GHCR  │"
echo "└──────────────────────────────────────────────┘"
echo ""
echo "  FFM_VERSION : ${FFM_VERSION}"
echo "  JNI_VERSION : ${JNI_VERSION}"
echo ""

# ── Lanzar build-rust.yml ─────────────────────────────────────────────────────
REPO="$(git -C "$ROOT" remote get-url origin \
  | sed 's|https://github.com/||;s|git@github.com:||;s|\.git$||')"

echo "  → gh workflow run build-rust.yml --repo ${REPO}"
echo ""

gh workflow run build-rust.yml \
  --repo "$REPO" \
  --ref main \
  --field "ffm_version=${FFM_VERSION}" \
  --field "jni_version=${JNI_VERSION}"

echo "✅  build-rust.yml lanzado."
echo ""
echo "    Monitorea el progreso en:"
echo "    https://github.com/${REPO}/actions/workflows/build-rust.yml"
echo ""
echo "    Cuando termine, lanza el release con:"
echo "      just release-dispatch v0.X.Y"
echo ""
