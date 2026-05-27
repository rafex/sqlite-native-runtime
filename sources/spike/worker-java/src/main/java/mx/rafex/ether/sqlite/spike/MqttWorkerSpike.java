package mx.rafex.ether.sqlite.spike;

import com.hivemq.client.mqtt.MqttGlobalPublishFilter;
import com.hivemq.client.mqtt.datatypes.MqttQos;
import com.hivemq.client.mqtt.mqtt3.Mqtt3AsyncClient;
import com.hivemq.client.mqtt.mqtt3.Mqtt3Client;
import mx.rafex.ether.sqlite.JniSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

/**
 * Spike A / Spike B — worker MQTT → SQLite.
 *
 * <h2>Spike A (JAR)</h2>
 * <pre>
 *   java -Dether.sqlite.jni.lib=/usr/local/lib/libether_sqlite_jni_runtime.so \
 *        -jar target/ether-sqlite-mqtt-worker-spike-0.1.0-SNAPSHOT-fat.jar
 * </pre>
 *
 * <h2>Spike B (native-image)</h2>
 * <pre>
 *   mvn -Pnative package
 *   ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so ./target/snr-mqtt-worker
 * </pre>
 *
 * <h2>Variables de entorno</h2>
 * Ver {@link WorkerConfig#fromEnv()}.
 */
public final class MqttWorkerSpike {

    public static void main(String[] args) throws Exception {

        long jvmStart = System.currentTimeMillis();

        WorkerConfig config = WorkerConfig.fromEnv();
        System.err.println("[worker] config = " + config);

        // ── 1. SQLite ─────────────────────────────────────────────────────────
        SqliteConnection db = JniSqliteConnection.open(config.sqliteDb());
        db.enableWal().busyTimeout(5_000);
        initSchema(db);

        // ── 2. Buffer + métricas + inserter ───────────────────────────────────
        BatchBuffer    buffer   = new BatchBuffer(config.batchSize(), config.flushMs());
        WorkerMetrics  metrics  = new WorkerMetrics();
        BatchInserter  inserter = new BatchInserter(buffer, db, metrics);

        Thread inserterThread = Thread.ofVirtual()
                .name("batch-inserter")
                .start(inserter);

        // ── 3. MQTT client ────────────────────────────────────────────────────
        Mqtt3AsyncClient mqttClient = Mqtt3Client.builder()
                .identifier(config.clientId())
                .serverHost(config.brokerHost())
                .serverPort(config.brokerPort())
                .automaticReconnect()
                    .initialDelay(1, TimeUnit.SECONDS)
                    .maxDelay(30, TimeUnit.SECONDS)
                    .applyAutomaticReconnect()
                .buildAsync();

        mqttClient.connect()
                .orTimeout(10, TimeUnit.SECONDS)
                .get();

        long startupMs = System.currentTimeMillis() - jvmStart;
        System.err.printf("[worker] MQTT connected — startup=%dms%n", startupMs);

        for (String topic : config.topics()) {
            mqttClient.subscribeWith()
                    .topicFilter(topic)
                    .qos(MqttQos.fromCode(config.qos()))
                    .send()
                    .get(5, TimeUnit.SECONDS);
            System.err.printf("[worker] subscribed → %s%n", topic);
        }

        // ── 4. Callback de mensajes MQTT ──────────────────────────────────────
        MqttQos targetQos = MqttQos.fromCode(config.qos());
        mqttClient.publishes(MqttGlobalPublishFilter.SUBSCRIBED, publish -> {
            String topic   = publish.getTopic().toString();
            String payload = publish.getPayload()
                    .map(bb -> {
                        byte[] bytes = new byte[bb.remaining()];
                        bb.get(bytes);
                        return new String(bytes, StandardCharsets.UTF_8);
                    })
                    .orElse("");
            long receivedMs = System.currentTimeMillis();
            buffer.offer(new BatchBuffer.MqttPayload(topic, payload, receivedMs));
        });

        // ── 5. Reporter de métricas cada 2 s ─────────────────────────────────
        var scheduler = Executors.newSingleThreadScheduledExecutor(r ->
                Thread.ofVirtual().name("metrics-reporter").unstarted(r));
        scheduler.scheduleAtFixedRate(
                () -> metrics.printStats(buffer), 2, 2, TimeUnit.SECONDS);

        // ── 6. Shutdown hook — flush + WAL checkpoint ─────────────────────────
        Runtime.getRuntime().addShutdownHook(Thread.ofVirtual().unstarted(() -> {
            System.err.println("[worker] shutdown: flushing buffer...");
            try {
                scheduler.shutdown();
                inserter.stop();
                inserterThread.join(10_000);
            } catch (InterruptedException ignored) {}

            db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
            db.close();

            metrics.printFinal(buffer);
            System.err.printf("[worker] binary_size_check: run 'ls -lh %s'%n",
                    ProcessHandle.current().info().command().orElse("?"));
        }));

        // ── 7. Esperar indefinidamente ────────────────────────────────────────
        inserterThread.join();
    }

    /** Crea el schema SQLite si no existe. */
    static void initSchema(SqliteConnection db) {
        db.exec("""
            CREATE TABLE IF NOT EXISTS mqtt_messages (
                id          INTEGER PRIMARY KEY,
                topic       TEXT    NOT NULL,
                payload     TEXT,
                received_ms INTEGER NOT NULL
            )
            """);
        db.exec("CREATE INDEX IF NOT EXISTS idx_mqtt_topic ON mqtt_messages(topic)");
        db.exec("CREATE INDEX IF NOT EXISTS idx_mqtt_ts    ON mqtt_messages(received_ms)");
    }
}
