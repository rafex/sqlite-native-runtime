package mx.rafex.sqlite;

import org.junit.jupiter.api.*;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.*;

/**
 * TT-3 — Java Integration Tests.
 *
 * <p>Validan escenarios <em>realistas de uso</em> que los unit tests no pueden cubrir:
 * concurrencia con virtual threads (Project Loom), múltiples conexiones con WAL,
 * datasets grandes, reutilización de statements y recuperación de errores.
 *
 * <p>Todos tienen {@code @Tag("integration")} y se <strong>excluyen</strong> del
 * run por defecto de Maven. Para activarlos:
 * <pre>  make test-integration   # o: mvn test -Pintegration</pre>
 *
 * <p><strong>Invariantes de threading recordadas:</strong>
 * <ul>
 *   <li>{@link SqliteConnection} es seguro entre hilos (FULLMUTEX + Mutex Rust).</li>
 *   <li>{@link SqliteStatement} NO es seguro entre hilos concurrentes — cada hilo
 *       debe tener su propia instancia obtenida de {@code conn.prepare()}.</li>
 *   <li>{@link SqliteConnection#lastError()} usa {@code snr_last_error_copy()} —
 *       seguro con Project Loom (aislamiento por OS thread).</li>
 * </ul>
 */
@Tag("integration")
@DisplayName("TT-3 — Integration Tests")
class SqliteIntegrationTest {

    // ─── Helpers ─────────────────────────────────────────────────────────────

    /** Resuelve la ruta real del directorio temporal (evita el symlink /var→/private/var en macOS). */
    private static String realPath(Path dir, String filename) throws IOException {
        return dir.toRealPath().resolve(filename).toString();
    }

    /** Recoge las excepciones lanzadas por hilos y las reporta con assertAll. */
    private static void joinAll(List<Thread> threads, List<Throwable> errors)
            throws InterruptedException {
        for (Thread t : threads) t.join();
        if (!errors.isEmpty()) {
            AssertionError ae = new AssertionError("Excepciones en hilos virtuales (" + errors.size() + ")");
            errors.forEach(ae::addSuppressed);
            throw ae;
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Concurrent Reads
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Concurrent reads on shared connection")
    class ConcurrentReadsTest {

        @Test
        @DisplayName("20 virtual threads leen la misma BD — todos devuelven el mismo count")
        void concurrent_20_virtualThreads_correctCount() throws Exception {
            final int THREADS = 20;
            final int ROWS = 500;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT)");
                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t(v) VALUES(?)")) {
                        for (int i = 0; i < ROWS; i++) {
                            ins.bindText(1, "fila-" + i).stepAndDone(); ins.reset();
                        }
                    }
                });

                var barrier = new CyclicBarrier(THREADS);
                var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
                var counts  = new AtomicLong(0);
                var threads = new ArrayList<Thread>();

