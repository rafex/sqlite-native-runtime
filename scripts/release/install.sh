#!/usr/bin/env sh
# ─────────────────────────────────────────────────────────────────────────────
# install.sh — Instala libsqlite_native_runtime en Linux (amd64 / arm64)
#
# Uso rápido (última versión):
#   curl -sS https://raw.githubusercontent.com/rafex/sqlite-native-runtime/main/scripts/release/install.sh | sh
#
# Versión específica:
#   SNR_VERSION=v0.1.1 curl -sS ...url.../install.sh | sh
#
# Forzar instalación en directorio de usuario (sin sudo):
#   SNR_USER_INSTALL=1 curl -sS ...url.../install.sh | sh
#
# Lógica de instalación:
#   Con sudo  → /usr/local/lib/libsqlite_native_runtime.so  (auto-detectado por la JVM)
#   Sin sudo  → ~/.local/lib/libsqlite_native_runtime.so   (auto-detectado por la JVM ≥0.1.1)
#              + export SNR_LIB añadido al shell rc del usuario
# ─────────────────────────────────────────────────────────────────────────────
set -e

REPO="rafex/sqlite-native-runtime"
GITHUB_API="https://api.github.com/repos/${REPO}/releases/latest"
GITHUB_DL="https://github.com/${REPO}/releases/download"

# ── Colores (desactivados si no hay TTY) ─────────────────────────────────────
if [ -t 1 ]; then
    BOLD="\033[1m"; GREEN="\033[32m"; YELLOW="\033[33m"; RED="\033[31m"; RESET="\033[0m"
else
    BOLD=""; GREEN=""; YELLOW=""; RED=""; RESET=""
fi

info()  { printf "${BOLD}%s${RESET}\n" "$*"; }
ok()    { printf "  ${GREEN}✓${RESET} %s\n" "$*"; }
warn()  { printf "  ${YELLOW}⚠${RESET}  %s\n" "$*"; }
error() { printf "  ${RED}✗${RESET}  %s\n" "$*" >&2; }
die()   { error "$*"; exit 1; }

# ── Herramienta de descarga ───────────────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
    download() { curl -fsSL "$1" -o "$2"; }
    fetch()    { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
    download() { wget -qO "$2" "$1"; }
    fetch()    { wget -qO- "$1"; }
else
    die "Se necesita curl o wget para continuar."
fi

# ── Detectar OS ───────────────────────────────────────────────────────────────
OS="$(uname -s)"
case "$OS" in
    Linux)  ;;
    Darwin)
        echo ""
        warn "macOS: los binarios pre-compilados no están disponibles en los releases."
        warn "Para compilar localmente (requiere Rust stable + GraalVM JDK 25):"
        echo ""
        echo "    git clone https://github.com/${REPO}.git"
        echo "    cd sqlite-native-runtime"
        echo "    make build-rust"
        echo "    # La librería queda en:"
        echo "    # sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib"
        echo ""
        exit 0
        ;;
    *)
        die "Sistema operativo no soportado: ${OS}"
        ;;
esac

# ── Detectar arquitectura ─────────────────────────────────────────────────────
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64)
        LIB_ARCH="linux-amd64"
        ;;
    aarch64|arm64)
        LIB_ARCH="linux-arm64"
        ;;
    armv7l|armv6l|armhf)
        echo ""
        warn "Raspberry Pi / ARM 32-bit detectado (${ARCH})."
        warn "sqlite-native-runtime NO soporta sistemas operativos de 32-bit en ARM."
        warn ""
        warn "Motivo: esta librería requiere Java 22+ (Panama FFM, JEP 454)."
        warn "Java 22 y superiores NO publican builds para arm32 (armhf/armv7l)."
        warn "GraalVM Native Image tampoco soporta arm32."
        warn ""
        warn "Solución para Raspberry Pi 3B / 4B:"
        warn "  Instala un sistema operativo de 64-bit:"
        warn "    • Raspberry Pi OS 64-bit → https://www.raspberrypi.com/software/"
        warn "    • Ubuntu Server 24.04 LTS arm64 → https://ubuntu.com/download/raspberry-pi"
        warn ""
        warn "Con un OS 64-bit (aarch64) la librería funciona sin cambios adicionales."
        echo ""
        exit 1
        ;;
    *)
        die "Arquitectura no soportada: ${ARCH}"
        ;;
esac

LIB_ARTIFACT="libsqlite_native_runtime-${LIB_ARCH}.so"
LIB_FILE="libsqlite_native_runtime.so"

# ── Detectar versión ─────────────────────────────────────────────────────────
if [ -n "${SNR_VERSION:-}" ]; then
    VERSION="$SNR_VERSION"
else
    info "→ Consultando última versión..."
    VERSION="$(fetch "$GITHUB_API" | grep '"tag_name"' \
        | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/' | tr -d '[:space:]')"
    [ -n "$VERSION" ] || die "No se pudo obtener la versión desde la API de GitHub."
fi

# ── Detectar modo de instalación ─────────────────────────────────────────────
SYS_DIR="/usr/local/lib"
USR_DIR="${HOME}/.local/lib"

