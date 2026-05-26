# PLAN_WORKER — Plan de ejecución fase por fase

Worker MQTT → SQLite para el ecosistema ether-sqlite.

> **Premisa**: no se compromete la tecnología de implementación hasta terminar la Fase 0.
> La decisión (Java JAR / Java native-image / Rust) se toma con datos del spike.

---

## Resumen de fases

| Fase | Objetivo | Entregable | Prerrequisito |
|---|---|---|---|
| **0** | Spike comparativo | Informe de benchmark con decisión | ninguno |
| **1** | Implementación core | Módulo/crate funcional + tests | Decisión Fase 0 |
| **2** | Validación de rendimiento | Benchmark reproducible | Fase 1 |
| **3** | Compilación nativa + CI | Binarios amd64 + arm64 en GHCR | Fase 2 OK |
| **4** | Release integration | Binarios en GitHub Release + install.sh | Fase 3 |
| **5** | Operaciones | systemd unit, docs, configuración | Fase 4 |

---

## Fase 0 — Spike comparativo

**Objetivo**: decidir la tecnología con datos, no con opiniones.

**Duración estimada**: 1-2 días de trabajo.

### 0.1 — Entorno de prueba

Requisitos mínimos para el spike:
- Mosquitto local en loopback (`localhost:1883`)
- SQLite en `/tmp/bench.db` (tmpfs o SSD)
- WAL activado, `PRAGMA busy_timeout=5000`
- Payload JSON realista: entre 50 y 200 bytes

```json
{
  "sensor_id": "T-42",
  "ts": 1718000000000,
  "temperature": 23.7,
  "humidity": 61.2,
  "location": "sala-a"
}
```

### 0.2 — Las tres implementaciones del spike

Cada implementación es mínima pero representativa. No optimizar antes de medir.

#### Spike A — Java JAR (baseline)

```
sources/spike/worker-java-jar/
  pom.xml               Java 21, HiveMQ client, ether-sqlite-jni-runtime
  src/main/java/.../
    WorkerSpike.java    subscriber + BlockingQueue + batch INSERT
```

Ejecutar con:
```sh
java -Dether.sqlite.jni.lib=/usr/local/lib/libether_sqlite_jni_runtime.so \
     -jar worker-spike.jar
```

#### Spike B — Java native-image

Mismo código que Spike A, compilado con GraalVM native-image:
```sh
native-image -jar worker-spike.jar \
  --no-fallback \
  -o worker-java-native
```

Ejecutar con:
```sh
ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so ./worker-java-native
```

#### Spike C — Rust

```
sources/spike/worker-rust/
  Cargo.toml   deps: rumqttc, ether-sqlite-core (path)
  src/main.rs  subscriber + channel mpsc + batch INSERT via core
```

Ejecutar con:
```sh
cargo run --release
```

Nota: `ether-sqlite-core` con `libsqlite3-sys` feature `bundled` → binario único, sin .so externa.

### 0.3 — Métricas a medir

Para cada spike, medir con los tres volúmenes:

| Volumen | Batch size | Flush timeout |
|---|---|---|
| 10 000 mensajes | 100 | 200 ms |
| 100 000 mensajes | 500 | 200 ms |
| 1 000 000 mensajes | 1 000 | 500 ms |

**Métricas por prueba:**

| Métrica | Herramienta |
|---|---|
| Throughput (msg/s) | contador en el worker |
| Latencia P50/P95/P99 (msg recv → commit) | timestamps en payload |
| RSS (RAM en steady state) | `/proc/{pid}/status` o `ps` |
| Startup time | `time ./worker` hasta primer mensaje procesado |
| Binary size | `ls -lh` |
| Piezas en despliegue | conteo manual |

### 0.4 — Plataformas

Mínimo en **amd64** (obligatorio). Idealmente también en **arm64** (Raspberry Pi 4 o runner arm64).

### 0.5 — Informe y criterio de decisión

Al terminar el spike, completar esta tabla:

| Métrica | Java JAR | Java native | Rust |
|---|---|---|---|
| Throughput 1M / batch 1000 | ? msg/s | ? msg/s | ? msg/s |
| Latencia P99 | ? ms | ? ms | ? ms |
| RSS steady state | ? MB | ? MB | ? MB |
| Startup time | ? ms | ? ms | ? ms |
| Binary size | ? MB | ? MB | ? MB |
| Piezas en despliegue | 3 (JVM+.so+JAR) | 2 (bin+.so) | 1 (bin) |

