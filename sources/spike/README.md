# Spike — Worker MQTT → SQLite (Fase 0)

Comparativa de tres implementaciones del mismo worker para decidir la tecnología final.
Ver [PLAN_WORKER.md](../../docs/PLAN_WORKER.md) para el contexto completo.

| Spike | Tecnología | Binario | Dependencias runtime |
|---|---|---|---|
| **A** | Java 21 JAR | fat JAR | JVM + libether_sqlite_jni_runtime.so |
| **B** | Java 21 + GraalVM native-image | binario nativo | libether_sqlite_jni_runtime.so |
| **C** | Rust (rumqttc + rusqlite bundled) | binario nativo | ninguna |

---

## Prerrequisitos

```sh
# 1. Mosquitto corriendo localmente
mosquitto -v                        # o: sudo systemctl start mosquitto

# 2. Python 3 + paho-mqtt (para el publisher)
pip install paho-mqtt

# 3. JDK 21+ (Spike A)
java -version                       # >= 21

# 4. GraalVM JDK 21 con native-image (Spike B)
native-image --version

# 5. Rust stable + cargo (Spike C)
cargo --version

# 6. libether_sqlite_jni_runtime.so instalada
# Opción A: con install.sh desde el release
curl -sS .../install.sh | sh
# Opción B: compilada localmente
cargo build -p ether-sqlite-jni --release --manifest-path ../../rust/Cargo.toml
export ETHER_SQLITE_JNI_LIB=../../rust/target/release/libether_sqlite_jni_runtime.so
```

---

## Instalar dependencias Java en ~/.m2

```sh
# Desde la raíz del proyecto
./mvnw install -f sources/java/ether-sqlite-core/pom.xml        -DskipTests
./mvnw install -f sources/java/ether-sqlite-jni-runtime/pom.xml -DskipTests
```

---

## Ejecutar el benchmark completo

```sh
cd sources/spike

# 10 000 mensajes (smoke test, ~30s)
./bench/run-benchmark.sh 10000

# 100 000 mensajes (~2-3 min)
./bench/run-benchmark.sh 100000

# 1 000 000 mensajes (~15-30 min)
./bench/run-benchmark.sh 1000000
```

El script:
1. Compila los tres workers
2. Para cada uno: arranca, publica N mensajes, mide métricas, para
3. Genera `bench/results/report-{timestamp}.md` con la tabla comparativa

---

## Ejecutar cada spike manualmente

### Spike A — Java JAR

```sh
cd worker-java
../../mvnw package -DskipTests   # genera target/*-fat.jar

ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so \
SQLITE_DB=/tmp/test.db \
BATCH_SIZE=500 \
FLUSH_MS=200 \
MQTT_TOPICS=benchmark/# \
  java -jar target/ether-sqlite-mqtt-worker-spike-0.1.0-SNAPSHOT-fat.jar
```

### Spike B — Java native-image

```sh
cd worker-java
../../mvnw -Pnative package -DskipTests   # genera target/snr-mqtt-worker

ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so \
SQLITE_DB=/tmp/test.db \
  ./target/snr-mqtt-worker
```

### Spike C — Rust

```sh
cd worker-rust
cargo build --release

SQLITE_DB=/tmp/test.db \
BATCH_SIZE=500 \
FLUSH_MS=200 \
  ./target/release/snr-mqtt-worker
```

---

## Publicar mensajes de prueba

```sh
# 10 000 mensajes en topic benchmark/sensor
cd sources/spike
python3 bench/publisher.py 10000

# Opciones
MQTT_BROKER=localhost \
MQTT_PORT=1883 \
MQTT_PUB_TOPIC=benchmark/sensor \
MQTT_PUB_QOS=1 \
PAYLOAD_SIZE=150 \
  python3 bench/publisher.py 100000
```

---

## Tests unitarios (Spike A/B)

```sh
cd worker-java
ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so \
  ../../mvnw test
```

---

## Nota: tabla del spike vs. tabla de producción

El spike usa `mqtt_messages` (tabla plana) para maximizar la simplicidad del benchmark.
La tabla de producción será `ingest_event` con campos estructurados por topic:

```
topic: db/{priority}/{tenant}/{database}/{entity}/{operation}
campos: tenant, database_name, entity, operation, priority, schema_name, payload, metadata
```

El throughput del spike es representativo del throughput real porque el cuello de botella
es el batch `INSERT + COMMIT`, no el parsing del topic (O(1) split por `/`).
Ver [WORKER_IDEA.md — Diseño del protocolo MQTT](../../docs/WORKER_IDEA.md) para el diseño completo.

---

## Métricas recogidas

| Métrica | Cómo se mide |
|---|---|
| Throughput (msg/s) | msgs committed / elapsed (desde los logs del worker) |
| Startup (ms) | tiempo de JVM start a "MQTT connected" |
| RAM idle (KB) | RSS del proceso antes de empezar a recibir mensajes |
| RAM carga (KB) | RSS del proceso durante la ingesta |
| last commit (ms) | duración de la última transacción |
| max commit (ms) | duración máxima de transacción en toda la sesión |
| Binary size | tamaño del artefacto distribuible |
| Piezas despliegue | cuántos archivos hay que copiar al servidor |

---

## Criterio de decisión

Ver [WORKER_IDEA.md](../../docs/WORKER_IDEA.md#decisión-cómo-elegir-entre-las-opciones).

```
Si throughput(Java) >= 80% de throughput(Rust):
  → Java native-image (Spike B) — mejor distribución, mismo ecosystem
Else si lógica de dominio es mínima:
  → Rust (Spike C) — binario único, footprint mínimo
Else:
  → Java sin importar throughput — mantenibilidad > micro-optimización
```
