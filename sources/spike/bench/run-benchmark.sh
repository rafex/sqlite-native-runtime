#!/usr/bin/env bash
# run-benchmark.sh — ejecuta los tres spikes y recoge métricas comparativas.
#
# Uso:
#   ./bench/run-benchmark.sh [10000|100000|1000000]
#
# Prerrequisitos:
#   1. Mosquitto corriendo en localhost:1883
#   2. python3 + paho-mqtt instalados
#   3. libether_sqlite_jni_runtime.so instalada (o ETHER_SQLITE_JNI_LIB definida)
#   4. GraalVM JDK 21+ (para Spike B native-image)
#   5. Rust toolchain + cargo (para Spike C)
#
# Ejecutar desde la raíz del spike:
#   cd sources/spike
#   ./bench/run-benchmark.sh 100000
set -euo pipefail

SPIKE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCH_DIR="${SPIKE_DIR}/bench"
JAVA_DIR="${SPIKE_DIR}/worker-java"
RUST_DIR="${SPIKE_DIR}/worker-rust"

COUNT="${1:-10000}"
BATCH_SIZE="${BATCH_SIZE:-500}"
FLUSH_MS="${FLUSH_MS:-200}"
MQTT_BROKER="${MQTT_BROKER:-tcp://localhost:1883}"
MQTT_TOPICS="${MQTT_TOPICS:-benchmark/#}"
MQTT_PUB_TOPIC="${MQTT_PUB_TOPIC:-benchmark/sensor}"
JNI_LIB="${ETHER_SQLITE_JNI_LIB:-/usr/local/lib/libether_sqlite_jni_runtime.so}"

RESULTS_DIR="${BENCH_DIR}/results"
mkdir -p "${RESULTS_DIR}"
REPORT="${RESULTS_DIR}/report-$(date +%Y%m%d-%H%M%S).md"

# ── colores ──────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'

info()    { echo -e "${CYAN}[bench]${NC} $*"; }
success() { echo -e "${GREEN}[bench]${NC} $*"; }
warn()    { echo -e "${YELLOW}[bench]${NC} $*"; }

# Milliseconds since epoch — portable (macOS + Linux)
# macOS 'date' does not support %3N; use python3 as fallback.
now_ms() {
    python3 -c "import time; print(int(time.time() * 1000))"
}

# ── helpers ───────────────────────────────────────────────────────────────────

check_prereqs() {
    info "Verificando prerrequisitos..."
    command -v mosquitto_pub >/dev/null 2>&1 || { warn "mosquitto-clients no encontrado"; }
    command -v python3       >/dev/null 2>&1 || { echo "ERROR: python3 requerido"; exit 1; }
    python3 -c "import paho" 2>/dev/null    || { echo "ERROR: pip install paho-mqtt"; exit 1; }

    # Verificar que Mosquitto responde
    if ! mosquitto_pub -h localhost -t "__bench_check" -m "ping" -q 0 2>/dev/null; then
        echo "ERROR: Mosquitto no responde en localhost:1883"
        echo "  Iniciar: mosquitto -v"
        exit 1
    fi
    success "Prerrequisitos OK"
}

# Mide RAM (RSS en KB) de un PID
rss_kb() {
    local pid="$1"
    if [[ -f "/proc/${pid}/status" ]]; then
        grep VmRSS "/proc/${pid}/status" | awk '{print $2}'
    else
        # macOS
        ps -o rss= -p "$pid" 2>/dev/null || echo "0"
    fi
}

# Espera a que el worker imprima "MQTT connected" (timeout 30s)
# $1 = log_file, $2 = worker PID (opcional — para detectar crash temprano)
wait_connected() {
    local log_file="$1"
    local wpid="${2:-}"
    local deadline=$(( $(date +%s) + 30 ))
    while [[ $(date +%s) -lt $deadline ]]; do
        if grep -q "MQTT connected" "${log_file}" 2>/dev/null; then
            return 0
        fi
        # Si el proceso ya murió, no esperar más
        if [[ -n "$wpid" ]] && ! kill -0 "$wpid" 2>/dev/null; then
            echo "ERROR: worker PID=${wpid} terminó antes de conectar"
            return 1
        fi
        sleep 0.2
    done
    echo "ERROR: worker no conectó en 30s"
    return 1
}

