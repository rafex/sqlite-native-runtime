# ─────────────────────────────────────────────────────────────────────────────
# Dockerfile — Validación de artefactos del GitHub Release
#
# Verifica que los artefactos publicados funcionan correctamente en:
#   linux/amd64  (x86_64)
#   linux/arm64  (aarch64)
#
# Artefactos validados:
#   FFM (Panama FFM, Java 25 stable, GraalVM native-image):
#     libether_sqlite_ffm_runtime-{arch}.so
#     ether-sqlite-ffm-runtime-{ver}.jar       (thin)
#     ether-sqlite-ffm-runtime-{ver}-fat.jar   (fat)
#     ether-sqlite-ffm-linux-{arch}.bin        (native image pre-compilado)
#
#   JNI (JNI real, Java 21, GraalVM native-image):
#     libether_sqlite_jni_runtime-{arch}.so
#     ether-sqlite-jni-runtime-{ver}.jar       (thin, bytecode Java 21)
#     ether-sqlite-jni-runtime-{ver}-fat.jar   (fat)
#     ether-sqlite-jni-linux-{arch}.bin        (native image pre-compilado)
#
# Etapas:
#   installer        → descarga install.sh del release y verifica todos los
#                      artefactos (SHA256); instala las .so en /artifacts/
#   test-jvm-ffm     → prueba FFM fat JAR en Java 25 (stable FFM, sin preview)
#   test-jvm-jni     → prueba JNI fat JAR en Java 21 (sin --enable-preview)
#   build-native-ffm → compila MainFfm.java como GraalVM Native Image (Java 25)
#   build-native-jni → compila MainJni.java como GraalVM Native Image (Java 21→25)
#   test-release     → ejecuta pre-built + compilados; arrastra todos los OK
#
# Uso manual (local):
#   docker buildx build --platform linux/amd64,linux/arm64 \
#     --progress=plain --target test-release containers/test-release/
#
#   # Versión específica:
#   docker buildx build --platform linux/amd64 \
#     --build-arg ETHER_SQLITE_VERSION=v0.2.0 \
#     --progress=plain --target test-release containers/test-release/
#
# Variables:
#   ETHER_SQLITE_VERSION   versión del release a probar (default: latest)
#   GRAALVM_TAG            tag de GraalVM para native-image (default: 25)
# ─────────────────────────────────────────────────────────────────────────────

ARG ETHER_SQLITE_VERSION=latest
ARG GRAALVM_TAG=25

# ── Etapa 1: installer — descarga y verifica todos los artefactos ─────────────
FROM debian:12-slim AS installer

