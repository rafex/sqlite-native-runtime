# ─────────────────────────────────────────────────────────────────────────────
# Dockerfile — Validación de artefactos del GitHub Release
#
# Verifica que los artefactos publicados funcionan correctamente en:
#   linux/amd64  (x86_64)
#   linux/arm64  (aarch64)
#
# Etapas:
#   downloader    → descarga y verifica (SHA256) .so, JARs y binario nativo pre-compilado
#   test-jvm      → ejecuta la suite con thin JAR y fat JAR en la JVM de GraalVM 25
#   build-native  → compila Main.java como GraalVM Native Image
#   test-release  → ejecuta el binario pre-compilado del release y el binario custom;
#                   también arrastra el resultado de test-jvm para fallar si éste falló.
#
# Uso rápido:
#   # Último release publicado, ambas arquitecturas:
#   docker buildx build --platform linux/amd64,linux/arm64 \
#     --progress=plain containers/test-release/
#
#   # Versión específica en una sola arquitectura:
#   docker buildx build --platform linux/amd64 \
#     --build-arg SNR_VERSION=v0.1.1 \
#     --progress=plain containers/test-release/
#
# Variables:
#   SNR_VERSION   versión del release a probar (default: latest)
#   GRAALVM_TAG   tag de la imagen GraalVM (default: 25)
# ─────────────────────────────────────────────────────────────────────────────

ARG SNR_VERSION=latest
ARG GRAALVM_TAG=25

# ── Etapa 1: Descargar y verificar todos los artefactos ───────────────────────
#
# TARGETARCH es establecido por Docker buildx automáticamente:
#   linux/amd64  → TARGETARCH=amd64
#   linux/arm64  → TARGETARCH=arm64
FROM ubuntu:24.04 AS downloader

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
        VERSION="$(curl -fsSL "${API}/latest" | grep '"tag_name"' \
            | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/' \
            | tr -d '[:space:]')"; \
        [ -n "${VERSION}" ] || { echo "ERROR: no se pudo obtener la versión"; exit 1; }; \
    else \
        VERSION="${SNR_VERSION}"; \
    fi; \
    echo "${VERSION}"     > VERSION; \
    echo "${VERSION#v}"   > VERSION_NUM; \
    printf "\n>>> Versión: %s  |  Arch: %s\n\n" "${VERSION}" "${TARGETARCH}"; \
    \
    BASE="${DL}/${VERSION}"; \
    VER="${VERSION#v}"; \
    \
    # ── Mapear TARGETARCH al nombre del artefacto ────────────────────────── \
    case "${TARGETARCH}" in \
        amd64) ARCH_LABEL="linux-amd64" ;; \
        arm64) ARCH_LABEL="linux-arm64" ;; \
        *)     echo "ERROR: arquitectura no soportada: ${TARGETARCH}"; exit 1 ;; \
    esac; \
    \
    # ── Función de descarga + verificación SHA256 ─────────────────────────── \
    # Uso: fetch_and_verify <url-artefacto> <destino-local> \
    fetch_and_verify() { \
        URL="$1"; DEST="$2"; \
        echo "  → Descargando $(basename ${DEST})..."; \
        curl -fsSL "${URL}"        -o "${DEST}"; \
        curl -fsSL "${URL}.sha256" -o "${DEST}.sha256"; \
        EXPECTED="$(awk '{print $1}' "${DEST}.sha256")"; \
        ACTUAL="$(sha256sum "${DEST}" | awk '{print $1}')"; \
        if [ "${ACTUAL}" != "${EXPECTED}" ]; then \
            echo "  ✗ SHA256 mismatch para $(basename ${DEST})"; \
            echo "    esperado : ${EXPECTED}"; \
            echo "    obtenido : ${ACTUAL}"; \
            exit 1; \
        fi; \
        echo "  ✓ SHA256 OK: $(basename ${DEST})"; \
    }; \
    \
    # ── Librería nativa ──────────────────────────────────────────────────── \
    LIB="libsqlite_native_runtime-${ARCH_LABEL}.so"; \
    fetch_and_verify "${BASE}/${LIB}" "${LIB}"; \
    cp "${LIB}" libsqlite_native_runtime.so; \
    \
    # ── Thin JAR ─────────────────────────────────────────────────────────── \
    JAR="sqlite-native-runtime-${VER}.jar"; \
    fetch_and_verify "${BASE}/${JAR}" "${JAR}"; \
    \
    # ── Fat JAR ──────────────────────────────────────────────────────────── \
    FAT="sqlite-native-runtime-${VER}-fat.jar"; \
    fetch_and_verify "${BASE}/${FAT}" "${FAT}"; \
    \
    # ── Binario nativo pre-compilado del release ──────────────────────────── \
    SNR_BINARY="snr-smoke-${ARCH_LABEL}"; \
    fetch_and_verify "${BASE}/${SNR_BINARY}" "${SNR_BINARY}"; \
    chmod +x "${SNR_BINARY}"; \
    cp "${SNR_BINARY}" snr-smoke-prebuilt; \
    \
    echo; ls -lh /artifacts/

# ── Etapa 2: Pruebas en la JVM (thin JAR + fat JAR) ──────────────────────────
FROM ghcr.io/graalvm/jdk-community:${GRAALVM_TAG} AS test-jvm