# Extrae métricas del log del worker.
# Soporta dos formatos:
#   Java:  [metrics] ... committed=N ... tps=T/s last_commit=Lms max_commit=Mms
#   Rust:  [inserter] committed=N ... tps=T/s commit=Cms
#          [subscriber] received=R dropped=D tps=T/s   (used for Rust final tps)
parse_metrics() {
    local log="$1"
    local committed tps last_commit max_commit
    # Java format: last [metrics] line
    local last_java; last_java=$(grep "\[metrics\]" "${log}" | tail -1)
    # Rust format: last [inserter] line for committed/tps, [subscriber] for received tps
    local last_rust; last_rust=$(grep "\[inserter\]" "${log}" | tail -1)
    local last_subscriber; last_subscriber=$(grep "\[subscriber\]" "${log}" | tail -1)

    if [[ -n "$last_java" ]]; then
        # Java worker
        committed=$(  echo "$last_java" | grep -oE 'committed=[0-9]+' | grep -oE '[0-9]+' || echo "0")
        tps=$(        echo "$last_java" | grep -oE 'tps=[0-9]+' | grep -oE '[0-9]+' || echo "0")
        last_commit=$(echo "$last_java" | grep -oE 'last_commit=[0-9]+' | grep -oE '[0-9]+' || echo "0")
        max_commit=$( echo "$last_java" | grep -oE 'max_commit=[0-9]+' | grep -oE '[0-9]+' || echo "0")
    elif [[ -n "$last_rust" ]]; then
        # Rust worker
        committed=$(  echo "$last_rust" | grep -oE 'committed=[0-9]+' | grep -oE '[0-9]+' || echo "0")
        tps=$(        echo "$last_rust" | grep -oE 'tps=[0-9]+' | grep -oE '[0-9]+' || echo "0")
        last_commit=$(echo "$last_rust" | grep -oE 'commit=[0-9]+' | grep -oE '[0-9]+' | head -1 || echo "0")
        max_commit="$last_commit"
    fi

    committed="${committed:-0}"
    tps="${tps:-0}"
    last_commit="${last_commit:-0}"
    max_commit="${max_commit:-0}"

    echo "${committed} ${tps} ${last_commit} ${max_commit}"
}

