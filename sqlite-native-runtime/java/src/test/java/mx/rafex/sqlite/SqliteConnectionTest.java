package mx.rafex.sqlite;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Path;
import java.util.logging.Level;
import java.util.logging.Logger;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests unitarios para {@link SqliteConnection}.
 * Usa bases de datos en memoria para evitar I/O de fichero salvo donde se necesita
 * (WAL checkpoint, apertura de fichero en modo READONLY, etc.).
 */
class SqliteConnectionTest {

    // ── Fábrica ────────────────────────────────────────────────────────────────

    @Test
    void memory_opensAnonymous() {
        try (var db = SqliteConnection.memory()) {
            assertNotNull(db);
            assertTrue(db.ping());
        }
    }

    @Test
    void memory_withName_opensTwoConnections() {
        // Dos conexiones al mismo nombre de memoria comparten datos (shared-cache URI)
        try (var db1 = SqliteConnection.memory("test_shared");
             var db2 = SqliteConnection.memory("test_shared")) {
            db1.exec("CREATE TABLE IF NOT EXISTS t (x INTEGER)");
            db1.exec("INSERT INTO t VALUES (42)");
            long count;
            try (var q = db2.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(1L, count);
        }
    }

    @Test
    void memory_invalidName_throws() {
        // El nombre contiene caracteres inválidos — Rust rechaza
        assertThrows(SqliteException.class, () -> SqliteConnection.memory("invalid!@#name"));
    }

    @Test
    void open_fileDb(@TempDir Path tmp) throws IOException {
        // toRealPath() resuelve symlinks — en macOS /var/folders -> /private/var/folders
        // SQLITE_OPEN_NOFOLLOW (siempre forzado) rechaza rutas con symlinks intermedios
        var path = tmp.toRealPath().resolve("test.db").toString();
        try (var db = SqliteConnection.open(path)) {
            assertTrue(db.ping());
        }
    }

    @Test
    void open_withFlags(@TempDir Path tmp) throws IOException {
        var realTmp = tmp.toRealPath();
        var path = realTmp.resolve("flags.db").toString();
        // Crear el archivo primero con flags por defecto
        try (var db = SqliteConnection.open(path)) {
            db.exec("CREATE TABLE t (x INTEGER)");
        }
        // Abrir en solo-lectura
        try (var db = SqliteConnection.open(path, SqliteLibrary.OPEN_READONLY)) {
            assertTrue(db.ping());
        }
    }

    @Test
    void open_nonexistentParent_throws() {
        assertThrows(SqliteException.class,
            () -> SqliteConnection.open("/nonexistent_8f4a2b/db.sqlite"));
    }

    @Test
    void open_withFlags_invalidPath_throws() {
        // Cubrir el error path del overload open(path, flags)
        assertThrows(SqliteException.class,
            () -> SqliteConnection.open("/nonexistent_8f4a2b/db.sqlite", SqliteLibrary.OPEN_READONLY));
    }

    @Test
    void open_withFineLogging_coversLambda(@TempDir Path tmp) throws IOException {
        // Activar el nivel FINE en el logger de SqliteConnection para que
        // el supplier del LOG.fine(() -> ...) en open(String) sea ejecutado.
        var logger = Logger.getLogger(SqliteConnection.class.getName());
        var prevLevel = logger.getLevel();
        logger.setLevel(Level.FINE);
        try {
            var path = tmp.toRealPath().resolve("fine.db").toString();
            try (var db = SqliteConnection.open(path)) {
                assertTrue(db.ping());
            }
        } finally {
            logger.setLevel(prevLevel);
        }
    }

    // ── sqliteVersion ─────────────────────────────────────────────────────────

    @Test
    void sqliteVersion_returnsNonEmpty() {
        var version = SqliteConnection.sqliteVersion();
        assertNotNull(version);
        assertFalse(version.isBlank());
        // SQLite siempre empieza por "3."
        assertTrue(version.startsWith("3."), "Versión inesperada: " + version);
    }

    // ── ping ──────────────────────────────────────────────────────────────────

    @Test
    void ping_returnsTrue() {
        try (var db = SqliteConnection.memory()) {
            assertTrue(db.ping());
        }
    }

    @Test
    void ping_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::ping);
    }

    // ── exec ──────────────────────────────────────────────────────────────────

    @Test
    void exec_ddl_ok() {
        try (var db = SqliteConnection.memory()) {
            assertDoesNotThrow(() -> db.exec("CREATE TABLE t (x INTEGER)"));
        }
    }

    @Test
    void exec_invalidSql_throws() {
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class, () -> db.exec("NOT VALID SQL !!!"));
        }
    }

    @Test
    void exec_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.exec("SELECT 1"));
    }

    @Test
    void exec_returnsThis_forChaining() {
        try (var db = SqliteConnection.memory()) {
            assertSame(db, db.exec("CREATE TABLE t (x INTEGER)"));
        }
    }

    // ── prepare ───────────────────────────────────────────────────────────────

    @Test
    void prepare_invalidSql_throws() {
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class, () -> db.prepare("GARBAGE SQL !!@#"));
        }
    }

    @Test
    void prepare_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.prepare("SELECT 1"));
    }

    // ── lastInsertRowid / changes ─────────────────────────────────────────────

    @Test
    void lastInsertRowid_afterInsert() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.exec("INSERT INTO t VALUES (99)");
            assertEquals(1L, db.lastInsertRowid());
        }
    }

    @Test
    void lastInsertRowid_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::lastInsertRowid);
    }

    @Test
    void changes_afterInsert() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.exec("INSERT INTO t VALUES (1)");
            assertEquals(1L, db.changes());
        }
    }

    @Test
    void changes_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::changes);
    }

    // ── busyTimeout ───────────────────────────────────────────────────────────

    @Test
    void busyTimeout_setsSuccessfully() {
        try (var db = SqliteConnection.memory()) {
            assertSame(db, db.busyTimeout(5000));
        }
    }

    @Test
    void busyTimeout_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.busyTimeout(1000));
    }

    // ── enableWal / walCheckpoint / walAutocheckpoint ─────────────────────────

    @Test
    void enableWal_onMemory_ok() {
        // WAL en memoria no tiene efecto pero no lanza excepción
        try (var db = SqliteConnection.memory()) {
            assertSame(db, db.enableWal());
        }
    }

    @Test
    void walCheckpoint_onWalDb(@TempDir Path tmp) throws IOException {
        var path = tmp.toRealPath().resolve("wal.db").toString();
        try (var db = SqliteConnection.open(path)) {
            db.enableWal();
            db.exec("CREATE TABLE t (x INTEGER)");
            db.exec("INSERT INTO t VALUES (1)");
            var result = db.walCheckpoint(SqliteConnection.WalMode.PASSIVE, null);
            assertNotNull(result);
            // walFrames y checkpointed son >= 0
            assertTrue(result.walFrames() >= 0);
            assertTrue(result.checkpointed() >= 0);
        }
    }

    @Test
    void walCheckpoint_invalidDbName_throws() {
        // Pasar un dbName que no corresponde a ninguna BD adjunta —
        // sqlite3_wal_checkpoint_v2 retorna SQLITE_ERROR y Rust propaga el error.
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class,
                () -> db.walCheckpoint(SqliteConnection.WalMode.PASSIVE, "nonexistent_db_xyz"));
        }
    }

    @Test
    void walCheckpoint_afterClose_throws(@TempDir Path tmp) throws IOException {
        var path = tmp.toRealPath().resolve("wal2.db").toString();
        try (var db = SqliteConnection.open(path)) {
            db.enableWal();
        }
        var db2 = SqliteConnection.open(path);
        db2.close();
        assertThrows(SqliteException.class,
            () -> db2.walCheckpoint(SqliteConnection.WalMode.PASSIVE, null));
    }

    @Test
    void walAutocheckpoint_ok() {
        try (var db = SqliteConnection.memory()) {
            assertSame(db, db.walAutocheckpoint(500));
        }
    }

    @Test
    void walAutocheckpoint_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.walAutocheckpoint(500));
    }

    // ── Transacciones ─────────────────────────────────────────────────────────

    @Test
    void transaction_commit() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.transaction(() -> db.exec("INSERT INTO t VALUES (1)"));
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(1L, count);
        }
    }

    @Test
    void transaction_rollbackOnException() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            assertThrows(RuntimeException.class, () ->
                db.transaction(() -> {
                    db.exec("INSERT INTO t VALUES (1)");
                    throw new RuntimeException("forzar rollback");
                })
            );
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(0L, count);
        }
    }

    @Test
    void transactionImmediate_commit() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.transactionImmediate(() -> db.exec("INSERT INTO t VALUES (2)"));
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(1L, count);
        }
    }

    @Test
    void transactionImmediate_rollbackOnException() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            assertThrows(RuntimeException.class, () ->
                db.transactionImmediate(() -> {
                    db.exec("INSERT INTO t VALUES (99)");
                    throw new RuntimeException("abort");
                })
            );
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(0L, count);
        }
    }

    @Test
    void begin_commit_ok() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.begin();
            db.exec("INSERT INTO t VALUES (1)");
            db.commit();
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(1L, count);
        }
    }

    @Test
    void begin_rollback_ok() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.begin();
            db.exec("INSERT INTO t VALUES (1)");
            db.rollback();
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(0L, count);
        }
    }

    @Test
    void beginImmediate_ok() {
        try (var db = SqliteConnection.memory()) {
            db.beginImmediate();
            db.rollback();
        }
    }

    @Test
    void beginExclusive_ok() {
        try (var db = SqliteConnection.memory()) {
            db.beginExclusive();
            db.rollback();
        }
    }

    @Test
    void begin_insideTransaction_throws() {
        // Segundo BEGIN dentro de una transacción activa → SQLITE_ERROR
        try (var db = SqliteConnection.memory()) {
            db.begin();
            assertThrows(SqliteException.class, db::begin);
            db.rollback();
        }
    }

    @Test
    void beginImmediate_insideTransaction_throws() {
        try (var db = SqliteConnection.memory()) {
            db.begin();
            assertThrows(SqliteException.class, db::beginImmediate);
            db.rollback();
        }
    }

    @Test
    void beginExclusive_insideTransaction_throws() {
        try (var db = SqliteConnection.memory()) {
            db.begin();
            assertThrows(SqliteException.class, db::beginExclusive);
            db.rollback();
        }
    }

    @Test
    void commit_noTransaction_throws() {
        // COMMIT sin transacción activa → "cannot commit - no transaction is active"
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class, db::commit);
        }
    }

    @Test
    void rollback_noTransaction_throws() {
        // ROLLBACK sin transacción activa → SQLITE_ERROR
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class, db::rollback);
        }
    }

    @Test
    void begin_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::begin);
    }

    @Test
    void beginImmediate_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::beginImmediate);
    }

    @Test
    void beginExclusive_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::beginExclusive);
    }

    @Test
    void commit_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::commit);
    }

    @Test
    void rollback_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::rollback);
    }

    // ── Savepoints ─────────────────────────────────────────────────────────────

    @Test
    void savepoint_release_ok() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.savepoint("sp1");
            db.exec("INSERT INTO t VALUES (1)");
            db.release("sp1");
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(1L, count);
        }
    }

    @Test
    void savepoint_rollbackTo_ok() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.savepoint("sp2");
            db.exec("INSERT INTO t VALUES (1)");
            db.rollbackTo("sp2");
            db.release("sp2");
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(0L, count);
        }
    }

    @Test
    void withSavepoint_commit() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            db.withSavepoint("sp3", () -> db.exec("INSERT INTO t VALUES (7)"));
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(1L, count);
        }
    }

    @Test
    void withSavepoint_rollbackOnException() {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            assertThrows(RuntimeException.class, () ->
                db.withSavepoint("sp4", () -> {
                    db.exec("INSERT INTO t VALUES (7)");
                    throw new RuntimeException("abort savepoint");
                })
            );
            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
                q.step();
                count = q.columnInt(0);
            }
            assertEquals(0L, count);
        }
    }

    @Test
    void release_nonexistent_throws() {
        // RELEASE de un savepoint que no existe → SQLITE_ERROR
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class, () -> db.release("no_existe_xyz"));
        }
    }

    @Test
    void rollbackTo_nonexistent_throws() {
        // ROLLBACK TO de un savepoint que no existe → SQLITE_ERROR
        try (var db = SqliteConnection.memory()) {
            assertThrows(SqliteException.class, () -> db.rollbackTo("no_existe_xyz"));
        }
    }

    @Test
    void savepoint_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.savepoint("sp"));
    }

    @Test
    void release_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.release("sp"));
    }

    @Test
    void rollbackTo_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, () -> db.rollbackTo("sp"));
    }

    // ── rawHandle ─────────────────────────────────────────────────────────────

    @Test
    void rawHandle_returnsNonNull() {
        try (var db = SqliteConnection.memory()) {
            var h = db.rawHandle();
            assertNotNull(h);
            assertFalse(h.equals(java.lang.foreign.MemorySegment.NULL));
        }
    }

    @Test
    void rawHandle_afterClose_throws() {
        var db = SqliteConnection.memory();
        db.close();
        assertThrows(SqliteException.class, db::rawHandle);
    }

    // ── close e idempotencia ───────────────────────────────────────────────────

    @Test
    void close_isIdempotent() {
        var db = SqliteConnection.memory();
        db.close();
        assertDoesNotThrow(db::close);  // segunda llamada no debe lanzar
    }

    @Test
    void close_withOpenStatements_logsWarning() {
        // Cerrar la conexión con un statement aún abierto activa LOG.warning().
        // El statement se cierra después para no dejar resource leak real.
        var db = SqliteConnection.memory();
        db.exec("CREATE TABLE t (x INTEGER)");
        var stmt = db.prepare("SELECT * FROM t");
        // Cerramos la conexión SIN cerrar el statement → dispara LOG.warning
        db.close();
        // Ahora cerramos el statement (ya sin conexión, pero Rust lo maneja)
        assertDoesNotThrow(stmt::close);
    }

    // ── lastError ─────────────────────────────────────────────────────────────

    @Test
    void lastError_returnsStringOrNull() {
        // Provocar un error para asegurarnos de que lastError funciona
        try (var db = SqliteConnection.memory()) {
            try {
                db.exec("BAD SQL !!!");
            } catch (SqliteException ignored) {}
        }
        // lastError estático debería devolver algo (o null si fue limpiado)
        // La prueba principal es que no lanza excepción
        assertDoesNotThrow(SqliteConnection::lastError);
    }
}
