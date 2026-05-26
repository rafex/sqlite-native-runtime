#!/usr/bin/env bash
# release-dispatch — Lanza release.yml vía workflow_dispatch con el tag y versiones
# leídas de VERSIONS. Alternativa manual a hacer push del tag v*.
#
# Uso:
#   just release-dispatch v0.3.6
#   scripts/release/release-dispatch.sh v0.3.6
#
# Variables opcionales (sobrescriben VERSIONS):
#   FFM_VERSION=0.2.0 just release-dispatch v0.3.6
#   JNI_VERSION=0.2.0 just release-dispatch v0.3.6
#
# Flujo recomendado:
#   1. just tag-create v0.3.6     → crea el tag local
#   2. just rust-publish          → publica .so en GHCR (si cambiaron)
#   3. just release-dispatch v0.3.6 → lanza el release con las versiones de VERSIONS
#
# Requisitos:
#   gh CLI autenticado con permisos contents:write, actions:write, packages:read.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

# ── Argumento: tag de release ─────────────────────────────────────────────────
TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  echo "Uso: just release-dispatch <tag>" >&2
  echo "     Ejemplo: just release-dispatch v0.3.6" >&2
  exit 1
fi

if ! [[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Error: el tag '${TAG}' no tiene formato semver (vMAJOR.MINOR.PATCH)" >&2
  exit 1
fi

# ── Leer versiones desde VERSIONS (env vars tienen prioridad) ─────────────────
VERSIONS_FILE="${ROOT}/VERSIONS"
if [[ ! -f "$VERSIONS_FILE" ]]; then
  echo "Error: archivo VERSIONS no encontrado en ${ROOT}" >&2
  exit 1
fi

_FFM="$(grep '^FFM_VERSION=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"
_JNI="$(grep '^JNI_VERSION=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"

FFM_VERSION="${FFM_VERSION:-${_FFM}}"
JNI_VERSION="${JNI_VERSION:-${_JNI}}"

if [[ -z "$FFM_VERSION" || -z "$JNI_VERSION" ]]; then
  echo "Error: FFM_VERSION o JNI_VERSION vacíos en VERSIONS." >&2
  exit 1
fi

# ── Verificar que el tag existe en el remoto ──────────────────────────────────
REPO="$(git -C "$ROOT" remote get-url origin \
  | sed 's|https://github.com/||;s|git@github.com:||;s|\.git$||')"

if ! git -C "$ROOT" ls-remote --tags origin "refs/tags/${TAG}" | grep -q .; then
  echo "Error: el tag '${TAG}' no existe en el remoto (origin)." >&2
  echo "       Ejecuta primero: just tag-push" >&2
  exit 1
fi

echo ""
echo "┌──────────────────────────────────────────────┐"
echo "│  release-dispatch — lanzar release manual   │"
echo "└──────────────────────────────────────────────┘"
echo ""
echo "  Tag         : ${TAG}"
echo "  FFM_VERSION : ${FFM_VERSION}  (ghcr.io/${REPO%/*}/ether-sqlite-ffm)"
echo "  JNI_VERSION : ${JNI_VERSION}  (ghcr.io/${REPO%/*}/ether-sqlite-jni)"
echo ""
echo "  → gh workflow run release.yml --repo ${REPO}"
echo ""

gh workflow run release.yml \
  --repo "$REPO" \
  --ref main \
  --field "tag=${TAG}" \
  --field "ffm_version=${FFM_VERSION}" \
  --field "jni_version=${JNI_VERSION}"

echo "✅  release.yml lanzado para ${TAG}."
echo ""
echo "    Monitorea el progreso en:"
echo "    https://github.com/${REPO}/actions/workflows/release.yml"
echo ""