# Ejecuta un spike completo: start → publish → collect → stop
run_spike() {
    local name="$1"
    local cmd="$2"          # comando para iniciar el worker
    local log_file="${RESULTS_DIR}/${name}-$(date +%H%M%S).log"
    # Resolve real path — macOS /tmp is a symlink to /private/tmp and
    # snr_open forces SQLITE_OPEN_NOFOLLOW which rejects symlinked paths.
    local db_file
    db_file="$(python3 -c "import os,sys; print(os.path.realpath('/tmp/snr-spike-${name}.db'))")"

    info "──────────────────────────────────────────"
    info "Spike: ${name}  (count=${COUNT})"
    # Kill any leftover worker from a previous failed run and clean its DB files.
    rm -f "${db_file}" "${db_file}-wal" "${db_file}-shm"

    # 1. Arrancar worker
    local start_epoch=$(now_ms)
    SQLITE_DB="${db_file}" \
    BATCH_SIZE="${BATCH_SIZE}" \
    FLUSH_MS="${FLUSH_MS}" \
    MQTT_BROKER="${MQTT_BROKER}" \
    MQTT_TOPICS="${MQTT_TOPICS}" \
    ETHER_SQLITE_JNI_LIB="${JNI_LIB}" \
        eval "$cmd" > "${log_file}" 2>&1 &
    local worker_pid=$!
    info "Worker PID=${worker_pid}"

    # 2. Esperar conexión MQTT
    if ! wait_connected "${log_file}" "${worker_pid}"; then
        kill "${worker_pid}" 2>/dev/null
        warn "SKIP ${name}: worker no conectó"
        return
    fi
    local connected_epoch=$(now_ms)
    local startup_ms=$(( connected_epoch - start_epoch ))
    info "Connected — startup=${startup_ms}ms"

    # 3. Medir RAM antes de carga
    local rss_idle
    rss_idle=$(rss_kb "${worker_pid}")

    # 4. Publicar N mensajes
    info "Publicando ${COUNT} mensajes..."
    # Strip tcp:// scheme and :port — publisher uses MQTT_BROKER as hostname only.
    local broker_host="${MQTT_BROKER##tcp://}"   # remove tcp://
    broker_host="${broker_host%%:*}"              # remove :port
    MQTT_BROKER="${broker_host}" \
    MQTT_PUB_TOPIC="${MQTT_PUB_TOPIC}" \
    MQTT_PUB_QOS="1" \
        python3 "${BENCH_DIR}/publisher.py" "${COUNT}"

    # 5. Esperar que el worker procese (flush_ms × 3 + margen de 3s mínimo)
    # Use ceiling division to avoid 0 for small FLUSH_MS values.
    local wait_s=$(( (FLUSH_MS * 3 + 999) / 1000 + 3 ))
    info "Esperando flush (${wait_s}s)..."
    sleep "${wait_s}"

    # 6. Medir RAM bajo carga
    local rss_load
    rss_load=$(rss_kb "${worker_pid}")

    # 7. Parar worker con SIGTERM, esperar hasta 10s, luego SIGKILL
    kill -TERM "${worker_pid}" 2>/dev/null
    local kill_deadline=$(( $(date +%s) + 10 ))
    while kill -0 "${worker_pid}" 2>/dev/null && [[ $(date +%s) -lt $kill_deadline ]]; do
        sleep 0.5
    done
    kill -KILL "${worker_pid}" 2>/dev/null
    wait "${worker_pid}" 2>/dev/null || true
    local end_epoch=$(now_ms)
    local total_s=$(( (end_epoch - start_epoch) / 1000 ))

    # 8. Parsear métricas del log
    local metrics
    metrics=$(parse_metrics "${log_file}")
    local committed tps last_commit_ms max_commit_ms
    read -r committed tps last_commit_ms max_commit_ms <<< "$metrics"

    # 9. Tamaño del binario
    local binary_size
    case "$name" in
        spike-a-jar)     binary_size=$(ls -lh "${JAVA_DIR}/target/"*-fat.jar 2>/dev/null | awk '{print $5}' | head -1) ;;
        spike-b-native)  binary_size=$(ls -lh "${JAVA_DIR}/target/snr-mqtt-worker"         2>/dev/null | awk '{print $5}' | head -1) ;;
        spike-c-rust)    binary_size=$(ls -lh "${RUST_DIR}/target/release/snr-mqtt-worker"  2>/dev/null | awk '{print $5}' | head -1) ;;
    esac
    binary_size="${binary_size:-N/A}"

    success "Spike ${name} completado:"
    echo "  committed   = ${committed}"
    echo "  tps         = ${tps}/s"
    echo "  startup     = ${startup_ms}ms"
    echo "  rss_idle    = ${rss_idle}KB"
    echo "  rss_load    = ${rss_load}KB"
    echo "  last_commit = ${last_commit_ms}ms"
    echo "  max_commit  = ${max_commit_ms}ms"
    echo "  binary_size = ${binary_size}"
    echo "  log         = ${log_file}"

    # Guardar para el reporte
    eval "RESULT_${name//-/_}_committed=${committed}"
    eval "RESULT_${name//-/_}_tps=${tps}"
    eval "RESULT_${name//-/_}_startup=${startup_ms}"
    eval "RESULT_${name//-/_}_rss_idle=${rss_idle}"
    eval "RESULT_${name//-/_}_rss_load=${rss_load}"
    eval "RESULT_${name//-/_}_last_commit=${last_commit_ms}"
    eval "RESULT_${name//-/_}_max_commit=${max_commit_ms}"
    eval "RESULT_${name//-/_}_binary=${binary_size}"
}

# ── build ─────────────────────────────────────────────────────────────────────

build_java_jar() {
    info "Build Spike A (Java JAR)..."
    # Instalar dependencias locales si no están en ~/.m2
    local repo_root
    repo_root="$(cd "${SPIKE_DIR}/../.." && pwd)"
    "${repo_root}/mvnw" install -f "${repo_root}/sources/java/ether-sqlite-core/pom.xml"        -DskipTests -Djacoco.skip=true -q >/dev/null 2>&1 || true
    "${repo_root}/mvnw" install -f "${repo_root}/sources/java/ether-sqlite-jni-runtime/pom.xml" -DskipTests -Djacoco.skip=true -q >/dev/null 2>&1 || true
    "${repo_root}/mvnw" package -f "${JAVA_DIR}/pom.xml" -DskipTests -Djacoco.skip=true -q 2>/dev/null
    success "Spike A JAR listo: $(ls -lh "${JAVA_DIR}/target/"*-fat.jar | awk '{print $5, $9}')"
}

build_java_native() {
    info "Build Spike B (native-image)..."
    local repo_root
    repo_root="$(cd "${SPIKE_DIR}/../.." && pwd)"
    if ! command -v native-image >/dev/null 2>&1; then
        warn "native-image no encontrado — skipping Spike B"
        return 1
    fi
    "${repo_root}/mvnw" package -f "${JAVA_DIR}/pom.xml" -Pnative -DskipTests -Djacoco.skip=true -q 2>/dev/null
    success "Spike B nativo listo: $(ls -lh "${JAVA_DIR}/target/snr-mqtt-worker" | awk '{print $5, $9}')"
}

