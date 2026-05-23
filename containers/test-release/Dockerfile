# ─────────────────────────────────────────────────────────────────────────────
# Dockerfile — Validación de artefactos del GitHub Release
#
# Verifica que los artefactos publicados funcionan correctamente en:
#   linux/amd64  (x86_64)
#   linux/arm64  (aarch64)
#
# Etapas:
#   installer     → descarga install.sh del release y lo ejecuta para instalar
#                   la .so; descarga y verifica (SHA256) JARs y binario nativo
#   test-jvm      → ejecuta la suite con thin JAR y fat JAR en GraalVM JDK 25
#   build-native  → compila Main.java como GraalVM Native Image (--no-fallback)
#   test-release  → ejecuta el binario pre-compilado del release y el binario
#                   compilado en esta build; arrastra el marcador de test-jvm
#
# Uso manual (local):
#   # Último release publicado, ambas arquitecturas (requiere QEMU o runners nativos):
#   docker buildx build --platform linux/amd64,linux/arm64 \
#     --progress=plain --target test-release containers/test-release/
#
#   # Versión específica en una sola arquitectura:
#   docker buildx build --platform linux/amd64 \
#     --build-arg SNR_VERSION=v0.1.1 \
#     --progress=plain --target test-release containers/test-release/
#
# Uso en CI (via just o el workflow verify-release.yml):
#   just test-release                  ← ambas archs, último release
#   just test-release v0.1.1           ← versión específica
#
# Variables:
#   SNR_VERSION   versión del release a probar (default: latest)
#   GRAALVM_TAG   tag de las imágenes GraalVM (default: 25)
# ─────────────────────────────────────────────────────────────────────────────

ARG SNR_VERSION=latest
ARG GRAALVM_TAG=25

# ── Etapa 1: install.sh del release + descarga de JARs y binario nativo ───────
#
# Se usa debian:12-slim (imagen mínima) para mantener el tamaño pequeño.
#
# install.sh se descarga del propio release (no del repositorio) para validar
# que el script publicado funciona correctamente.
#
# Sin sudo disponible en el contenedor, install.sh usa modo usuario:
#   → /root/.local/lib/libsqlite_native_runtime.so
# Las etapas siguientes copian la .so a /usr/local/lib/ donde la JVM la
# encontrará automáticamente.
#
# TARGETARCH es establecido por Docker buildx automáticamente.
# Cuando se usa `docker build` sin --platform en un runner nativo, se infiere
# desde uname -m como fallback (el mismo mecanismo que usa install.sh).
FROM debian:12-slim AS installer

ARG SNR_VERSION
ARG TARGETARCH

RUN apt-get update \
 && apt-get install -y --no-install-recommends curl ca-certificates \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /artifacts

