package mx.rafex.ether.sqlite;

import java.lang.foreign.MemorySegment;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Nested;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests unitarios para {@link SqliteStatement}.
 * Cubre binds, step, stepAndDone, reset, clearBindings,
 * columnXxx, columnName, parameterIndex y rawHandle.
 */
class SqliteStatementTest {

    private SqliteConnection db;

    @BeforeEach
    void setUp() {
        db = FfmJava21SqliteConnection.memory();
        db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, txt TEXT, num REAL, blob BLOB)");
    }

    @AfterEach
    void tearDown() {
        db.close();
    }

    // ── bindNull ──────────────────────────────────────────────────────────────

    @Test
    void bindNull_outOfRange_throws() {
        // SQLite retorna SQLITE_RANGE para un índice fuera de los parámetros del statement
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES (?)")) {
            assertThrows(SqliteException.class, () -> stmt.bindNull(999));
        }
    }

    @Test
    void bindNull_ok() {
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES(?)")) {
            assertSame(stmt, stmt.bindNull(1));
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT txt FROM t WHERE id = 1")) {
            assertTrue(q.step());
            assertNull(q.columnText(0));
        }
    }

    @Test
    void bindNull_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.bindNull(1));
    }

    // ── bindInt (long) ────────────────────────────────────────────────────────

    @Test
    void bindInt_outOfRange_throws() {
        try (var stmt = db.prepare("SELECT ?")) {
            assertThrows(SqliteException.class, () -> stmt.bindInt(999, 1L));
        }
    }

    @Test
    void bindInt_long_ok() {
        db.exec("CREATE TABLE nums (n INTEGER)");
        try (var stmt = db.prepare("INSERT INTO nums(n) VALUES(?)")) {
            assertSame(stmt, stmt.bindInt(1, 9876543210L));
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT n FROM nums")) {
            assertTrue(q.step());
            assertEquals(9876543210L, q.columnInt(0));
        }
    }

    @Test
    void bindInt_int_ok() {
        db.exec("CREATE TABLE nums (n INTEGER)");
        try (var stmt = db.prepare("INSERT INTO nums(n) VALUES(?)")) {
            assertSame(stmt, stmt.bindInt(1, 42));
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT n FROM nums")) {
            assertTrue(q.step());
            assertEquals(42L, q.columnInt(0));
        }
    }

    @Test
    void bindInt_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.bindInt(1, 1L));
    }

    // ── bindDouble ────────────────────────────────────────────────────────────

    @Test
    void bindDouble_outOfRange_throws() {
        try (var stmt = db.prepare("SELECT ?")) {
            assertThrows(SqliteException.class, () -> stmt.bindDouble(999, 1.0));
        }
    }

    @Test
    void bindDouble_ok() {
        try (var stmt = db.prepare("INSERT INTO t(num) VALUES(?)")) {
            assertSame(stmt, stmt.bindDouble(1, 3.14));
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT num FROM t")) {
            assertTrue(q.step());
            assertEquals(3.14, q.columnDouble(0), 1e-9);
        }
    }

    @Test
    void bindDouble_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.bindDouble(1, 1.0));
    }

    // ── bindText ──────────────────────────────────────────────────────────────

    @Test
    void bindText_outOfRange_throws() {
        try (var stmt = db.prepare("SELECT ?")) {
            assertThrows(SqliteException.class, () -> stmt.bindText(999, "x"));
        }
    }

    @Test
    void bindText_ok() {
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES(?)")) {
            assertSame(stmt, stmt.bindText(1, "hola mundo"));
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertEquals("hola mundo", q.columnText(0));
        }
    }

    @Test
    void bindText_null_bindsNull() {
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES(?)")) {
            stmt.bindText(1, null).stepAndDone();
        }
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertNull(q.columnText(0));
        }
    }

    @Test
    void bindText_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.bindText(1, "x"));
    }

    // ── bindBlob ──────────────────────────────────────────────────────────────

    @Test
    void bindBlob_outOfRange_throws() {
        try (var stmt = db.prepare("SELECT ?")) {
            assertThrows(SqliteException.class, () -> stmt.bindBlob(999, new byte[]{1}));
        }
    }

    @Test
    void bindBlob_ok() {
        byte[] data = {0x01, 0x02, (byte) 0xFF};
        try (var stmt = db.prepare("INSERT INTO t(blob) VALUES(?)")) {
            assertSame(stmt, stmt.bindBlob(1, data));
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT blob FROM t")) {
            assertTrue(q.step());
            byte[] result = q.columnBlob(0);
            assertNotNull(result);
            assertArrayEquals(data, result);
        }
    }

    @Test
    void bindBlob_null_bindsNull() {
        try (var stmt = db.prepare("INSERT INTO t(blob) VALUES(?)")) {
            stmt.bindBlob(1, (byte[]) null).stepAndDone();
        }
        try (var q = db.prepare("SELECT blob FROM t")) {
            assertTrue(q.step());
            assertNull(q.columnBlob(0));
        }
    }

    @Test
    void bindBlob_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.bindBlob(1, new byte[]{1}));
    }

    // ── parameterIndex ────────────────────────────────────────────────────────

    @Test
    void parameterIndex_knownName() {
        try (var stmt = db.prepare("SELECT * FROM t WHERE txt = :name")) {
            int idx = stmt.parameterIndex(":name");
            assertEquals(1, idx);
        }
    }

    @Test
    void parameterIndex_unknownName_returnsZero() {
        try (var stmt = db.prepare("SELECT 1")) {
            assertEquals(0, stmt.parameterIndex(":nope"));
        }
    }

    @Test
    void parameterIndex_null_throws() {
        try (var stmt = db.prepare("SELECT 1")) {
            assertThrows(SqliteException.class, () -> stmt.parameterIndex(null));
        }
    }

    @Test
    void parameterIndex_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.parameterIndex(":x"));
    }

    // ── step ──────────────────────────────────────────────────────────────────

    @Test
    void step_returnsTrue_whenRowAvailable() {
        db.exec("INSERT INTO t(txt) VALUES ('a')");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertFalse(q.step());  // DONE
        }
    }

    @Test
    void step_error_throws() {
        // Trigger que fuerza SQLITE_ABORT en el INSERT
        db.exec("""
            CREATE TRIGGER abort_trig BEFORE INSERT ON t
            BEGIN SELECT RAISE(ABORT, 'forced error'); END
            """);
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES ('x')")) {
            assertThrows(SqliteException.class, stmt::step);
        }
    }

    @Test
    void step_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, stmt::step);
    }

    // ── stepAndDone ───────────────────────────────────────────────────────────

    @Test
    void stepAndDone_ok() {
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES ('ok')")) {
            assertDoesNotThrow(stmt::stepAndDone);
        }
    }

    @Test
    void stepAndDone_unexpectedRows_throws() {
        // SELECT devuelve filas — stepAndDone debe lanzar
        db.exec("INSERT INTO t(txt) VALUES ('row')");
        try (var stmt = db.prepare("SELECT * FROM t")) {
            assertThrows(SqliteException.class, stmt::stepAndDone);
        }
    }

    @Test
    void stepAndDone_error_throws() {
        db.exec("""
            CREATE TRIGGER abort_trig2 BEFORE INSERT ON t
            BEGIN SELECT RAISE(ABORT, 'forzado'); END
            """);
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES ('y')")) {
            assertThrows(SqliteException.class, stmt::stepAndDone);
        }
    }

    @Test
    void stepAndDone_afterClose_throws() {
        var stmt = db.prepare("INSERT INTO t(txt) VALUES ('z')");
        stmt.close();
        assertThrows(SqliteException.class, stmt::stepAndDone);
    }

    // ── reset ─────────────────────────────────────────────────────────────────

    @Test
    void reset_afterStepError_throws() {
        // Después de que step() falla (trigger ABORT), sqlite3_reset() retorna
        // el error del último step (SQLITE_ABORT != SQLITE_OK) → Java lanza excepción.
        db.exec("""
            CREATE TRIGGER abort_trig_reset BEFORE INSERT ON t
            BEGIN SELECT RAISE(ABORT, 'forced reset error'); END
            """);
        var stmt = db.prepare("INSERT INTO t(txt) VALUES ('x')");
        try {
            stmt.step();  // provoca SQLITE_ABORT
        } catch (SqliteException ignored) {}
        try {
            assertThrows(SqliteException.class, stmt::reset);
        } finally {
            stmt.close();
        }
    }

    @Test
    void reset_allowsReuse() {
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES (?)")) {
            stmt.bindText(1, "uno").stepAndDone();
            stmt.reset();
            stmt.bindText(1, "dos").stepAndDone();
        }
        long count;
        try (var q = db.prepare("SELECT COUNT(*) FROM t")) {
            q.step();
            count = q.columnInt(0);
        }
        assertEquals(2L, count);
    }

    @Test
    void reset_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, stmt::reset);
    }

    // ── clearBindings ─────────────────────────────────────────────────────────

    @Test
    void clearBindings_ok() {
        try (var stmt = db.prepare("INSERT INTO t(txt) VALUES (?)")) {
            stmt.bindText(1, "antes");
            assertSame(stmt, stmt.clearBindings());
            // Tras clearBindings, el parámetro es NULL
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertNull(q.columnText(0));
        }
    }

    @Test
    void clearBindings_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, stmt::clearBindings);
    }

    // ── columnCount ───────────────────────────────────────────────────────────

    @Test
    void columnCount_ok() {
        try (var q = db.prepare("SELECT id, txt, num FROM t")) {
            assertEquals(3, q.columnCount());
        }
    }

    @Test
    void columnCount_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, stmt::columnCount);
    }

    // ── columnType ────────────────────────────────────────────────────────────

    @Test
    void columnType_integerAndText() {
        db.exec("INSERT INTO t(txt, num) VALUES ('hello', 3.14)");
        try (var q = db.prepare("SELECT id, txt, num, blob FROM t")) {
            assertTrue(q.step());
            assertEquals(SqliteStatement.TYPE_INTEGER, q.columnType(0)); // id
            assertEquals(SqliteStatement.TYPE_TEXT,    q.columnType(1)); // txt
            assertEquals(SqliteStatement.TYPE_FLOAT,   q.columnType(2)); // num
            assertEquals(SqliteStatement.TYPE_NULL,    q.columnType(3)); // blob (null)
        }
    }

    @Test
    void columnType_blob() {
        byte[] data = {1, 2, 3};
        try (var stmt = db.prepare("INSERT INTO t(blob) VALUES (?)")) {
            stmt.bindBlob(1, data).stepAndDone();
        }
        try (var q = db.prepare("SELECT blob FROM t")) {
            assertTrue(q.step());
            assertEquals(SqliteStatement.TYPE_BLOB, q.columnType(0));
        }
    }

    @Test
    void columnType_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnType(0));
    }

    // ── columnInt ────────────────────────────────────────────────────────────

    @Test
    void columnInt_ok() {
        db.exec("INSERT INTO t(txt) VALUES ('x')");
        try (var q = db.prepare("SELECT id FROM t")) {
            assertTrue(q.step());
            assertEquals(1L, q.columnInt(0));
        }
    }

    @Test
    void columnInt_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnInt(0));
    }

    // ── columnDouble ──────────────────────────────────────────────────────────

    @Test
    void columnDouble_ok() {
        try (var stmt = db.prepare("INSERT INTO t(num) VALUES (?)")) {
            stmt.bindDouble(1, 2.718).stepAndDone();
        }
        try (var q = db.prepare("SELECT num FROM t")) {
            assertTrue(q.step());
            assertEquals(2.718, q.columnDouble(0), 1e-9);
        }
    }

    @Test
    void columnDouble_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnDouble(0));
    }

    // ── columnText ────────────────────────────────────────────────────────────

    @Test
    void columnText_nonNull() {
        db.exec("INSERT INTO t(txt) VALUES ('mundo')");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertEquals("mundo", q.columnText(0));
        }
    }

    @Test
    void columnText_null_returnsNull() {
        db.exec("INSERT INTO t(txt) VALUES (NULL)");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertNull(q.columnText(0));
        }
    }

    @Test
    void columnText_emptyString() {
        db.exec("INSERT INTO t(txt) VALUES ('')");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertEquals("", q.columnText(0));
        }
    }

    @Test
    void columnText_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnText(0));
    }

    // ── columnTextSafe ────────────────────────────────────────────────────────

    @Test
    void columnTextSafe_nonNull() {
        db.exec("INSERT INTO t(txt) VALUES ('seguro')");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertEquals("seguro", q.columnTextSafe(0));
        }
    }

    @Test
    void columnTextSafe_null_returnsNull() {
        db.exec("INSERT INTO t(txt) VALUES (NULL)");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertNull(q.columnTextSafe(0));
        }
    }

    @Test
    void columnTextSafe_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnTextSafe(0));
    }

    // ── columnBlob ────────────────────────────────────────────────────────────

    @Test
    void columnBlob_emptyBlob() {
        // SQLite retorna puntero NULL para blobs de longitud 0 (X'').
        // columnBlob() devuelve null cuando el puntero interno es NULL,
        // lo que hace indistinguible un blob vacío de un NULL a nivel Java.
        // Para distinguirlos hay que usar columnType() == TYPE_BLOB.
        try (var stmt = db.prepare("INSERT INTO t(blob) VALUES (X'')")) {
            stmt.stepAndDone();
        }
        try (var q = db.prepare("SELECT blob FROM t")) {
            assertTrue(q.step());
            // Tipo de columna: BLOB (no NULL)
            assertEquals(SqliteStatement.TYPE_BLOB, q.columnType(0));
            // columnBytes: 0 bytes
            assertEquals(0, q.columnBytes(0));
            // columnBlob: null por el puntero NULL de SQLite en blob vacío
            assertNull(q.columnBlob(0));
        }
    }

    @Test
    void columnBlob_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnBlob(0));
    }

    // ── columnBytes ───────────────────────────────────────────────────────────

    @Test
    void columnBytes_forText() {
        db.exec("INSERT INTO t(txt) VALUES ('abc')");
        try (var q = db.prepare("SELECT txt FROM t")) {
            assertTrue(q.step());
            assertEquals(3, q.columnBytes(0));
        }
    }

    @Test
    void columnBytes_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnBytes(0));
    }

    // ── columnName ────────────────────────────────────────────────────────────

    @Test
    void columnName_ok() {
        try (var q = db.prepare("SELECT id, txt AS nombre FROM t")) {
            assertEquals("id",     q.columnName(0));
            assertEquals("nombre", q.columnName(1));
        }
    }

    @Test
    void columnName_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, () -> stmt.columnName(0));
    }

    // ── rawHandle ─────────────────────────────────────────────────────────────

    @Test
    void rawHandle_returnsNonNull() {
        try (var stmt = db.prepare("SELECT 1")) {
            var h = stmt.rawHandle();
            assertNotNull(h);
            assertFalse(h.equals(MemorySegment.NULL));
        }
    }

    @Test
    void rawHandle_afterClose_throws() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertThrows(SqliteException.class, stmt::rawHandle);
    }

    // ── close e idempotencia ──────────────────────────────────────────────────

    @Test
    void close_isIdempotent() {
        var stmt = db.prepare("SELECT 1");
        stmt.close();
        assertDoesNotThrow(stmt::close);
    }

    @Test
    void close_withOnCloseCallback() {
        // El constructor package-private con onClose es el que usa SqliteConnection.prepare()
        // snr_stmt_close(NULL) es no-op en Rust — solo verificamos que el callback se llame
        boolean[] called = {false};
        var stmt = new SqliteStatement(MemorySegment.NULL, () -> called[0] = true);
        stmt.close();
        assertTrue(called[0], "onClose debe llamarse al cerrar");
    }

    @Test
    void close_withoutOnCloseCallback() {
        // Constructor sin callback — cierre sin NPE
        var stmt = new SqliteStatement(MemorySegment.NULL);
        assertDoesNotThrow(stmt::close);
    }

    // ── lastError (estático) ──────────────────────────────────────────────────

    @Test
    void lastError_doesNotThrow() {
        // Simplemente verificamos que el helper no lanza
        assertDoesNotThrow(SqliteStatement::lastError);
    }

    // ── readInternalString / readAndFreeString ─────────────────────────────────

    @Test
    void readInternalString_null_returnsNull() {
        assertNull(SqliteStatement.readInternalString(null));
        assertNull(SqliteStatement.readInternalString(MemorySegment.NULL));
    }

    @Test
    void readAndFreeString_null_returnsNull() {
        assertNull(SqliteStatement.readAndFreeString(null));
        assertNull(SqliteStatement.readAndFreeString(MemorySegment.NULL));
    }
}