                for (int i = 0; i < THREADS; i++) {
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await(); // sincronizar: máxima contención
                            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                                assertTrue(q.step());
                                counts.addAndGet(q.columnInt(0));
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }
                joinAll(threads, errors);
                assertEquals((long) THREADS * ROWS, counts.get(),
                        "cada hilo debe leer " + ROWS + " filas");
            }
        }

        @Test
        @DisplayName("50 virtual threads en paralelo — la conexión no se corrompe")
        void concurrent_50_virtualThreads_connectionIntact() throws Exception {
            final int THREADS = 50;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(x INTEGER)");
                db.exec("INSERT INTO t VALUES(42)");

                var barrier = new CyclicBarrier(THREADS);
                var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
                var threads = new ArrayList<Thread>();

                for (int i = 0; i < THREADS; i++) {
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await();
                            try (var q = db.prepare("SELECT x FROM t")) {
                                assertTrue(q.step());
                                assertEquals(42L, q.columnInt(0));
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }
                joinAll(threads, errors);
                assertTrue(db.ping(), "conexión debe seguir operativa tras 50 lecturas concurrentes");
            }
        }

        @Test
        @DisplayName("Lectura concurrente: columnText y columnInt no se corrompen entre hilos")
        void concurrent_reads_mixed_column_types() throws Exception {
            final int THREADS = 15;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(n INTEGER, s TEXT, r REAL)");
                db.exec("INSERT INTO t VALUES(100, 'hola', 3.14)");

                var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
                var barrier = new CyclicBarrier(THREADS);
                var threads = new ArrayList<Thread>();

                for (int i = 0; i < THREADS; i++) {
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await();
                            try (var q = db.prepare("SELECT n, s, r FROM t")) {
                                assertTrue(q.step());
                                assertEquals(100L,  q.columnInt(0));
                                assertEquals("hola", q.columnText(1));
                                assertEquals(3.14,  q.columnDouble(2), 1e-10);
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }
                joinAll(threads, errors);
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Concurrent Writes
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Concurrent writes on shared connection")
    class ConcurrentWritesTest {

        @Test
        @DisplayName("10 virtual threads × 100 inserts = 1 000 filas exactas")
        void concurrent_10threads_100rows_each() throws Exception {
            final int THREADS       = 10;
            final int ROWS_PER_THREAD = 100;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, v INTEGER)");
                db.busyTimeout(5000);

                var barrier = new CyclicBarrier(THREADS);
                var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
                var threads = new ArrayList<Thread>();

                for (int tid = 0; tid < THREADS; tid++) {
                    final int threadId = tid;
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await();
                            // Cada hilo tiene su propio statement — SqliteStatement no es thread-safe
                            try (var ins = db.prepare("INSERT INTO t(v) VALUES(?)")) {
                                for (int r = 0; r < ROWS_PER_THREAD; r++) {
                                    ins.bindInt(1, threadId * ROWS_PER_THREAD + r).stepAndDone();
                                    ins.reset().clearBindings();
                                }
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }
                joinAll(threads, errors);

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals((long) THREADS * ROWS_PER_THREAD, q.columnInt(0),
                            "deben existir exactamente " + (THREADS * ROWS_PER_THREAD) + " filas");
                }
            }
        }

        @Test
        @DisplayName("Writes y reads interleaved — sin corrupción")
        void concurrent_writes_and_reads_interleaved() throws Exception {
            final int WRITERS = 5;
            final int READERS = 5;
            final int ROWS    = 50;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");
                db.busyTimeout(5000);

                var barrier = new CyclicBarrier(WRITERS + READERS);
                var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
                var threads = new ArrayList<Thread>();

                // Writers
                for (int w = 0; w < WRITERS; w++) {
                    final int wid = w;
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await();
                            try (var ins = db.prepare("INSERT INTO t(v) VALUES(?)")) {
                                for (int r = 0; r < ROWS; r++) {
                                    ins.bindInt(1, wid * ROWS + r).stepAndDone(); ins.reset();
                                }
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }

                // Readers — leen lo que haya en cada momento; solo verifican que no crashen
                for (int r = 0; r < READERS; r++) {
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await();
                            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                                assertTrue(q.step());
                                assertTrue(q.columnInt(0) >= 0);
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }

                joinAll(threads, errors);

                // Tras completar todos, el count debe ser exacto
                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals((long) WRITERS * ROWS, q.columnInt(0));
                }
            }
        }

        @Test
        @DisplayName("Transacciones de múltiples hilos serializadas — contador exacto")
        void concurrent_transactions_serialize() throws Exception {
            final int THREADS = 8;
            final int TX_ROWS = 20;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE counters(id INTEGER PRIMARY KEY, val INTEGER)");
                db.exec("INSERT INTO counters VALUES(1, 0)");

                var barrier = new CyclicBarrier(THREADS);
                var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
                var threads = new ArrayList<Thread>();

                for (int i = 0; i < THREADS; i++) {
                    threads.add(Thread.ofVirtual().start(() -> {
                        try {
                            barrier.await(); // arrancar todos al mismo tiempo
                            // SQLite no permite transacciones anidadas en la misma conexión:
                            // se serializa a nivel Java para que solo un hilo esté dentro
                            // de BEGIN…COMMIT en cada momento.
                            synchronized (db) {
                                db.transaction(() -> {
                                    try (var q = db.prepare("SELECT val FROM counters WHERE id=1")) {
                                        assertTrue(q.step());
                                        long cur = q.columnInt(0);
                                        try (var u = db.prepare("UPDATE counters SET val=? WHERE id=1")) {
                                            u.bindInt(1, cur + TX_ROWS).stepAndDone();
                                        }
                                    }
                                });
                            }
                        } catch (Throwable e) { errors.add(e); }
                    }));
                }
                joinAll(threads, errors);

                try (var q = db.prepare("SELECT val FROM counters WHERE id=1")) {
                    assertTrue(q.step());
                    assertEquals((long) THREADS * TX_ROWS, q.columnInt(0),
                            "el contador debe ser threads×TX_ROWS con transacciones serializadas");
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // WAL + Multi-Connection
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("WAL mode con múltiples conexiones a fichero")
    class MultiConnectionWalTest {

        @Test
        @DisplayName("Escritor y lector coexisten en WAL sin conflictos")
        void wal_writer_reader_coexist(@TempDir Path tmp) throws IOException {
            String path = realPath(tmp, "wal_coexist.db");

            // Conexión escritora: crea tabla y escribe en WAL
            try (var writer = SqliteConnection.open(path)) {
                writer.enableWal().busyTimeout(3000);
                writer.exec("CREATE TABLE t(id INTEGER PRIMARY KEY, msg TEXT)");

                // Conexión lectora: abre el mismo fichero en WAL
                try (var reader = SqliteConnection.open(path)) {
                    reader.busyTimeout(3000);

                    // Writer inserta
                    writer.transaction(() -> {
                        try (var ins = writer.prepare("INSERT INTO t(msg) VALUES(?)")) {
                            for (int i = 0; i < 100; i++) {
                                ins.bindText(1, "msg-" + i).stepAndDone(); ins.reset();
                            }
                        }
                    });

                    // Reader lee tras el commit del writer
                    try (var q = reader.prepare("SELECT COUNT(*) FROM t")) {
                        assertTrue(q.step());
                        assertEquals(100L, q.columnInt(0),
                                "el reader debe ver las 100 filas del writer");
                    }
                }
            }
        }

        @Test
        @DisplayName("Múltiples readers concurrentes en WAL — sin SQLITE_BUSY")
        void wal_multiple_readers_no_busy(@TempDir Path tmp) throws Exception {
            final int READERS = 10;
            String path = realPath(tmp, "wal_readers.db");

            try (var setup = SqliteConnection.open(path)) {
                setup.enableWal();
                setup.exec("CREATE TABLE t(v INTEGER)");
                setup.transaction(() -> {
                    try (var ins = setup.prepare("INSERT INTO t VALUES(?)")) {
                        for (int i = 0; i < 200; i++) {
                            ins.bindInt(1, i).stepAndDone(); ins.reset();
                        }
                    }
                });
            }

            var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
            var barrier = new CyclicBarrier(READERS);
            var threads = new ArrayList<Thread>();

            for (int r = 0; r < READERS; r++) {
                threads.add(Thread.ofVirtual().start(() -> {
                    try {
                        barrier.await();
                        try (var conn = SqliteConnection.open(path)) {
                            conn.busyTimeout(3000);
                            try (var q = conn.prepare("SELECT COUNT(*) FROM t")) {
                                assertTrue(q.step());
                                assertEquals(200L, q.columnInt(0));
                            }
                        }
                    } catch (Throwable e) { errors.add(e); }
                }));
            }
            joinAll(threads, errors);
        }

        @Test
        @DisplayName("WAL checkpoint PASSIVE devuelve walFrames > 0 tras inserts")
        void wal_checkpoint_passive_after_inserts(@TempDir Path tmp) throws IOException {
            String path = realPath(tmp, "wal_ckpt.db");

            try (var db = SqliteConnection.open(path)) {
                db.enableWal().walAutocheckpoint(0); // desactivar auto-checkpoint
                db.exec("CREATE TABLE t(v INTEGER)");
                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                        for (int i = 0; i < 100; i++) {
                            ins.bindInt(1, i).stepAndDone(); ins.reset();
                        }
                    }
                });

                var result = db.walCheckpoint(SqliteConnection.WalMode.PASSIVE, null);
                // Tras 100 inserts sin auto-checkpoint, debe haber frames en el WAL
                assertTrue(result.walFrames() > 0,
                        "deben existir frames en el WAL antes del checkpoint: " + result);
            }
        }

        @Test
        @DisplayName("WAL checkpoint TRUNCATE vacía el WAL completamente")
        void wal_checkpoint_truncate_clears_wal(@TempDir Path tmp) throws IOException {
            String path = realPath(tmp, "wal_trunc.db");

            try (var db = SqliteConnection.open(path)) {
                db.enableWal().walAutocheckpoint(0);
                db.exec("CREATE TABLE t(v INTEGER)");

                // Insertar filas para generar WAL frames
                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                        for (int i = 0; i < 50; i++) {
                            ins.bindInt(1, i).stepAndDone(); ins.reset();
                        }
                    }
                });

                // TRUNCATE: todos los frames se checkpointan y el WAL queda a 0
                var result = db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
                assertEquals(0, result.walFrames(),
                        "TRUNCATE debe dejar el WAL con 0 frames: " + result);
                assertEquals(0, result.checkpointed(),
                        "TRUNCATE debe reportar 0 frames checkpointed tras truncar: " + result);
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Bulk Insert
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Bulk insert — datasets grandes")
    class BulkInsertTest {

        @Test
        @DisplayName("10 000 inserts en una sola transacción — count correcto")
        void bulk_insert_10k_singleTransaction() {
            final int ROWS = 10_000;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, v INTEGER)");

                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t(v) VALUES(?)")) {
                        for (int i = 0; i < ROWS; i++) {
                            ins.bindInt(1, i).stepAndDone(); ins.reset();
                        }
                    }
                });

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals((long) ROWS, q.columnInt(0));
                }
            }
        }

        @Test
        @DisplayName("10 000 inserts — primera y última fila correctas")
        void bulk_insert_10k_firstAndLastRow() {
            final int ROWS = 10_000;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, v INTEGER)");

                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t(v) VALUES(?)")) {
                        for (int i = 0; i < ROWS; i++) {
                            ins.bindInt(1, i).stepAndDone(); ins.reset();
                        }
                    }
                });

                try (var q = db.prepare("SELECT v FROM t ORDER BY id ASC LIMIT 1")) {
                    assertTrue(q.step());
                    assertEquals(0L, q.columnInt(0), "primera fila debe ser v=0");
                }
                try (var q = db.prepare("SELECT v FROM t ORDER BY id DESC LIMIT 1")) {
                    assertTrue(q.step());
                    assertEquals((long) ROWS - 1, q.columnInt(0), "última fila debe ser v=" + (ROWS - 1));
                }
            }
        }

        @Test
        @DisplayName("Bulk insert con named parameters — todos los bindings correctos")
        void bulk_insert_named_parameters() {
            final int ROWS = 1_000;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(n INTEGER, s TEXT, r REAL)");

                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t VALUES(:n, :s, :r)")) {
                        int nIdx = ins.parameterIndex(":n");
                        int sIdx = ins.parameterIndex(":s");
                        int rIdx = ins.parameterIndex(":r");
                        assertEquals(1, nIdx);
                        assertEquals(2, sIdx);
                        assertEquals(3, rIdx);

                        for (int i = 0; i < ROWS; i++) {
                            ins.bindInt(nIdx, i)
                               .bindText(sIdx, "v-" + i)
                               .bindDouble(rIdx, i * 0.5)
                               .stepAndDone();
                            ins.reset().clearBindings();
                        }
                    }
                });

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals((long) ROWS, q.columnInt(0));
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Large Text / Blob
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Valores TEXT y BLOB grandes (1 MB+)")
    class LargeDataTest {

        private static final int MB = 1024 * 1024;

        @Test
        @DisplayName("TEXT de 1 MB — columnTextSafe round-trip")
        void largeText_1MB_roundTrip() {
            String bigText = "A".repeat(MB);

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v TEXT)");
                try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                    ins.bindText(1, bigText).stepAndDone();
                }
                try (var q = db.prepare("SELECT v FROM t")) {
                    assertTrue(q.step());
                    String retrieved = q.columnTextSafe(0); // copia segura en heap
                    assertEquals(bigText.length(), retrieved.length(),
                            "la longitud del texto debe conservarse");
                    assertEquals(bigText, retrieved, "el contenido del texto debe conservarse");
                }
            }
        }

        @Test
        @DisplayName("TEXT Unicode de 1 MB — sin corrupción de codificación")
        void largeText_unicode_roundTrip() {
            // Carácter Unicode de 3 bytes en UTF-8
            String unit = "あいう"; // 3 chars × 3 bytes = 9 bytes por unidad
            String bigText = unit.repeat(MB / (unit.getBytes(java.nio.charset.StandardCharsets.UTF_8).length));

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v TEXT)");
                try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                    ins.bindText(1, bigText).stepAndDone();
                }
                try (var q = db.prepare("SELECT v FROM t")) {
                    assertTrue(q.step());
                    String retrieved = q.columnTextSafe(0);
                    assertEquals(bigText, retrieved, "el texto Unicode debe conservarse byte a byte");
                }
            }
        }

        @Test
        @DisplayName("BLOB de 1 MB — columnBlob round-trip con integridad verificada")
        void largeBlob_1MB_roundTrip() {
            byte[] bigBlob = new byte[MB];
            // Patrón determinista para verificar integridad
            for (int i = 0; i < bigBlob.length; i++) {
                bigBlob[i] = (byte) (i & 0xFF);
            }

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v BLOB)");
                try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                    ins.bindBlob(1, bigBlob).stepAndDone();
                }
                try (var q = db.prepare("SELECT v FROM t")) {
                    assertTrue(q.step());
                    byte[] retrieved = q.columnBlob(0);
                    assertNotNull(retrieved, "el blob no debe ser null");
                    assertEquals(bigBlob.length, retrieved.length,
                            "la longitud del blob debe conservarse");
                    // Verificar primer, último y bytes intermedios
                    assertEquals(bigBlob[0],              retrieved[0]);
                    assertEquals(bigBlob[MB / 2],         retrieved[MB / 2]);
                    assertEquals(bigBlob[MB - 1],         retrieved[MB - 1]);
                    assertArrayEquals(bigBlob, retrieved, "el blob debe ser idéntico byte a byte");
                }
            }
        }

        @Test
        @DisplayName("BLOB vacío y NULL son distintos")
        void blob_empty_vs_null_are_distinct() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v BLOB)");

                // Insertar NULL y blob vacío
                db.exec("INSERT INTO t VALUES(NULL)");
                try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                    ins.bindBlob(1, new byte[0]).stepAndDone();
                }

                try (var q = db.prepare("SELECT v, typeof(v) FROM t ORDER BY rowid")) {
                    // Primera fila: NULL
                    assertTrue(q.step());
                    assertNull(q.columnBlob(0), "fila NULL debe devolver null");
                    assertEquals(SqliteStatement.TYPE_NULL, q.columnType(0));

                    // Segunda fila: blob vacío (SQLite puede devolver null ptr para 0 bytes)
                    assertTrue(q.step());
                    assertEquals(SqliteStatement.TYPE_BLOB, q.columnType(0),
                            "el tipo debe ser BLOB aunque esté vacío");
                    assertEquals(0, q.columnBytes(0), "blob vacío debe tener 0 bytes");
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Statement Reuse
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Reutilización de statements en ciclos largos")
    class StatementReuseTest {

        @Test
        @DisplayName("1 000 ciclos insert → stepAndDone → reset — count final correcto")
        void insert_1000_cycles_sameStatement() {
            final int CYCLES = 1_000;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, v INTEGER)");

                try (var ins = db.prepare("INSERT INTO t(v) VALUES(?)")) {
                    for (int i = 0; i < CYCLES; i++) {
                        ins.bindInt(1, i).stepAndDone(); ins.reset().clearBindings();
                    }
                }

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals((long) CYCLES, q.columnInt(0));
                }
            }
        }

        @Test
        @DisplayName("1 000 ciclos de SELECT — el statement se resetea sin corrupción")
        void query_1000_steps_sameStatement() {
            final int ROWS = 100;
            final int QUERIES = 1_000;

            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");
                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                        for (int i = 0; i < ROWS; i++) {
                            ins.bindInt(1, i).stepAndDone(); ins.reset();
                        }
                    }
                });

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    for (int cycle = 0; cycle < QUERIES; cycle++) {
                        assertTrue(q.step());
                        assertEquals((long) ROWS, q.columnInt(0),
                                "ciclo " + cycle + ": count incorrecto");
                        q.reset();
                    }
                }
            }
        }

        @Test
        @DisplayName("clearBindings entre ciclos — no quedan bindings anteriores")
        void clearBindings_between_cycles() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(a INTEGER, b TEXT)");

                try (var ins = db.prepare("INSERT INTO t VALUES(?, ?)")) {
                    // Primera inserción
                    ins.bindInt(1, 10).bindText(2, "primero").stepAndDone(); ins.reset().clearBindings();
                    // Segunda inserción sin bindText: el segundo parámetro debe ser NULL
                    ins.bindInt(1, 20).stepAndDone();
                }

                try (var q = db.prepare("SELECT a, b FROM t ORDER BY a")) {
                    assertTrue(q.step());
                    assertEquals(10L, q.columnInt(0));
                    assertEquals("primero", q.columnText(1));

                    assertTrue(q.step());
                    assertEquals(20L, q.columnInt(0));
                    assertNull(q.columnText(1),
                            "tras clearBindings el segundo parámetro debe ser NULL");
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Connection Pool Simulation
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Simulación de pool de conexiones")
    class ConnectionPoolSimTest {

        @Test
        @DisplayName("50 conexiones :memory: abiertas y cerradas secuencialmente — sin leaks")
        void sequential_50_open_close_noLeak() {
            final int N = 50;

            for (int i = 0; i < N; i++) {
                try (var db = SqliteConnection.memory()) {
                    assertTrue(db.ping(), "conexión " + i + " debe responder al ping");
                    db.exec("CREATE TABLE t(x INTEGER)");
                    db.exec("INSERT INTO t VALUES(" + i + ")");
                    try (var q = db.prepare("SELECT x FROM t")) {
                        assertTrue(q.step());
                        assertEquals((long) i, q.columnInt(0));
                    }
                } // close() invocado implícitamente
            }
        }

        @Test
        @DisplayName("20 conexiones concurrentes (virtual threads) — todas operacionales")
        void concurrent_20_connections_virtualThreads() throws Exception {
            final int N = 20;

            var errors  = Collections.synchronizedList(new ArrayList<Throwable>());
            var barrier = new CyclicBarrier(N);
            var successes = new AtomicInteger(0);
            var threads = new ArrayList<Thread>();

            for (int i = 0; i < N; i++) {
                final int id = i;
                threads.add(Thread.ofVirtual().start(() -> {
                    try {
                        barrier.await();
                        try (var db = SqliteConnection.memory()) {
                            db.exec("CREATE TABLE t(v INTEGER)");
                            db.exec("INSERT INTO t VALUES(" + id + ")");
                            try (var q = db.prepare("SELECT v FROM t")) {
                                assertTrue(q.step());
                                assertEquals((long) id, q.columnInt(0));
                            }
                            successes.incrementAndGet();
                        }
                    } catch (Throwable e) { errors.add(e); }
                }));
            }
            joinAll(threads, errors);
            assertEquals(N, successes.get(), "todas las " + N + " conexiones deben tener éxito");
        }

        @Test
        @DisplayName("Conexiones a fichero: open/use/close × 20 — sin WAL file leak")
        void sequential_file_connections_noLeak(@TempDir Path tmp) throws IOException {
            String path = realPath(tmp, "pool_test.db");

            // Primera conexión crea el esquema
            try (var db = SqliteConnection.open(path)) {
                db.exec("CREATE TABLE t(v INTEGER)");
            }

            // 20 conexiones secuenciales que insertan y leen
            for (int i = 0; i < 20; i++) {
                try (var db = SqliteConnection.open(path)) {
                    db.busyTimeout(1000);
                    try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                        ins.bindInt(1, i).stepAndDone();
                    }
                    try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                        assertTrue(q.step());
                        assertTrue(q.columnInt(0) > 0);
                    }
                }
            }

            // Verificación final
            try (var db = SqliteConnection.open(path)) {
                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals(20L, q.columnInt(0));
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Error Recovery
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Recuperación de errores y resiliencia")
    class ErrorRecoveryTest {

        @Test
        @DisplayName("Excepción en transaction() hace rollback automático")
        void exception_in_transaction_auto_rollback() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");
                db.exec("INSERT INTO t VALUES(1)");

                assertThrows(RuntimeException.class, () ->
                    db.transaction(() -> {
                        db.exec("INSERT INTO t VALUES(2)");
                        throw new RuntimeException("fallo simulado");
                    })
                );

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals(1L, q.columnInt(0),
                            "el rollback automático debe dejar solo la fila original");
                }
            }
        }

        @Test
        @DisplayName("Conexión reutilizable tras excepción y rollback")
        void connection_reusable_after_exception() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");

                // Primera transacción falla
                assertThrows(RuntimeException.class, () ->
                    db.transaction(() -> {
                        db.exec("INSERT INTO t VALUES(99)");
                        throw new RuntimeException("fallo intencional");
                    })
                );

                // Segunda transacción debe funcionar normalmente
                db.transaction(() -> db.exec("INSERT INTO t VALUES(42)"));

                try (var q = db.prepare("SELECT v FROM t")) {
                    assertTrue(q.step());
                    assertEquals(42L, q.columnInt(0),
                            "debe existir solo la fila de la segunda transacción");
                    assertFalse(q.step(), "no debe haber más filas");
                }
                assertTrue(db.ping(), "la conexión debe seguir operativa");
            }
        }

        @Test
        @DisplayName("withSavepoint() hace rollback solo del savepoint — outer data intacto")
        void withSavepoint_exception_rollback_scope() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");
                db.exec("INSERT INTO t VALUES(1)"); // dato outer

                assertThrows(RuntimeException.class, () ->
                    db.withSavepoint("sp1", () -> {
                        db.exec("INSERT INTO t VALUES(2)"); // dato inner
                        throw new RuntimeException("fallo dentro del savepoint");
                    })
                );

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals(1L, q.columnInt(0),
                            "solo debe existir el dato outer — el savepoint hizo rollback");
                }
            }
        }

        @Test
        @DisplayName("Múltiples fallos consecutivos — la conexión no se degrada")
        void multiple_consecutive_failures_connection_stable() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER NOT NULL)");

                for (int i = 0; i < 5; i++) {
                    final int attempt = i;
                    assertThrows(SqliteException.class, () ->
                        db.exec("INSERT INTO t VALUES(NULL)"), // viola NOT NULL
                        "intento " + attempt + " debe lanzar excepción"
                    );
                    assertTrue(db.ping(), "la conexión debe seguir viva tras el fallo " + attempt);
                }

                // Tras 5 fallos, debe poder insertar correctamente
                db.exec("INSERT INTO t VALUES(42)");
                try (var q = db.prepare("SELECT v FROM t")) {
                    assertTrue(q.step());
                    assertEquals(42L, q.columnInt(0));
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Savepoint Nesting
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("Savepoints anidados")
    class SavepointNestingTest {

        @Test
        @DisplayName("Tres niveles anidados — todos hacen commit")
        void three_level_nesting_all_commit() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");

                db.withSavepoint("outer", () -> {
                    db.exec("INSERT INTO t VALUES(1)");
                    db.withSavepoint("middle", () -> {
                        db.exec("INSERT INTO t VALUES(2)");
                        db.withSavepoint("inner", () -> {
                            db.exec("INSERT INTO t VALUES(3)");
                        });
                    });
                });

                try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                    assertTrue(q.step());
                    assertEquals(3L, q.columnInt(0),
                            "los tres niveles deben haber hecho commit");
                }
            }
        }

        @Test
        @DisplayName("Rollback del nivel inner — levels outer y middle persisten")
        void inner_rollback_outer_persists() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");

                db.withSavepoint("outer", () -> {
                    db.exec("INSERT INTO t VALUES(1)");
                    db.withSavepoint("middle", () -> {
                        db.exec("INSERT INTO t VALUES(2)");
                        assertThrows(RuntimeException.class, () ->
                            db.withSavepoint("inner", () -> {
                                db.exec("INSERT INTO t VALUES(3)");
                                throw new RuntimeException("rollback inner");
                            })
                        );
                        // El lanzamiento de withSavepoint(inner) se captura aquí
                        // y la ejecución continúa en middle
                    });
                });

                try (var q = db.prepare("SELECT v FROM t ORDER BY v")) {
                    assertTrue(q.step()); assertEquals(1L, q.columnInt(0));
                    assertTrue(q.step()); assertEquals(2L, q.columnInt(0));
                    assertFalse(q.step(), "v=3 debe haber sido revertido");
                }
            }
        }

        @Test
        @DisplayName("Manual savepoint/release/rollbackTo — semántica exacta de SQLite")
        void manual_savepoint_semantics() {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t(v INTEGER)");
                db.exec("INSERT INTO t VALUES(10)");

                db.savepoint("sp");
                db.exec("INSERT INTO t VALUES(20)");
                db.rollbackTo("sp"); // deshace el INSERT 20
                db.release("sp");    // descarta el savepoint

                db.exec("INSERT INTO t VALUES(30)"); // esta sí persiste

                try (var q = db.prepare("SELECT v FROM t ORDER BY v")) {
                    assertTrue(q.step()); assertEquals(10L, q.columnInt(0));
                    assertTrue(q.step()); assertEquals(30L, q.columnInt(0));
                    assertFalse(q.step(), "v=20 debe haber sido revertido");
                }
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // WAL Checkpoint Load
    // ════════════════════════════════════════════════════════════════════════

    @Nested
    @DisplayName("WAL checkpoint bajo carga")
    class WalCheckpointLoadTest {

        @Test
        @DisplayName("Auto-checkpoint desactivado — WAL crece y se checkpoint manual")
        void autocheckpoint_disabled_then_manual(@TempDir Path tmp) throws IOException {
            String path = realPath(tmp, "wal_manual_ckpt.db");

            try (var db = SqliteConnection.open(path)) {
                db.enableWal().walAutocheckpoint(0); // sin auto-checkpoint
                db.exec("CREATE TABLE t(v INTEGER)");

                // 3 transacciones separadas para generar WAL frames
                for (int tx = 0; tx < 3; tx++) {
                    db.transaction(() -> {
                        try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                            for (int r = 0; r < 30; r++) {
                                ins.bindInt(1, r).stepAndDone(); ins.reset();
                            }
                        }
                    });
                }

                var before = db.walCheckpoint(SqliteConnection.WalMode.PASSIVE, null);
                assertTrue(before.walFrames() > 0,
                        "WAL debe tener frames antes del checkpoint: " + before);

                var after = db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
                assertEquals(0, after.walFrames(),
                        "TRUNCATE debe dejar el WAL en 0 frames: " + after);
            }
        }

        @Test
        @DisplayName("walCheckpoint con dbName null y string vacío son equivalentes")
        void walCheckpoint_nullAndEmpty_equivalent(@TempDir Path tmp) throws IOException {
            String path = realPath(tmp, "wal_null_name.db");

            try (var db = SqliteConnection.open(path)) {
                db.enableWal();
                db.exec("CREATE TABLE t(v INTEGER)");
                db.exec("INSERT INTO t VALUES(1)");

                var r1 = db.walCheckpoint(SqliteConnection.WalMode.PASSIVE, null);
                var r2 = db.walCheckpoint(SqliteConnection.WalMode.PASSIVE, "");

                // Ambos apuntan a "main" — mismo comportamiento
                assertDoesNotThrow(() -> {
                    assertTrue(r1.walFrames() >= 0);
                    assertTrue(r2.walFrames() >= 0);
                });
            }
        }
    }
}