**Regla de decisión:**

```
Si throughput de Java JAR >= 80% de Rust:
  → Java native-image (mejor distribución, misma lógica, mismo ecosystem)
  → Opción A (Java 25 FFM) o B (Java 21 JNI) según LTS target

Si throughput de Java JAR < 80% de Rust Y la lógica de dominio es mínima:
  → Rust worker (binario único, footprint mínimo)

Si el requisito es lógica de dominio Java compleja (parsers, transformaciones, etc.):
  → Java sin importar el throughput (mantenibilidad > micro-optimización)
```

---

## Fase 1 — Implementación core

> Inicia después de la decisión de Fase 0.

### Camino Java (Opción A o B)

#### 1A — Módulo Maven

```
sources/java/ether-sqlite-mqtt-worker/
  pom.xml
  src/main/java/mx/rafex/ether/sqlite/worker/
    WorkerConfig.java       config: broker, topics, db path, batch, flush
    MqttSqliteWorker.java   main: subscriber + inserter con virtual threads
    BatchInserter.java      flush a SQLite (BEGIN → N inserts → COMMIT)
    TopicRouter.java        mapeo topic → tabla SQLite (configurable)
    WorkerMetrics.java      contadores: msgs recv, batches, errores
  src/test/java/.../
    BatchInserterTest.java  unit: insert en memoria, rollback en error
    WorkerConfigTest.java   unit: parseo de env vars y config file
```

#### 1B — `pom.xml` base

```xml
<groupId>mx.rafex.ether</groupId>
<artifactId>ether-sqlite-mqtt-worker</artifactId>
<version>${project.version}</version>

<!-- Java 21 LTS si Opción B, Java 25 si Opción A -->
<properties>
  <maven.compiler.release>21</maven.compiler.release>
</properties>

<dependencies>
  <!-- binding SQLite -->
  <dependency>
    <groupId>mx.rafex.ether</groupId>
    <artifactId>ether-sqlite-jni-runtime</artifactId>  <!-- o ffm-runtime -->
  </dependency>
  <!-- MQTT client -->
  <dependency>
    <groupId>com.hivemq</groupId>
    <artifactId>hivemq-mqtt-client</artifactId>
    <version>1.3.3</version>
  </dependency>
</dependencies>
```

#### 1C — Esquema SQLite mínimo

```sql
-- Tabla de ingesta genérica (configurable por topic → tabla)
CREATE TABLE IF NOT EXISTS mqtt_messages (
    id        INTEGER PRIMARY KEY,
    topic     TEXT    NOT NULL,
    payload   TEXT,           -- JSON o raw text
    received  INTEGER NOT NULL -- epoch ms
);
CREATE INDEX IF NOT EXISTS idx_mqtt_topic ON mqtt_messages(topic);
CREATE INDEX IF NOT EXISTS idx_mqtt_received ON mqtt_messages(received);
```

#### 1D — Configuración por variables de entorno

| Variable | Default | Descripción |
|---|---|---|
| `MQTT_BROKER` | `tcp://localhost:1883` | URL del broker |
| `MQTT_CLIENT_ID` | `snr-worker-{hostname}` | Client ID MQTT |
| `MQTT_TOPICS` | `#` | Topics separados por coma (acepta wildcards) |
| `MQTT_QOS` | `1` | QoS: 0, 1 o 2 |
| `MQTT_USERNAME` | *(vacío)* | Usuario MQTT |
| `MQTT_PASSWORD` | *(vacío)* | Contraseña MQTT |
| `SQLITE_DB` | `/var/lib/snr/mqtt.db` | Ruta del archivo SQLite |
| `BATCH_SIZE` | `500` | Máximo mensajes por transacción |
| `FLUSH_MS` | `200` | Máximo ms de espera antes de hacer flush |
| `ETHER_SQLITE_JNI_LIB` | *(auto-detectado)* | Ruta a libether_sqlite_jni_runtime.so |

### Camino Rust (Opción C)

#### 1E — Crate en el workspace

```
sources/rust/
  Cargo.toml        ← añadir "ether-sqlite-mqtt-worker" a members
  ether-sqlite-mqtt-worker/
    Cargo.toml      deps: ether-sqlite-core, rumqttc, tokio
    src/
      main.rs       config + subscriber + batch inserter
      config.rs     env vars → Config struct
      inserter.rs   batch loop con ether_sqlite_core
      schema.rs     CREATE TABLE IF NOT EXISTS
```

#### 1F — `Cargo.toml` del worker Rust

