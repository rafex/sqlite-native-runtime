#!/usr/bin/env bash
# rust-publish — Publicación manual de librerías Rust independientes.
#
# Lee RUST= de VERSIONS y dispara build-rust.yml con esa versión
# (tanto FFM como JNI simultáneamente).
#
# Útil cuando solo cambia el código Rust (sin cambios en la API Java):
#   1. Actualiza VERSIONS → RUST=v0.2.0
#   2. just rust-publish
#   3. just release v1.0.1
#
# Uso:
#   just rust-publish
#   scripts/release/rust-publish.sh
#
# Variables opcionales (sobrescriben VERSIONS):
#   RUST_VERSION=v0.2.0 just rust-publish
#
# Requisitos:
#   gh CLI autenticado con permisos packages:write y actions:write.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

VERSIONS_FILE="${ROOT}/VERSIONS"

# ── Leer versión Rust desde VERSIONS ─────────────────────────────────────────
if [[ ! -f "$VERSIONS_FILE" ]]; then
  echo "Error: archivo VERSIONS no encontrado en ${ROOT}" >&2
  exit 1
fi

_RUST="$(grep '^RUST=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"
RUST_VERSION="${RUST_VERSION:-${_RUST}}"

if [[ -z "$RUST_VERSION" ]]; then
  echo "Error: RUST no definido en VERSIONS o en la variable RUST_VERSION." >&2
  exit 1
fi

# Strips 'v' prefix for build-rust.yml (espera semver sin 'v')
RUST_V="${RUST_VERSION#v}"

echo ""
echo "┌──────────────────────────────────────────────────────────┐"
echo "│  rust-publish — publicar librerías Rust en GHCR         │"
echo "└──────────────────────────────────────────────────────────┘"
echo ""
echo "  RUST = ${RUST_VERSION}  (ffm_version=${RUST_V}, jni_version=${RUST_V})"
echo ""

# ── Lanzar build-rust.yml ─────────────────────────────────────────────────────
REPO="$(git -C "$ROOT" remote get-url origin \
  | sed 's|https://github.com/||;s|git@github.com:||;s|\.git$||')"

echo "  → gh workflow run build-rust.yml --repo ${REPO}"
echo ""

gh workflow run build-rust.yml \
  --repo "$REPO" \
  --ref main \
  --field "ffm_version=${RUST_V}" \
  --field "jni_version=${RUST_V}"

echo "✅  build-rust.yml lanzado."
echo ""
echo "    Monitorea el progreso en:"
echo "    https://github.com/${REPO}/actions/workflows/build-rust.yml"
echo ""
echo "    Cuando termine, actualiza VERSIONS y lanza el release con:"
echo "      just release v1.0.0"
echo ""
