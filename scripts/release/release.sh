#!/usr/bin/env bash
# release.sh — Flujo 3: Lanza release.yml vía workflow_dispatch.
#
# Lee el archivo VERSIONS del repositorio y muestra las versiones que se usarán.
# Luego dispara el workflow release.yml en GitHub Actions para que descargue
# los artefactos de GHCR y cree el GitHub Release.
#
# Uso:
#   just release v1.0.0
#   scripts/release/release.sh v1.0.0
#
# Prerequisitos:
#   - El tag <version> debe existir en el remoto (just tag-push)
#   - Los artefactos en VERSIONS deben estar publicados en GHCR (Flujo 2)
#   - gh CLI autenticado con permisos: contents:write, actions:write, packages:read
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

# ── Argumento: tag del release ────────────────────────────────────────────────
TAG="${1:-}"
if [[ -z "$TAG" ]]; then
  echo "Uso: just release <tag>" >&2
  echo "     Ejemplo: just release v1.0.0" >&2
  exit 1
fi

if ! [[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Error: el tag '${TAG}' no tiene formato semver (vMAJOR.MINOR.PATCH)" >&2
  exit 1
fi

# ── Leer VERSIONS ─────────────────────────────────────────────────────────────
VERSIONS_FILE="${ROOT}/VERSIONS"
if [[ ! -f "$VERSIONS_FILE" ]]; then
  echo "Error: archivo VERSIONS no encontrado en ${ROOT}" >&2
  exit 1
fi

RUST_TAG="$(grep '^RUST=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"
FAT_TAG="$(grep '^JAVA_FAT=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"
NAT_TAG="$(grep '^JAVA_NATIVE=' "$VERSIONS_FILE" | cut -d= -f2 | tr -d '[:space:]')"

if [[ -z "$RUST_TAG" || -z "$FAT_TAG" || -z "$NAT_TAG" ]]; then
  echo "Error: VERSIONS incompleto — se requieren RUST, JAVA_FAT y JAVA_NATIVE" >&2
  echo "       Archivo: ${VERSIONS_FILE}" >&2
  exit 1
fi

# ── Verificar que el tag del release existe en el remoto ──────────────────────
REPO="$(git -C "$ROOT" remote get-url origin \
  | sed 's|https://github.com/||;s|git@github.com:||;s|\.git$||')"

if ! git -C "$ROOT" ls-remote --tags origin "refs/tags/${TAG}" | grep -q .; then
  echo "Error: el tag '${TAG}' no existe en el remoto (origin)." >&2
  echo "       Ejecuta primero: just tag-push" >&2
  exit 1
fi

# ── Resumen y confirmación ─────────────────────────────────────────────────────
echo ""
echo "┌─────────────────────────────────────────────────────────────────┐"
echo "│  release — Flujo 3: Construcción del release final             │"
echo "└─────────────────────────────────────────────────────────────────┘"
echo ""
echo "  Release tag  : ${TAG}"
echo ""
echo "  Artefactos a descargar desde GHCR:"
echo "    RUST        = ${RUST_TAG}  → .so FFM + JNI (amd64, arm64)"
echo "    JAVA_FAT    = ${FAT_TAG}   → JARs FFM25 + FFM21 + JNI"
echo "    JAVA_NATIVE = ${NAT_TAG}   → binarios native FFM + JNI (amd64, arm64)"
echo ""
echo "  → gh workflow run release.yml --repo ${REPO}"
echo ""

gh workflow run release.yml \
  --repo "$REPO" \
  --ref main \
  --field "release_tag=${TAG}"

echo "✅  release.yml lanzado para ${TAG}."
echo ""
echo "    Monitorea el progreso en:"
echo "    https://github.com/${REPO}/actions/workflows/release.yml"
echo ""
