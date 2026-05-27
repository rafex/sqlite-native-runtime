package mx.rafex.ether.sqlite.spike;

import mx.rafex.ether.sqlite.JniSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteStatement;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests unitarios del BatchInserter — usa SQLite en memoria.
 *
 * <p>Prerequisito: ETHER_SQLITE_JNI_LIB apuntando a libether_sqlite_jni_runtime.so.
 */
class BatchInserterTest {

    private static final String INSERT_SQL =
        "INSERT INTO mqtt_messages(topic, payload, received_ms) VALUES(?,?,?)";

    // ── helpers ───────────────────────────────────────────────────────────────

    private SqliteConnection openMemoryDb() {
        SqliteConnection db = JniSqliteConnection.memory();
        db.enableWal().busyTimeout(1_000);
        MqttWorkerSpike.initSchema(db);
        return db;
    }

    private List<BatchBuffer.MqttPayload> makeBatch(int n) {
        var batch = new ArrayList<BatchBuffer.MqttPayload>(n);
        for (int i = 0; i < n; i++) {
            batch.add(new BatchBuffer.MqttPayload(
                "benchmark/sensor-" + (i % 5),
                "{\"seq\":" + i + ",\"temp\":23.7}",
                System.currentTimeMillis()
            ));
        }
        return batch;
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    @Test
    void shouldInsert100Messages() {
        try (SqliteConnection db = openMemoryDb()) {
            var buffer   = new BatchBuffer(100, 50);
            var metrics  = new WorkerMetrics();
            var inserter = new BatchInserter(buffer, db, metrics);
            var batch    = makeBatch(100);

            try (SqliteStatement stmt = db.prepare(INSERT_SQL)) {
                inserter.insertBatch(stmt, batch);
            }

            try (SqliteStatement q = db.prepare("SELECT count(*) FROM mqtt_messages")) {
                assertTrue(q.step());
                assertEquals(100, q.columnInt(0));
            }
        }
    }

    @Test
    void shouldInsert1000MessagesInMultipleBatches() {
        try (SqliteConnection db = openMemoryDb()) {
            var buffer   = new BatchBuffer(200, 50);
            var metrics  = new WorkerMetrics();
            var inserter = new BatchInserter(buffer, db, metrics);

            try (SqliteStatement stmt = db.prepare(INSERT_SQL)) {
                inserter.insertBatch(stmt, makeBatch(200));
                inserter.insertBatch(stmt, makeBatch(200));
                inserter.insertBatch(stmt, makeBatch(200));
                inserter.insertBatch(stmt, makeBatch(200));
                inserter.insertBatch(stmt, makeBatch(200));
            }

            try (SqliteStatement q = db.prepare("SELECT count(*) FROM mqtt_messages")) {
                assertTrue(q.step());
                assertEquals(1_000, q.columnInt(0));
            }
        }
    }

    @Test
    void metricsCountsMatchInserted() {
        try (SqliteConnection db = openMemoryDb()) {
            var buffer   = new BatchBuffer(50, 50);
            var metrics  = new WorkerMetrics();
            var inserter = new BatchInserter(buffer, db, metrics);

            try (SqliteStatement stmt = db.prepare(INSERT_SQL)) {
                inserter.insertBatch(stmt, makeBatch(50));
                inserter.insertBatch(stmt, makeBatch(50));
            }

            assertEquals(100, metrics.totalCommitted());
        }
    }

    @Test
    void emptyBatchDoesNotInsert() {
        try (SqliteConnection db = openMemoryDb()) {
            var buffer   = new BatchBuffer(100, 50);
            var metrics  = new WorkerMetrics();
            var inserter = new BatchInserter(buffer, db, metrics);

            try (SqliteStatement stmt = db.prepare(INSERT_SQL)) {
                inserter.insertBatch(stmt, List.of());
            }

            try (SqliteStatement q = db.prepare("SELECT count(*) FROM mqtt_messages")) {
                assertTrue(q.step());
                assertEquals(0, q.columnInt(0));
            }
            assertEquals(0, metrics.totalCommitted());
        }
    }

    @Test
    void batchBufferDropsWhenFull() {
        var buffer = new BatchBuffer(10, 50);  // capacity = 10 * 20 = 200
        for (int i = 0; i < 300; i++) {
            buffer.offer(new BatchBuffer.MqttPayload("t", "{}", System.currentTimeMillis()));
        }
        assertEquals(300, buffer.received());
        // los primeros 200 caben, los restantes 100 se descartan
        assertEquals(200, buffer.size() + (300 - 200 - buffer.dropped() < 0 ? 0 : 300 - 200 - buffer.dropped()));
        assertTrue(buffer.dropped() > 0);
    }
}