```toml
[package]
name    = "ether-sqlite-mqtt-worker"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ether-sqlite-mqtt-worker"

[dependencies]
ether-sqlite-core = { path = "../ether-sqlite-core" }
rumqttc            = "0.24"
tokio              = { version = "1", features = ["full"] }
serde              = { version = "1", features = ["derive"] }
serde_json         = "1"
```

---

## Fase 2 — Validación de rendimiento

Con la implementación real (no el spike), repetir el benchmark de Fase 0:

- Mismos volúmenes: 10k / 100k / 1M mensajes
- Mismos batch sizes: 100 / 500 / 1000
- Mismas métricas: throughput, latencia, RAM, startup
- Comparar contra baseline del spike

**Criterio de pase:** throughput estable (sin degradación en 1M), P99 < 10 ms de latencia de commit,
sin memory leaks en `/proc/{pid}/status` tras 30 min.

---

## Fase 3 — Compilación nativa + CI

### 3.1 — Jobs nuevos en `publish.yml`

#### Camino Java

```yaml
build-native-mqtt-amd64:
  name: "Worker MQTT — native amd64"
  runs-on: ubuntu-latest
  needs: [build-rust-jni, build-java-fat]
  steps:
    - uses: actions/checkout@v4
      with: { ref: "${{ env.RELEASE_TAG }}" }
    - name: Setup GraalVM JDK 21
      uses: graalvm/setup-graalvm@v1
      with: { java-version: '21', distribution: 'graalvm' }
    - name: Download JNI .so
      # descargar libether_sqlite_jni_runtime-linux-amd64.so de GHCR
    - name: Build worker native
      run: ./mvnw -Pnative package -f sources/java/ether-sqlite-mqtt-worker/pom.xml
    - name: Publicar en GHCR con ORAS
      run: |
        oras push ghcr.io/${{ github.repository_owner }}/ether-sqlite-mqtt-worker:${EFF_FAT}-amd64 \
          ether-sqlite-mqtt-worker-linux-amd64.bin

build-native-mqtt-arm64:
  name: "Worker MQTT — native arm64"
  runs-on: ubuntu-24.04-arm
  needs: [build-rust-jni, build-java-fat]
  # igual que amd64
```

#### Camino Rust

```yaml
build-worker-rust-amd64:
  name: "Worker MQTT Rust — amd64"
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Install cargo-zigbuild
      run: pip install ziglang && cargo install cargo-zigbuild
    - name: Build worker
      run: cargo zigbuild -p ether-sqlite-mqtt-worker --release \
             --target x86_64-unknown-linux-gnu.2.17
    - name: Publicar en GHCR
      run: oras push ghcr.io/${{ github.repository_owner }}/ether-sqlite-mqtt-worker:${RUST_V}-amd64 \
             target/x86_64-unknown-linux-gnu/release/ether-sqlite-mqtt-worker

build-worker-rust-arm64:
  # igual con aarch64-unknown-linux-gnu.2.17
```

### 3.2 — Actualizar `VERSIONS`

```
# Si el worker tiene ciclo de release independiente:
MQTT_WORKER=v0.1.0

# Si sigue el ciclo general (recomendado para empezar):
# usa JAVA_FAT o RUST como versión
```

### 3.3 — Actualizar `release.yml`

Añadir descarga de los dos binarios del worker desde GHCR:

```bash
oras pull "${REG}/${OWNER}/ether-sqlite-mqtt-worker:${WORKER_V}-amd64"
oras pull "${REG}/${OWNER}/ether-sqlite-mqtt-worker:${WORKER_V}-arm64"
```

---

## Fase 4 — Release integration

### 4.1 — Artefactos en GitHub Release

Nuevos archivos en el release:

```
ether-sqlite-mqtt-worker-linux-amd64.bin
ether-sqlite-mqtt-worker-linux-amd64.bin.sha256
ether-sqlite-mqtt-worker-linux-arm64.bin
ether-sqlite-mqtt-worker-linux-arm64.bin.sha256
```

No requiere cambios en `release.yml` si los binarios ya están en `artifacts/` antes del
`gh release create artifacts/*`.

### 4.2 — Actualizar `install.sh`

Añadir modo de instalación para el worker:

```sh
# Detectar si el usuario quiere instalar el worker
if [[ "${SNR_INSTALL_WORKER:-0}" == "1" ]]; then
  download_and_verify "ether-sqlite-mqtt-worker-linux-${ARCH}.bin"
  install_binary "ether-sqlite-mqtt-worker" "/usr/local/bin/"
  install_systemd_unit
fi
```