RUN set -e; \
    REPO="rafex/sqlite-native-runtime"; \
    API="https://api.github.com/repos/${REPO}/releases"; \
    DL="https://github.com/${REPO}/releases/download"; \
    \
    # ── Resolver versión ─────────────────────────────────────────────────── \
    if [ "${SNR_VERSION}" = "latest" ]; then \
        VERSION="$(curl -fsSL "${API}/latest" \
            | grep '"tag_name"' \
            | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/' \
            | tr -d '[:space:]')"; \
        [ -n "${VERSION}" ] || { echo "ERROR: no se pudo obtener la versión"; exit 1; }; \
    else \
        VERSION="${SNR_VERSION}"; \
    fi; \
    echo "${VERSION}"   > /artifacts/VERSION; \
    echo "${VERSION#v}" > /artifacts/VERSION_NUM; \
    printf "\n>>> Release: %s\n\n" "${VERSION}"; \
    \
    BASE="${DL}/${VERSION}"; \
    VER="${VERSION#v}"; \
    \
    # ── Inferir arquitectura (TARGETARCH puede estar vacío sin --platform) ── \
    TARCH="${TARGETARCH:-}"; \
    if [ -z "${TARCH}" ]; then \
        case "$(uname -m)" in \
            x86_64)         TARCH="amd64" ;; \
            aarch64|arm64)  TARCH="arm64" ;; \
            *) echo "ERROR: arquitectura no soportada: $(uname -m)"; exit 1 ;; \
        esac; \
    fi; \
    case "${TARCH}" in \
        amd64) ARCH_LABEL="linux-amd64" ;; \
        arm64) ARCH_LABEL="linux-arm64" ;; \
        *)     echo "ERROR: TARGETARCH no soportado: ${TARCH}"; exit 1 ;; \
    esac; \
    echo "${ARCH_LABEL}" > /artifacts/ARCH_LABEL; \
    printf ">>> Arquitectura: %s\n\n" "${ARCH_LABEL}"; \
    \
    # ── install.sh del release — valida el script de instalación publicado ── \
    # Descarga el install.sh desde los assets del release (no del repo),       \
    # lo ejecuta con SNR_VERSION fijado para instalar exactamente esta versión. \
    # Sin sudo en el contenedor → modo usuario → /root/.local/lib/             \
    printf ">>> Ejecutando install.sh del release %s...\n" "${VERSION}"; \
    curl -fsSL "${BASE}/install.sh" -o /tmp/install.sh; \
    chmod +x /tmp/install.sh; \
    SNR_VERSION="${VERSION}" sh /tmp/install.sh; \
    \
    # Verificar que la librería quedó instalada correctamente \
    LIB_PATH="/root/.local/lib/libsqlite_native_runtime.so"; \
    [ -f "${LIB_PATH}" ] || { echo "ERROR: install.sh no instaló la librería en ${LIB_PATH}"; exit 1; }; \
    printf "\n✓ Librería instalada en %s\n" "${LIB_PATH}"; \
    cp "${LIB_PATH}" /artifacts/libsqlite_native_runtime.so; \
    \
    # ── Función de descarga + verificación SHA256 ─────────────────────────── \
    fetch_and_verify() { \
        URL="$1"; DEST="$2"; \
        printf "  → %s\n" "$(basename "${DEST}")"; \
        curl -fsSL "${URL}"        -o "${DEST}"; \
        curl -fsSL "${URL}.sha256" -o "${DEST}.sha256"; \
        EXPECTED="$(awk '{print $1}' "${DEST}.sha256")"; \
        ACTUAL="$(sha256sum "${DEST}" | awk '{print $1}')"; \
        if [ "${ACTUAL}" != "${EXPECTED}" ]; then \
            printf "  ✗ SHA256 mismatch: %s\n    esperado : %s\n    obtenido : %s\n" \
                "$(basename "${DEST}")" "${EXPECTED}" "${ACTUAL}"; \
            exit 1; \
        fi; \
        printf "  ✓ SHA256 OK: %s\n" "$(basename "${DEST}")"; \
    }; \
    \
    printf "\n>>> Descargando y verificando JARs y binario nativo...\n"; \
    \
    # ── Thin JAR ─────────────────────────────────────────────────────────── \
    fetch_and_verify "${BASE}/sqlite-native-runtime-${VER}.jar" \
                     "/artifacts/sqlite-native-runtime-${VER}.jar"; \
    \
    # ── Fat JAR ──────────────────────────────────────────────────────────── \
    fetch_and_verify "${BASE}/sqlite-native-runtime-${VER}-fat.jar" \
                     "/artifacts/sqlite-native-runtime-${VER}-fat.jar"; \
    \
    # ── Binario nativo pre-compilado del release ──────────────────────────── \
    fetch_and_verify "${BASE}/snr-smoke-${ARCH_LABEL}" \
                     "/artifacts/snr-smoke-${ARCH_LABEL}"; \
    chmod +x "/artifacts/snr-smoke-${ARCH_LABEL}"; \
    cp "/artifacts/snr-smoke-${ARCH_LABEL}" /artifacts/snr-smoke-prebuilt; \
    \
    printf "\n>>> Artefactos listos:\n"; \
    ls -lh /artifacts/*.so /artifacts/*.jar /artifacts/snr-smoke-prebuilt

# ── Etapa 2: Pruebas en la JVM ────────────────────────────────────────────────
#
# ghcr.io/graalvm/jdk-community:25 es multi-arch (amd64 + arm64).
# La .so instalada por install.sh se copia a /usr/local/lib/ (ruta estándar
# de búsqueda de la JVM) y se registra con ldconfig.
FROM ghcr.io/graalvm/jdk-community:${GRAALVM_TAG} AS test-jvm

ARG GRAALVM_TAG

WORKDIR /test

COPY --from=installer /artifacts/libsqlite_native_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=installer /artifacts/ /artifacts/
COPY src/Main.java ./Main.java

# Compilar la suite con el thin JAR
RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    javac -cp "/artifacts/sqlite-native-runtime-${VER}.jar" Main.java

# ── Test 1: thin JAR ──────────────────────────────────────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 1/4 — JVM thin JAR                ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    VER="$(cat /artifacts/VERSION_NUM)" && \
    java --enable-native-access=ALL-UNNAMED \
         -Dsnr.test.mode=JVM-thin \
         -cp "/artifacts/sqlite-native-runtime-${VER}.jar:." \
         Main

# ── Test 2: fat JAR ───────────────────────────────────────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 2/4 — JVM fat JAR                 ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    VER="$(cat /artifacts/VERSION_NUM)" && \
    java --enable-native-access=ALL-UNNAMED \
         -Dsnr.test.mode=JVM-fat \
         -cp "/artifacts/sqlite-native-runtime-${VER}-fat.jar:." \
         Main

# Marcador de éxito (la etapa test-release lo arrastra para crear dependencia)
RUN echo "JVM tests OK" > /test/jvm-ok

# ── Etapa 3: GraalVM Native Image ─────────────────────────────────────────────
#
# ghcr.io/graalvm/native-image-community:25 incluye native-image pre-instalado.
# Se usa --no-fallback para garantizar compilación nativa pura (sin JVM backup).
FROM ghcr.io/graalvm/native-image-community:${GRAALVM_TAG} AS build-native

ARG GRAALVM_TAG

WORKDIR /build

COPY --from=installer /artifacts/libsqlite_native_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=installer /artifacts/ /artifacts/
COPY src/Main.java ./Main.java

RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    JAR="/artifacts/sqlite-native-runtime-${VER}.jar"; \
    printf "\n>>> Compilando Main.java como Native Image (GraalVM %s)...\n\n" "${GRAALVM_TAG}"; \
    javac -cp "${JAR}" Main.java; \
    native-image \
        --initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary \
        --enable-native-access=ALL-UNNAMED \
        --no-fallback \
        -H:Name=snr-release-test \
        -cp "${JAR}:." \
        Main; \
    printf "\n>>> Binario compilado:\n"; \
    ls -lh snr-release-test

# ── Etapa 4: Ejecutar binarios nativos ────────────────────────────────────────
#
# debian:12-slim + libstdc++6 es suficiente para ejecutar binarios GraalVM.
# Esta etapa tiene dependencia explícita de test-jvm (COPY --from=test-jvm):
# si test-jvm falló, el build falla aunque no llegue a este punto.
FROM debian:12-slim AS test-release

RUN apt-get update \
 && apt-get install -y --no-install-recommends libstdc++6 \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /test

COPY --from=installer /artifacts/libsqlite_native_runtime.so /usr/local/lib/
RUN ldconfig

# Arrastrar el marcador: si test-jvm no completó, este COPY falla → build falla
COPY --from=test-jvm    /test/jvm-ok              /test/jvm-ok
COPY --from=installer   /artifacts/snr-smoke-prebuilt ./snr-smoke-prebuilt
COPY --from=build-native /build/snr-release-test   ./snr-release-test

# ── Test 3: binario nativo pre-compilado del release ─────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 3/4 — Native Image (prebuilt)     ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    ./snr-smoke-prebuilt && \
    printf "\n✓ snr-smoke prebuilt: OK\n"

# ── Test 4: Main.java compilado como Native Image en esta build ───────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 4/4 — Native Image (esta build)   ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    ./snr-release-test && \
    printf "\n✓ snr-release-test native: OK\n"

# ── Resumen final ─────────────────────────────────────────────────────────────
RUN printf "\n"; \
    printf "╔══════════════════════════════════════════╗\n"; \
    printf "║           RESULTADO FINAL                ║\n"; \
    printf "╠══════════════════════════════════════════╣\n"; \
    printf "║  ✓ install.sh del release    — OK        ║\n"; \
    printf "║  ✓ JVM + thin JAR            — OK        ║\n"; \
    printf "║  ✓ JVM + fat JAR             — OK        ║\n"; \
    printf "║  ✓ Native Image prebuilt     — OK        ║\n"; \
    printf "║  ✓ Native Image (esta build) — OK        ║\n"; \
    printf "╚══════════════════════════════════════════╝\n\n"