ARG GRAALVM_TAG

WORKDIR /test

# Instalar la .so en la ruta estándar del sistema
COPY --from=downloader /artifacts/libsqlite_native_runtime.so /usr/local/lib/
RUN ldconfig

# Copiar artefactos y fuente de la suite de tests
COPY --from=downloader /artifacts/ /artifacts/
COPY src/Main.java ./Main.java

# Compilar la suite con el thin JAR
RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    javac -cp "/artifacts/sqlite-native-runtime-${VER}.jar" Main.java

# ── Test 1: thin JAR ──────────────────────────────────────────────────────────
RUN printf "\n╔══════════════════════════════════╗\n"; \
    printf "║  Test: thin JAR  (JVM %-10s║\n" "$(java -version 2>&1 | head -1 | sed 's/.*"\(.*\)".*/\1/')"; \
    printf "╚══════════════════════════════════╝\n\n"; \
    VER="$(cat /artifacts/VERSION_NUM)"; \
    java --enable-native-access=ALL-UNNAMED \
         -Dsnr.test.mode=JVM-thin \
         -cp "/artifacts/sqlite-native-runtime-${VER}.jar:." \
         Main

# ── Test 2: fat JAR (no recompilamos: el fat JAR ya embebe todo) ──────────────
RUN printf "\n╔══════════════════════════════════╗\n"; \
    printf "║  Test: fat JAR                   ║\n"; \
    printf "╚══════════════════════════════════╝\n\n"; \
    VER="$(cat /artifacts/VERSION_NUM)"; \
    java --enable-native-access=ALL-UNNAMED \
         -Dsnr.test.mode=JVM-fat \
         -cp "/artifacts/sqlite-native-runtime-${VER}-fat.jar:." \
         Main

# Marcador de éxito — si algún RUN anterior falló, este fichero no existe
RUN echo "JVM tests OK" > /test/jvm-ok

# ── Etapa 3: Compilar con GraalVM Native Image ────────────────────────────────
FROM ghcr.io/graalvm/native-image-community:${GRAALVM_TAG} AS build-native

ARG GRAALVM_TAG

WORKDIR /build

# La .so debe estar presente durante la compilación (aunque SqliteLibrary
# se inicializa en runtime, native-image resuelve la clase en build-time)
COPY --from=downloader /artifacts/libsqlite_native_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=downloader /artifacts/ /artifacts/
COPY src/Main.java ./Main.java

RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    JAR="/artifacts/sqlite-native-runtime-${VER}.jar"; \
    printf "\n>>> Compilando suite como Native Image (GraalVM %s)...\n\n" "${GRAALVM_TAG}"; \
    javac -cp "${JAR}" Main.java; \
    native-image \
        --initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary \
        --enable-native-access=ALL-UNNAMED \
        --no-fallback \
        -H:Name=snr-release-test \
        -cp "${JAR}:." \
        Main; \
    echo; ls -lh snr-release-test; echo

# ── Etapa 4: Ejecutar binarios nativos ────────────────────────────────────────
#
# Esta etapa tiene dependencia explícita de test-jvm (vía COPY --from).
# Si test-jvm falló, esta etapa también falla aunque no haya llegado a ejecutar.
FROM ubuntu:24.04 AS test-release

RUN apt-get update \
 && apt-get install -y --no-install-recommends libstdc++6 \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /test

# Instalar la .so
COPY --from=downloader /artifacts/libsqlite_native_runtime.so /usr/local/lib/
RUN ldconfig

# Arrastrar el marcador de JVM — falla si test-jvm no completó con éxito
COPY --from=test-jvm /test/jvm-ok /test/jvm-ok

# Binario nativo pre-compilado del release
COPY --from=downloader /artifacts/snr-smoke-prebuilt ./snr-smoke-prebuilt

# Binario nativo compilado en esta build a partir de Main.java
COPY --from=build-native /build/snr-release-test ./snr-release-test

# ── Test 3: binario nativo pre-compilado del release ─────────────────────────
RUN printf "\n╔══════════════════════════════════╗\n"; \
    printf "║  Test: snr-smoke (prebuilt)      ║\n"; \
    printf "╚══════════════════════════════════╝\n\n"; \
    ./snr-smoke-prebuilt; \
    echo; echo "✓ snr-smoke (prebuilt release binary): OK"

# ── Test 4: Main.java compilado como Native Image en esta build ───────────────
RUN printf "\n╔══════════════════════════════════╗\n"; \
    printf "║  Test: snr-release-test (native) ║\n"; \
    printf "╚══════════════════════════════════╝\n\n"; \
    ./snr-release-test; \
    echo; echo "✓ Main.java native image: OK"

RUN printf "\n"; \
    printf "══════════════════════════════════════\n"; \
    printf "  ✓ JVM thin JAR  — OK (etapa 2)\n"; \
    printf "  ✓ JVM fat JAR   — OK (etapa 2)\n"; \
    printf "  ✓ Native prebuilt release — OK\n"; \
    printf "  ✓ Native image build      — OK\n"; \
    printf "══════════════════════════════════════\n"; \
    printf "  TODOS LOS TESTS PASARON ✓\n"; \
    printf "══════════════════════════════════════\n\n"
