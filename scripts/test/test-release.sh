#!/usr/bin/env sh
# ─────────────────────────────────────────────────────────────────────────────
# test-release.sh — Valida los artefactos del GitHub Release
#
# Uso:
#   scripts/test/test-release.sh                         # último release, ambas archs
#   scripts/test/test-release.sh v0.1.1                  # versión específica
#   scripts/test/test-release.sh v0.1.1 linux/amd64      # solo amd64
#   scripts/test/test-release.sh latest linux/arm64      # solo arm64
#
# También via just:
#   just test-release              # último release, ambas archs
#   just test-release v0.1.1       # versión específica
# ─────────────────────────────────────────────────────────────────────────────
set -e

SNR_VERSION="${1:-latest}"
PLATFORM="${2:-linux/amd64,linux/arm64}"
CONTAINER_ENGINE="${CONTAINER_ENGINE:-docker}"

# ── Colores ───────────────────────────────────────────────────────────────────
if [ -t 1 ]; then
    BOLD="\033[1m"; GREEN="\033[32m"; YELLOW="\033[33m"; RED="\033[31m"; RESET="\033[0m"
else
    BOLD=""; GREEN=""; YELLOW=""; RED=""; RESET=""
fi

info()  { printf "${BOLD}%s${RESET}\n" "$*"; }
ok()    { printf "  ${GREEN}✓${RESET} %s\n" "$*"; }
warn()  { printf "  ${YELLOW}⚠${RESET}  %s\n" "$*"; }
die()   { printf "  ${RED}✗${RESET}  %s\n" "$*" >&2; exit 1; }

# ── Verificar que buildx esté disponible ─────────────────────────────────────
if ! "${CONTAINER_ENGINE}" buildx version >/dev/null 2>&1; then
    die "${CONTAINER_ENGINE} buildx no está disponible. Instala docker-buildx o buildx plugin."
fi

# ── Detectar si buildx tiene un builder multi-arch activo ────────────────────
BUILDER="$("${CONTAINER_ENGINE}" buildx inspect --bootstrap 2>/dev/null | grep -E 'Name:|Platforms:' | head -4 || true)"
if echo "${PLATFORM}" | grep -q ","; then
    # Multi-arch: necesita un builder con soporte QEMU o runners nativos
    if ! "${CONTAINER_ENGINE}" buildx inspect --bootstrap 2>/dev/null | grep -q "linux/arm64"; then
        warn "El builder activo no soporta linux/arm64 (necesario para multi-arch local)."
        warn "Para habilitar emulación QEMU:"
        echo ""
        echo "    docker buildx create --name multiarch --use"
        echo "    docker run --privileged --rm tonistiigi/binfmt --install all"
        echo ""
        warn "En CI se usan runners nativos (ubuntu-latest / ubuntu-24.04-arm)."
        warn "Continuando con platform=${PLATFORM} de todas formas..."
    fi
fi

# ── Ruta al Dockerfile ────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DOCKERFILE_DIR="${REPO_ROOT}/containers/test-release"

[ -f "${DOCKERFILE_DIR}/Dockerfile" ] \
    || die "No se encontró Dockerfile en ${DOCKERFILE_DIR}"

echo ""
info "sqlite-native-runtime — test de artefactos del release"
echo "  Versión   : ${SNR_VERSION}"
echo "  Plataforma: ${PLATFORM}"
echo "  Engine    : ${CONTAINER_ENGINE}"
echo "  Contexto  : ${DOCKERFILE_DIR}"
echo ""

# ── Ejecutar el build multi-plataforma ────────────────────────────────────────
"${CONTAINER_ENGINE}" buildx build \
    --platform "${PLATFORM}" \
    --build-arg "SNR_VERSION=${SNR_VERSION}" \
    --progress=plain \
    --target test-release \
    "${DOCKERFILE_DIR}"

echo ""
ok "Test de release completado: versión ${SNR_VERSION} en ${PLATFORM}"
echo ""