ARG ETHER_SQLITE_VERSION
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
    if [ "${ETHER_SQLITE_VERSION}" = "latest" ]; then \
        VERSION="$(curl -fsSL "${API}/latest" \
            | grep '"tag_name"' \
            | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/' \
            | tr -d '[:space:]')"; \
        [ -n "${VERSION}" ] || { echo "ERROR: no se pudo obtener la versión"; exit 1; }; \
    else \
        VERSION="${ETHER_SQLITE_VERSION}"; \
    fi; \
    echo "${VERSION}"   > /artifacts/VERSION; \
    echo "${VERSION#v}" > /artifacts/VERSION_NUM; \
    printf "\n>>> Release: %s\n\n" "${VERSION}"; \
    \
    BASE="${DL}/${VERSION}"; \
    VER="${VERSION#v}"; \
    \
    # ── Inferir arquitectura ─────────────────────────────────────────────── \
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
    printf ">>> Ejecutando install.sh del release %s...\n" "${VERSION}"; \
    curl -fsSL "${BASE}/install.sh" -o /tmp/install.sh; \
    chmod +x /tmp/install.sh; \
    # install.sh instala UNA librería por ejecución según ETHER_SQLITE_BINDING. \
    # El destino es ~/.local/lib/{lib_base}.so (sin sufijo de arquitectura). \
    ETHER_SQLITE_VERSION="${VERSION}" ETHER_SQLITE_BINDING=ffm sh /tmp/install.sh; \
    ETHER_SQLITE_VERSION="${VERSION}" ETHER_SQLITE_BINDING=jni sh /tmp/install.sh; \
    \
    FFM_LIB="/root/.local/lib/libether_sqlite_ffm_runtime.so"; \
    JNI_LIB="/root/.local/lib/libether_sqlite_jni_runtime.so"; \
    [ -f "${FFM_LIB}" ] || { echo "ERROR: install.sh no instaló ${FFM_LIB}"; exit 1; }; \
    [ -f "${JNI_LIB}" ] || { echo "ERROR: install.sh no instaló ${JNI_LIB}"; exit 1; }; \
    cp "${FFM_LIB}" /artifacts/libether_sqlite_ffm_runtime.so; \
    cp "${JNI_LIB}" /artifacts/libether_sqlite_jni_runtime.so; \
    printf "\n✓ Librerías FFM y JNI instaladas\n"; \
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
    printf "\n>>> Descargando y verificando artefactos...\n"; \
    \
    # ── FFM JARs ─────────────────────────────────────────────────────────── \
    fetch_and_verify "${BASE}/ether-sqlite-ffm-runtime-${VER}.jar" \
                     "/artifacts/ether-sqlite-ffm-runtime-${VER}.jar"; \
    fetch_and_verify "${BASE}/ether-sqlite-ffm-runtime-${VER}-fat.jar" \
                     "/artifacts/ether-sqlite-ffm-runtime-${VER}-fat.jar"; \
    \
    # ── JNI JARs ─────────────────────────────────────────────────────────── \
    fetch_and_verify "${BASE}/ether-sqlite-jni-runtime-${VER}.jar" \
                     "/artifacts/ether-sqlite-jni-runtime-${VER}.jar"; \
    fetch_and_verify "${BASE}/ether-sqlite-jni-runtime-${VER}-fat.jar" \
                     "/artifacts/ether-sqlite-jni-runtime-${VER}-fat.jar"; \
    \
    # ── Binarios nativos pre-compilados ──────────────────────────────────── \
    fetch_and_verify "${BASE}/ether-sqlite-ffm-${ARCH_LABEL}.bin" \
                     "/artifacts/ether-sqlite-ffm-${ARCH_LABEL}.bin"; \
    chmod +x "/artifacts/ether-sqlite-ffm-${ARCH_LABEL}.bin"; \
    cp "/artifacts/ether-sqlite-ffm-${ARCH_LABEL}.bin" /artifacts/ether-sqlite-ffm-prebuilt; \
    \
    fetch_and_verify "${BASE}/ether-sqlite-jni-${ARCH_LABEL}.bin" \
                     "/artifacts/ether-sqlite-jni-${ARCH_LABEL}.bin"; \
    chmod +x "/artifacts/ether-sqlite-jni-${ARCH_LABEL}.bin"; \
    cp "/artifacts/ether-sqlite-jni-${ARCH_LABEL}.bin" /artifacts/ether-sqlite-jni-prebuilt; \
    \
    printf "\n>>> Artefactos listos:\n"; \
    ls -lh /artifacts/*.so /artifacts/*.jar /artifacts/*-prebuilt

# ── Etapa 2: test-jvm-ffm — FFM fat JAR en Java 25 ───────────────────────────
#
# Panama FFM es estable en Java 25 (JEP 454). No requiere --enable-preview.
# Solo --enable-native-access=ALL-UNNAMED para acceso a memoria nativa.
FROM ghcr.io/graalvm/jdk-community:25 AS test-jvm-ffm

WORKDIR /test

COPY --from=installer /artifacts/libether_sqlite_ffm_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=installer /artifacts/ /artifacts/
COPY src/MainFfm.java ./MainFfm.java

RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    javac -cp "/artifacts/ether-sqlite-ffm-runtime-${VER}-fat.jar" MainFfm.java

# ── Test 1: FFM fat JAR en JVM Java 25 ───────────────────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 1/6 — JVM FFM fat JAR (Java 25)  ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    VER="$(cat /artifacts/VERSION_NUM)" && \
    java --enable-native-access=ALL-UNNAMED \
         -Dether.sqlite.test.mode=JVM-FFM-fat \
         -cp "/artifacts/ether-sqlite-ffm-runtime-${VER}-fat.jar:." \
         MainFfm

RUN echo "JVM FFM tests OK" > /test/jvm-ffm-ok

# ── Etapa 3: test-jvm-jni — JNI fat JAR en Java 21 ──────────────────────────
#
# JNI no requiere --enable-preview ni --enable-native-access.
# Compatible con Java 21+ (bytecode class 65.0).
FROM eclipse-temurin:21-jdk-noble AS test-jvm-jni

WORKDIR /test

COPY --from=installer /artifacts/libether_sqlite_jni_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=installer /artifacts/ /artifacts/
COPY src/MainJni.java ./MainJni.java

RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    javac -cp "/artifacts/ether-sqlite-jni-runtime-${VER}-fat.jar" MainJni.java

# ── Test 2: JNI fat JAR en JVM Java 21 ───────────────────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 2/6 — JVM JNI fat JAR (Java 21)  ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    VER="$(cat /artifacts/VERSION_NUM)" && \
    java -Dether.sqlite.test.mode=JVM-JNI-fat \
         -cp "/artifacts/ether-sqlite-jni-runtime-${VER}-fat.jar:." \
         MainJni

RUN echo "JVM JNI tests OK" > /test/jvm-jni-ok

# ── Etapa 4: build-native-ffm — GraalVM Native Image FFM (Java 25) ───────────
#
# Panama FFM estable en Java 25 — native-image sin --enable-preview.
# --initialize-at-run-time evita que la carga de la .so ocurra en build-time.
FROM ghcr.io/graalvm/native-image-community:${GRAALVM_TAG} AS build-native-ffm

ARG GRAALVM_TAG

WORKDIR /build

COPY --from=installer /artifacts/libether_sqlite_ffm_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=installer /artifacts/ /artifacts/
COPY src/MainFfm.java ./MainFfm.java

RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    FAT_JAR="/artifacts/ether-sqlite-ffm-runtime-${VER}-fat.jar"; \
    printf "\n>>> Compilando MainFfm.java como Native Image FFM (GraalVM %s)...\n\n" "${GRAALVM_TAG}"; \
    javac -cp "${FAT_JAR}" MainFfm.java; \
    native-image \
        --initialize-at-run-time=mx.rafex.ether.sqlite.SqliteLibrary \
        --enable-native-access=ALL-UNNAMED \
        --no-fallback \
        -H:Name=ether-sqlite-ffm-release-test \
        -cp "${FAT_JAR}:." \
        MainFfm; \
    printf "\n>>> Binario FFM compilado:\n"; \
    ls -lh ether-sqlite-ffm-release-test

# ── Etapa 5: build-native-jni — GraalVM Native Image JNI (Java 21 bytecode) ──
#
# GraalVM 25 compila bytecode Java 21 (class 65.0, sin preview).
# JNI no requiere --enable-native-access.
# --initialize-at-run-time evita carga de la .so en build-time.
FROM ghcr.io/graalvm/native-image-community:${GRAALVM_TAG} AS build-native-jni

ARG GRAALVM_TAG

WORKDIR /build

COPY --from=installer /artifacts/libether_sqlite_jni_runtime.so /usr/local/lib/
RUN ldconfig

COPY --from=installer /artifacts/ /artifacts/
COPY src/MainJni.java ./MainJni.java

RUN VER="$(cat /artifacts/VERSION_NUM)"; \
    FAT_JAR="/artifacts/ether-sqlite-jni-runtime-${VER}-fat.jar"; \
    printf "\n>>> Compilando MainJni.java como Native Image JNI (GraalVM %s)...\n\n" "${GRAALVM_TAG}"; \
    javac -source 21 -target 21 -cp "${FAT_JAR}" MainJni.java; \
    native-image \
        --initialize-at-run-time=mx.rafex.ether.sqlite.jni.SqliteLibraryJni \
        --no-fallback \
        -H:Name=ether-sqlite-jni-release-test \
        -cp "${FAT_JAR}:." \
        MainJni; \
    printf "\n>>> Binario JNI compilado:\n"; \
    ls -lh ether-sqlite-jni-release-test

# ── Etapa 6: test-release — ejecutar binarios nativos ────────────────────────
#
# debian:12-slim + libstdc++6 es suficiente para ejecutar binarios GraalVM.
# Las dependencias de JVM se arrastran via COPY --from=test-jvm-*.
FROM debian:12-slim AS test-release

RUN apt-get update \
 && apt-get install -y --no-install-recommends libstdc++6 \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /test

COPY --from=installer        /artifacts/libether_sqlite_ffm_runtime.so /usr/local/lib/
COPY --from=installer        /artifacts/libether_sqlite_jni_runtime.so  /usr/local/lib/
RUN ldconfig

# Arrastrar marcadores JVM: si alguna etapa JVM falló, estos COPY fallan
COPY --from=test-jvm-ffm     /test/jvm-ffm-ok                     /test/jvm-ffm-ok
COPY --from=test-jvm-jni     /test/jvm-jni-ok                     /test/jvm-jni-ok

# Binarios pre-compilados del release
COPY --from=installer        /artifacts/ether-sqlite-ffm-prebuilt  ./ether-sqlite-ffm-prebuilt
COPY --from=installer        /artifacts/ether-sqlite-jni-prebuilt  ./ether-sqlite-jni-prebuilt

# Binarios compilados en esta build
COPY --from=build-native-ffm /build/ether-sqlite-ffm-release-test  ./ether-sqlite-ffm-release-test
COPY --from=build-native-jni /build/ether-sqlite-jni-release-test  ./ether-sqlite-jni-release-test

# ── Test 3: FFM native image pre-compilado del release ────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 3/6 — Native FFM (prebuilt)       ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    ./ether-sqlite-ffm-prebuilt && \
    printf "\n✓ ether-sqlite-ffm prebuilt: OK\n"

# ── Test 4: JNI native image pre-compilado del release ────────────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 4/6 — Native JNI (prebuilt)       ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    ./ether-sqlite-jni-prebuilt && \
    printf "\n✓ ether-sqlite-jni prebuilt: OK\n"

# ── Test 5: MainFfm compilado como Native Image en esta build ─────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 5/6 — Native FFM (esta build)     ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    ./ether-sqlite-ffm-release-test && \
    printf "\n✓ ether-sqlite-ffm-release-test: OK\n"

# ── Test 6: MainJni compilado como Native Image en esta build ─────────────────
RUN printf "\n╔══════════════════════════════════════════╗\n" && \
    printf   "║  Test 6/6 — Native JNI (esta build)     ║\n" && \
    printf   "╚══════════════════════════════════════════╝\n\n" && \
    ./ether-sqlite-jni-release-test && \
    printf "\n✓ ether-sqlite-jni-release-test: OK\n"

# ── Resumen final ─────────────────────────────────────────────────────────────
RUN printf "\n"; \
    printf "╔══════════════════════════════════════════╗\n"; \
    printf "║           RESULTADO FINAL                ║\n"; \
    printf "╠══════════════════════════════════════════╣\n"; \
    printf "║  ✓ install.sh del release    — OK        ║\n"; \
    printf "║  ✓ JVM FFM fat JAR (Java 25) — OK        ║\n"; \
    printf "║  ✓ JVM JNI fat JAR (Java 21) — OK        ║\n"; \
    printf "║  ✓ Native FFM prebuilt       — OK        ║\n"; \
    printf "║  ✓ Native JNI prebuilt       — OK        ║\n"; \
    printf "║  ✓ Native FFM (esta build)   — OK        ║\n"; \
    printf "║  ✓ Native JNI (esta build)   — OK        ║\n"; \
    printf "╚══════════════════════════════════════════╝\n\n"