build_rust() {
    info "Build Spike C (Rust)..."
    if ! command -v cargo >/dev/null 2>&1; then
        warn "cargo no encontrado — skipping Spike C"
        return 1
    fi
    cargo build --release --manifest-path "${RUST_DIR}/Cargo.toml" 2>/dev/null
    success "Spike C Rust listo: $(ls -lh "${RUST_DIR}/target/release/snr-mqtt-worker" | awk '{print $5, $9}')"
}

# ── reporte final ─────────────────────────────────────────────────────────────

write_report() {
    cat > "${REPORT}" << EOF
# Benchmark Spike — MQTT → SQLite

**Fecha**: $(date)
**Mensajes**: ${COUNT}
**Batch size**: ${BATCH_SIZE}
**Flush timeout**: ${FLUSH_MS}ms
**Broker**: ${MQTT_BROKER}

## Resultados

| Métrica | Spike A (JAR) | Spike B (native-image) | Spike C (Rust) |
|---|---|---|---|
| Committed (msgs) | ${RESULT_spike_a_jar_committed:-?} | ${RESULT_spike_b_native_committed:-?} | ${RESULT_spike_c_rust_committed:-?} |
| Throughput (msg/s) | ${RESULT_spike_a_jar_tps:-?} | ${RESULT_spike_b_native_tps:-?} | ${RESULT_spike_c_rust_tps:-?} |
| Startup (ms) | ${RESULT_spike_a_jar_startup:-?} | ${RESULT_spike_b_native_startup:-?} | ${RESULT_spike_c_rust_startup:-?} |
| RAM idle (KB) | ${RESULT_spike_a_jar_rss_idle:-?} | ${RESULT_spike_b_native_rss_idle:-?} | ${RESULT_spike_c_rust_rss_idle:-?} |
| RAM carga (KB) | ${RESULT_spike_a_jar_rss_load:-?} | ${RESULT_spike_b_native_rss_load:-?} | ${RESULT_spike_c_rust_rss_load:-?} |
| last commit (ms) | ${RESULT_spike_a_jar_last_commit:-?} | ${RESULT_spike_b_native_last_commit:-?} | ${RESULT_spike_c_rust_last_commit:-?} |
| max commit (ms) | ${RESULT_spike_a_jar_max_commit:-?} | ${RESULT_spike_b_native_max_commit:-?} | ${RESULT_spike_c_rust_max_commit:-?} |
| Binary size | ${RESULT_spike_a_jar_binary:-?} | ${RESULT_spike_b_native_binary:-?} | ${RESULT_spike_c_rust_binary:-?} |
| Piezas despliegue | JVM + .so + JAR | binario + .so | binario único |

## Decisión

<!-- Completar después de revisar los resultados -->

- [ ] Throughput Java >= 80% de Rust → Java native-image (Spike B)
- [ ] Throughput Java < 80% de Rust Y lógica mínima → Rust (Spike C)
- [ ] Se necesita lógica de dominio Java → Java sin importar throughput

**Decisión**: _______________

**Justificación**: _______________
EOF
    success "Reporte guardado: ${REPORT}"
    cat "${REPORT}"
}

# ── main ─────────────────────────────────────────────────────────────────────

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  MQTT → SQLite Spike Benchmark                       ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""
info "Parámetros: count=${COUNT} batch=${BATCH_SIZE} flush=${FLUSH_MS}ms"
echo ""

check_prereqs

# Build
build_java_jar
SPIKE_B_OK=true; build_java_native || SPIKE_B_OK=false
SPIKE_C_OK=true; build_rust        || SPIKE_C_OK=false
echo ""

# Spike A — Java JAR
FAT_JAR="${JAVA_DIR}/target/ether-sqlite-mqtt-worker-spike-0.1.0-SNAPSHOT-fat.jar"
run_spike "spike-a-jar" \
    "java -Dether.sqlite.jni.lib='${JNI_LIB}' -jar '${FAT_JAR}'"
sleep 5

# Spike B — Java native-image
if [[ "$SPIKE_B_OK" == "true" ]]; then
    NATIVE_BIN="${JAVA_DIR}/target/snr-mqtt-worker"
    run_spike "spike-b-native" \
        "ETHER_SQLITE_JNI_LIB='${JNI_LIB}' '${NATIVE_BIN}'"
    sleep 5
fi

# Spike C — Rust
if [[ "$SPIKE_C_OK" == "true" ]]; then
    RUST_BIN="${RUST_DIR}/target/release/snr-mqtt-worker"
    run_spike "spike-c-rust" "'${RUST_BIN}'"
fi

echo ""
write_report