### 4.3 — Unidad systemd

```ini
# /etc/systemd/system/snr-mqtt-worker.service
[Unit]
Description=ether-sqlite MQTT Worker
After=network.target mosquitto.service
Wants=mosquitto.service

[Service]
Type=simple
User=snr
EnvironmentFile=/etc/snr/worker.env
ExecStart=/usr/local/bin/ether-sqlite-mqtt-worker
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

---

## Fase 5 — Operaciones

### 5.1 — Logging estructurado

El worker debe emitir logs en formato JSON con al menos:

```json
{ "ts": "2026-05-26T10:00:00Z", "level": "INFO",
  "event": "batch_committed", "count": 500, "duration_ms": 12, "lag_ms": 3 }
{ "ts": "2026-05-26T10:00:01Z", "level": "WARN",
  "event": "mqtt_reconnect", "attempt": 2, "broker": "tcp://localhost:1883" }
{ "ts": "2026-05-26T10:00:05Z", "level": "ERROR",
  "event": "batch_failed", "error": "disk full", "batch_size": 500 }
```

### 5.2 — Métricas de operación (stdout o endpoint HTTP)

| Métrica | Tipo | Descripción |
|---|---|---|
| `msgs_received_total` | counter | Mensajes recibidos de MQTT |
| `msgs_committed_total` | counter | Mensajes escritos en SQLite |
| `batches_total` | counter | Commits realizados |
| `batch_size_avg` | gauge | Tamaño medio del batch último minuto |
| `commit_duration_p99_ms` | gauge | Latencia P99 de commit |
| `mqtt_reconnects_total` | counter | Reconexiones al broker |
| `queue_depth` | gauge | Mensajes en el buffer pendientes de flush |

### 5.3 — Graceful shutdown

Al recibir `SIGTERM`:
1. Dejar de consumir nuevos mensajes del broker (unsubscribe)
2. Hacer flush del buffer actual (commit del batch pendiente)
3. Cerrar la conexión SQLite limpiamente (WAL checkpoint TRUNCATE)
4. Cerrar conexión MQTT
5. Exit 0

### 5.4 — Documentación operacional

Crear `docs/WORKER.md` con:
- Requisitos de sistema
- Instalación con install.sh
- Configuración completa de variables de entorno
- Ejemplos de uso: sensor IoT, telemetría, logs de aplicación
- Integración con systemd
- Troubleshooting: broker no disponible, disco lleno, permisos .so

---

## Checklist global

### Fase 0
- [ ] Spike A: Java JAR — código mínimo + publisher de prueba
- [ ] Spike B: Java native-image — misma base compilada
- [ ] Spike C: Rust — rumqttc + ether-sqlite-core
- [ ] Benchmark 10k / 100k / 1M en amd64
- [ ] Benchmark en arm64 (si disponible)
- [ ] Tabla de resultados completada
- [ ] Decisión documentada con justificación

### Fase 1
- [ ] Estructura de módulo/crate creada
- [ ] Configuración por env vars implementada
- [ ] Subscriber MQTT funcional (conecta, suscribe, recibe)
- [ ] Batch inserter funcional (drainTo + transacción)
- [ ] Schema SQLite aplicado al arranque
- [ ] Tests unitarios: inserter + config (sin MQTT)
- [ ] Tests de integración: con Mosquitto en Docker

### Fase 2
- [ ] Benchmark reproducible documentado
- [ ] Sin degradación en 1M mensajes
- [ ] P99 latencia commit < 10 ms (WAL en SSD)
- [ ] Sin memory leaks en 30 min de ejecución

### Fase 3
- [ ] Job `build-worker-amd64` en `publish.yml`
- [ ] Job `build-worker-arm64` en `publish.yml`
- [ ] Binario publicado en GHCR como OCI artifact
- [ ] `VERSIONS` actualizado (clave `MQTT_WORKER` o integrada)

### Fase 4
- [ ] Binarios en GitHub Release con SHA256
- [ ] `install.sh` actualizado con flag `SNR_INSTALL_WORKER`
- [ ] Unidad systemd incluida en install.sh
- [ ] `release.yml` descarga y publica worker binaries

### Fase 5
- [ ] Logging estructurado JSON
- [ ] Graceful shutdown con flush + WAL checkpoint
- [ ] `docs/WORKER.md` completo
- [ ] Métricas básicas en stdout (o endpoint `/metrics`)
