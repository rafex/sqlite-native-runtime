package mx.rafex.ether.sqlite.spike;

import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteStatement;

import java.util.List;

/**
 * Inserta mensajes MQTT en SQLite en lotes dentro de un único virtual thread.
 *
 * <h2>Patrón de uso</h2>
 * <pre>{@code
 * var inserter = new BatchInserter(buffer, db, metrics);
 * Thread.ofVirtual().name("batch-inserter").start(inserter);
 * // ...
 * inserter.stop();  // en el shutdown hook
 * }</pre>
 *
 * <h2>Estrategia de flush</h2>
 * <ol>
 *   <li>Espera hasta {@code FLUSH_MS} por al menos un mensaje.</li>
 *   <li>Drena hasta {@code BATCH_SIZE} mensajes sin bloquear.</li>
 *   <li>Ejecuta un único BEGIN → N inserts → COMMIT.</li>
 *   <li>En error: ROLLBACK, espera 100 ms, reintenta (el batch se descarta).</li>
 * </ol>
 *
 * <h2>Testabilidad</h2>
 * {@link #insertBatch(SqliteStatement, List)} es package-private para tests unitarios.
 */
public final class BatchInserter implements Runnable {

    private static final String INSERT_SQL =
        "INSERT INTO mqtt_messages(topic, payload, received_ms) VALUES(?,?,?)";

    private final BatchBuffer    buffer;
    private final SqliteConnection db;
    private final WorkerMetrics  metrics;
    private volatile boolean     running = true;

    public BatchInserter(BatchBuffer buffer, SqliteConnection db, WorkerMetrics metrics) {
        this.buffer  = buffer;
        this.db      = db;
        this.metrics = metrics;
    }

    @Override
    public void run() {
        try (SqliteStatement stmt = db.prepare(INSERT_SQL)) {
            while (running || buffer.size() > 0) {
                List<BatchBuffer.MqttPayload> batch = buffer.drain();
                if (batch.isEmpty()) {
                    continue;
                }
                insertBatch(stmt, batch);
            }
            // flush final tras stop()
            List<BatchBuffer.MqttPayload> remaining = buffer.drain();
            if (!remaining.isEmpty()) {
                insertBatch(stmt, remaining);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    /**
     * Inserta un batch en una única transacción.
     * Package-private para tests unitarios.
     */
    void insertBatch(SqliteStatement stmt, List<BatchBuffer.MqttPayload> batch) {
        long start = System.nanoTime();
        try {
            db.transaction(() -> {
                for (BatchBuffer.MqttPayload msg : batch) {
                    stmt.bindText(1, msg.topic())
                        .bindInt(2, msg.receivedMs())
                        .bindText(3, msg.payload())
                        .stepAndDone();
                    stmt.reset().clearBindings();
                }
            });
            long durationMs = (System.nanoTime() - start) / 1_000_000L;
            metrics.recordBatch(batch.size(), durationMs);

        } catch (Exception e) {
            metrics.recordError();
            System.err.printf("[inserter] ERROR batch=%d msg=%s%n", batch.size(), e.getMessage());
            // backoff antes de continuar para no saturar en caso de error persistente
            try { Thread.sleep(100); } catch (InterruptedException ie) {
                Thread.currentThread().interrupt();
            }
        }
    }

    /**
     * Señaliza al inserter que deje de consumir nuevos mensajes.
     * El loop vacía el buffer antes de terminar.
     */
    public void stop() {
        running = false;
    }
}