if [ "${SNR_USER_INSTALL:-0}" = "1" ]; then
    USE_SUDO=0
elif sudo -n true 2>/dev/null; then
    USE_SUDO=1
else
    USE_SUDO=0
fi

if [ "$USE_SUDO" = "1" ]; then
    INSTALL_DIR="$SYS_DIR"
    MODE_LABEL="sistema (${INSTALL_DIR})"
else
    INSTALL_DIR="$USR_DIR"
    MODE_LABEL="usuario (${INSTALL_DIR})"
fi

DOWNLOAD_URL="${GITHUB_DL}/${VERSION}/${LIB_ARTIFACT}"
CHECKSUM_URL="${GITHUB_DL}/${VERSION}/${LIB_ARTIFACT}.sha256"
DEST="${INSTALL_DIR}/${LIB_FILE}"

# ── Resumen ───────────────────────────────────────────────────────────────────
echo ""
info "sqlite-native-runtime — instalador"
echo "  Versión     : ${VERSION}"
echo "  Artefacto   : ${LIB_ARTIFACT}"
echo "  Destino     : ${DEST}"
echo "  Modo        : ${MODE_LABEL}"
echo ""

# ── Descargar ─────────────────────────────────────────────────────────────────
TMP_DIR="$(mktemp -d)"
# shellcheck disable=SC2064
trap "rm -rf '${TMP_DIR}'" EXIT

TMP_LIB="${TMP_DIR}/${LIB_ARTIFACT}"
TMP_SHA="${TMP_DIR}/${LIB_ARTIFACT}.sha256"

info "→ Descargando ${LIB_ARTIFACT}..."
download "$DOWNLOAD_URL" "$TMP_LIB" \
    || die "Descarga fallida. Comprueba que el release ${VERSION} existe:\n    ${DOWNLOAD_URL}"

info "→ Verificando integridad (SHA256)..."
download "$CHECKSUM_URL" "$TMP_SHA" \
    || die "No se pudo descargar el checksum: ${CHECKSUM_URL}"

EXPECTED="$(awk '{print $1}' "$TMP_SHA")"
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL="$(sha256sum "$TMP_LIB" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL="$(shasum -a 256 "$TMP_LIB" | awk '{print $1}')"
else
    warn "sha256sum/shasum no disponible — omitiendo verificación de integridad."
    ACTUAL="$EXPECTED"
fi

if [ "$ACTUAL" != "$EXPECTED" ]; then
    error "Verificación SHA256 fallida."
    error "  esperado : ${EXPECTED}"
    error "  obtenido : ${ACTUAL}"
    die "El archivo descargado podría estar corrupto o manipulado."
fi
ok "SHA256 verificado"

# ── Instalar ──────────────────────────────────────────────────────────────────
info "→ Instalando en ${DEST}..."
mkdir -p "$INSTALL_DIR"

if [ "$USE_SUDO" = "1" ]; then
    sudo cp "$TMP_LIB" "$DEST"
    sudo chmod 755 "$DEST"
    # Actualizar caché del linker dinámico (ldconfig solo en Linux)
    if command -v ldconfig >/dev/null 2>&1; then
        sudo ldconfig 2>/dev/null || true
    fi
else
    cp "$TMP_LIB" "$DEST"
    chmod 755 "$DEST"
fi

ok "Instalado en ${DEST}"
echo ""

# ── Instrucciones post-instalación ────────────────────────────────────────────
if [ "$USE_SUDO" = "1" ]; then
    info "✅ Instalación completada (modo sistema)."
    echo ""
    echo "  La librería está en una ruta auto-detectada por la JVM."
    echo "  Ejecuta tu aplicación con:"
    echo ""
    echo "    java --enable-native-access=ALL-UNNAMED -jar mi-app.jar"
    echo ""
else
    info "✅ Instalación completada (modo usuario)."
    echo ""
    echo "  La JVM detecta automáticamente ~/.local/lib desde la versión v0.1.1."
    echo "  Si usas una versión anterior, añade al shell rc:"
    echo ""
    echo "    export SNR_LIB=\"${DEST}\""
    echo ""

    # Auto-configurar SNR_LIB en el shell rc si no está ya configurado
    _added=0
    for RC in "${HOME}/.bashrc" "${HOME}/.zshrc" "${HOME}/.profile"; do
        if [ -f "$RC" ] && ! grep -q "SNR_LIB" "$RC" 2>/dev/null; then
            printf '\n# sqlite-native-runtime (añadido por install.sh)\nexport SNR_LIB="%s"\n' \
                "$DEST" >> "$RC"
            ok "SNR_LIB añadido a ${RC}"
            _added=1
        fi
    done

    if [ "$_added" = "1" ]; then
        echo ""
        echo "  Recarga el shell para activarlo:"
        echo "    source ~/.bashrc   # o ~/.zshrc"
    fi

    echo ""
    echo "  Ejecuta tu aplicación con:"
    echo "    java --enable-native-access=ALL-UNNAMED -jar mi-app.jar"
    echo ""
fi
