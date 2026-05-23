import mx.rafex.sqlite.SqliteConnection;
import mx.rafex.sqlite.SqliteException;

import java.io.File;
import java.nio.file.Files;

/**
 * Suite de pruebas para validar los artefactos del GitHub Release.
 *
 * Ejecutado por el Dockerfile en tres modos:
 *   1. JVM  + thin JAR
 *   2. JVM  + fat JAR
 *   3. GraalVM Native Image (binario compilado)
 *
 * Cubre toda la superficie de la API pública de sqlite-native-runtime:
 *   - SqliteConnection.open() / memory()
 *   - DDL, INSERT, SELECT, batch, parámetros nombrados
 *   - lastInsertRowid(), changes()
 *   - Transacciones automáticas y manuales
 *   - Savepoints (commit + rollback)
 *   - WAL, walCheckpoint, walAutocheckpoint
 *   - Tipos BLOB
 *   - Metadatos de columna (count, name)
 *   - Manejo de errores (SqliteException)
 *   - Reset + clearBindings en prepared statements
 *   - Apertura de BD en disco
 */
public class Main {

    private static int passed = 0;
    private static int failed = 0;

    @FunctionalInterface
    interface TestFn { void run() throws Exception; }

    public static void main(String[] args) {
        String mode = System.getProperty("snr.test.mode", "JVM");
        System.out.println("┌──────────────────────────────────────────────┐");
        System.out.printf( "│  sqlite-native-runtime — Release Test %-7s│%n", "[" + mode + "]");
        System.out.println("└──────────────────────────────────────────────┘");
        System.out.println();

        // ── Conectividad básica ───────────────────────────────────────────────
        test("SQLite version",           Main::testSqliteVersion);
        test("Open memory DB",           Main::testOpenMemory);
        test("Open disk DB",             Main::testOpenDisk);

        // ── DDL y DML ─────────────────────────────────────────────────────────
        test("DDL exec",                 Main::testDdl);
        test("Single insert + query",    Main::testInsertQuery);
        test("Batch insert",             Main::testBatchInsert);
        test("Named parameters",         Main::testNamedParams);

        // ── Información de filas afectadas ────────────────────────────────────
        test("lastInsertRowid()",        Main::testLastInsertRowid);
        test("changes()",                Main::testChanges);

        // ── Transacciones ─────────────────────────────────────────────────────
        test("Auto transaction",         Main::testAutoTransaction);
        test("Manual transaction",       Main::testManualTransaction);
        test("Transaction rollback",     Main::testTransactionRollback);

        // ── Savepoints ────────────────────────────────────────────────────────
        test("Savepoint commit",         Main::testSavepointCommit);
        test("Savepoint rollback",       Main::testSavepointRollback);

        // ── WAL ───────────────────────────────────────────────────────────────
        test("WAL mode + checkpoint",    Main::testWal);

        // ── Tipos de datos ────────────────────────────────────────────────────
        test("BLOB round-trip",          Main::testBlob);
        test("NULL value",               Main::testNullValue);
        test("All numeric types",        Main::testNumericTypes);

        // ── Metadatos ─────────────────────────────────────────────────────────
        test("Column metadata",          Main::testColumnMetadata);
        test("columnType()",             Main::testColumnType);

        // ── Robustez ─────────────────────────────────────────────────────────
        test("Error handling",           Main::testErrorHandling);
        test("Reset + clearBindings",    Main::testResetRebind);
        test("Multiple open/close",      Main::testMultipleOpenClose);

        // ── Resultados ────────────────────────────────────────────────────────
        System.out.println();
        System.out.printf("  Results: %d passed, %d failed%n", passed, failed);
        System.out.println();

        if (failed > 0) {
            System.err.printf("FAIL — %d test(s) failed%n", failed);
            System.exit(1);
        }
        System.out.println("✓ All tests passed");
    }

    // ── Harness ───────────────────────────────────────────────────────────────

    static void test(String name, TestFn fn) {
        try {
            fn.run();
            System.out.printf("  ✓ %-38s%n", name);
            passed++;
        } catch (Throwable t) {
            System.err.printf("  ✗ %-38s → %s%n", name, t.getMessage());
            if (!(t instanceof AssertionError)) t.printStackTrace(System.err);
            failed++;
        }
    }

    static void check(boolean condition, String msg) {
        if (!condition) throw new AssertionError(msg);
    }

    // ── Conectividad básica ───────────────────────────────────────────────────

    static void testSqliteVersion() throws Exception {
        try (var db = SqliteConnection.memory()) {
            String v = db.sqliteVersion();
            check(v != null && v.startsWith("3"),
                  "Expected SQLite 3.x, got: " + v);
            System.out.printf("       (SQLite %s)%n", v);
        }
    }

    static void testOpenMemory() throws Exception {
        try (var db = SqliteConnection.memory()) {
            check(db != null, "db is null");
            db.exec("SELECT 1");
        }
    }

    static void testOpenDisk() throws Exception {
        File tmp = Files.createTempFile("snr-disk-", ".db").toFile();
        try {
            try (var db = SqliteConnection.open(tmp.getAbsolutePath())) {
                db.exec("CREATE TABLE t (x INTEGER)");
                db.exec("INSERT INTO t VALUES(42)");
            }
            // reopen and verify persistence
            try (var db = SqliteConnection.open(tmp.getAbsolutePath())) {
                try (var q = db.prepare("SELECT x FROM t")) {
                    check(q.step(), "no row after reopen");
                    check(q.columnInt(0) == 42, "persisted value != 42");
                }
            }
        } finally {
            tmp.delete();
        }
    }

    // ── DDL y DML ─────────────────────────────────────────────────────────────

    static void testDdl() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("""
                CREATE TABLE IF NOT EXISTS productos (
                    id     INTEGER PRIMARY KEY,
                    nombre TEXT    NOT NULL,
                    precio REAL    NOT NULL,
                    stock  INTEGER DEFAULT 0
                )
            """);
        }
    }

    static void testInsertQuery() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, val REAL)");

            try (var s = db.prepare("INSERT INTO t(name, val) VALUES(?, ?)")) {
                s.bindText(1, "sqlite-native-runtime")
                 .bindDouble(2, 3.14159)
                 .stepAndDone();
            }

            try (var q = db.prepare("SELECT id, name, val FROM t")) {
                check(q.step(), "expected a row");
                check(q.columnInt(0) == 1, "id != 1, got " + q.columnInt(0));
                check("sqlite-native-runtime".equals(q.columnText(1)),
                      "name mismatch: " + q.columnText(1));
                check(Math.abs(q.columnDouble(2) - 3.14159) < 1e-9,
                      "val mismatch: " + q.columnDouble(2));
                check(!q.step(), "expected only one row");
            }
        }
    }

    static void testBatchInsert() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (n INTEGER, label TEXT)");

            try (var s = db.prepare("INSERT INTO t VALUES(?, ?)")) {
                for (int i = 1; i <= 100; i++) {
                    s.bindInt(1, i).bindText(2, "item-" + i).stepAndDone();
                    s.reset().clearBindings();
                }
            }

            try (var q = db.prepare("SELECT count(*), sum(n) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 100, "count != 100, got " + q.columnInt(0));
                check(q.columnInt(1) == 5050, "sum != 5050, got " + q.columnInt(1));
            }
        }
    }

    static void testNamedParams() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (name TEXT, price REAL)");

            try (var s = db.prepare("INSERT INTO t VALUES(:name, :price)")) {
                s.bindText(s.parameterIndex(":name"), "laptop")
                 .bindDouble(s.parameterIndex(":price"), 999.99)
                 .stepAndDone();
            }

            try (var q = db.prepare("SELECT price FROM t WHERE name = :n")) {
                q.bindText(q.parameterIndex(":n"), "laptop");
                check(q.step(), "no row");
                check(Math.abs(q.columnDouble(0) - 999.99) < 1e-9,
                      "price mismatch: " + q.columnDouble(0));
            }
        }
    }

    // ── Filas afectadas ───────────────────────────────────────────────────────

    static void testLastInsertRowid() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, x TEXT)");

            try (var s = db.prepare("INSERT INTO t(x) VALUES(?)")) {
                s.bindText(1, "first").stepAndDone();
                check(db.lastInsertRowid() == 1, "rowid != 1, got " + db.lastInsertRowid());

                s.reset();
                s.bindText(1, "second").stepAndDone();
                check(db.lastInsertRowid() == 2, "rowid != 2, got " + db.lastInsertRowid());
            }
        }
    }

    static void testChanges() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)");
            db.exec("INSERT INTO t VALUES(1,10),(2,20),(3,30)");
            db.exec("UPDATE t SET v = v * 2 WHERE v > 10");
            check(db.changes() == 2, "changes != 2, got " + db.changes());
            db.exec("DELETE FROM t");
            check(db.changes() == 3, "changes != 3 after delete, got " + db.changes());
        }
    }

    // ── Transacciones ─────────────────────────────────────────────────────────

    static void testAutoTransaction() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v INTEGER)");

            db.transaction(() -> {
                try (var s = db.prepare("INSERT INTO t VALUES(?)")) {
                    s.bindInt(1, 42).stepAndDone();
                    s.reset();
                    s.bindInt(1, 43).stepAndDone();
                }
            });

            try (var q = db.prepare("SELECT count(*), sum(v) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 2, "count != 2");
                check(q.columnInt(1) == 85, "sum != 85, got " + q.columnInt(1));
            }
        }
    }

    static void testManualTransaction() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v INTEGER)");

            db.beginImmediate();
            try {
                db.exec("INSERT INTO t VALUES(99)");
                db.exec("INSERT INTO t VALUES(100)");
                db.commit();
            } catch (Exception e) {
                db.rollback();
                throw e;
            }

            try (var q = db.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 2, "expected 2 rows after commit");
            }
        }
    }

    static void testTransactionRollback() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v INTEGER)");
            db.exec("INSERT INTO t VALUES(1)");

            db.begin();
            db.exec("INSERT INTO t VALUES(2)");
            db.rollback();

            try (var q = db.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 1, "rollback failed: expected 1 row, got " + q.columnInt(0));
            }
        }
    }

    // ── Savepoints ────────────────────────────────────────────────────────────

    static void testSavepointCommit() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v INTEGER)");
            db.exec("INSERT INTO t VALUES(1)");

            db.withSavepoint("sp_commit", () ->
                db.exec("INSERT INTO t VALUES(2)")
            );

            try (var q = db.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 2,
                      "savepoint commit failed: expected 2, got " + q.columnInt(0));
            }
        }
    }

    static void testSavepointRollback() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v INTEGER)");
            db.exec("INSERT INTO t VALUES(1)");

            try {
                db.withSavepoint("sp_rollback", () -> {
                    db.exec("INSERT INTO t VALUES(2)");
                    throw new RuntimeException("forzar rollback");
                });
            } catch (RuntimeException ignored) {}

            try (var q = db.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 1,
                      "savepoint rollback failed: expected 1, got " + q.columnInt(0));
            }
        }
    }

    // ── WAL ───────────────────────────────────────────────────────────────────

    static void testWal() throws Exception {
        File tmp = Files.createTempFile("snr-wal-", ".db").toFile();
        try {
            try (var db = SqliteConnection.open(tmp.getAbsolutePath())) {
                db.enableWal();
                db.exec("CREATE TABLE t (v INTEGER)");
                db.exec("INSERT INTO t VALUES(1),(2),(3)");
                db.walAutocheckpoint(1000);
                db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
            }
        } finally {
            tmp.delete();
            new File(tmp.getAbsolutePath() + "-wal").delete();
            new File(tmp.getAbsolutePath() + "-shm").delete();
        }
    }

    // ── Tipos de datos ────────────────────────────────────────────────────────

    static void testBlob() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (data BLOB)");
            byte[] original = {0, 1, 2, 127, (byte) 128, (byte) 255};

            try (var s = db.prepare("INSERT INTO t VALUES(?)")) {
                s.bindBlob(1, original).stepAndDone();
            }

            try (var q = db.prepare("SELECT data FROM t")) {
                check(q.step(), "no row");
                byte[] retrieved = q.columnBlob(0);
                check(retrieved.length == original.length,
                      "blob length: expected " + original.length + ", got " + retrieved.length);
                for (int i = 0; i < original.length; i++) {
                    check(retrieved[i] == original[i],
                          "blob byte[" + i + "]: expected " + original[i] + ", got " + retrieved[i]);
                }
            }
        }
    }

    static void testNullValue() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v TEXT)");

            try (var s = db.prepare("INSERT INTO t VALUES(?)")) {
                s.bindNull(1).stepAndDone();
            }

            try (var q = db.prepare("SELECT v, typeof(v) FROM t")) {
                check(q.step(), "no row");
                // SQLite type 5 = NULL
                check(q.columnType(0) == 5,
                      "expected NULL type (5), got " + q.columnType(0));
                check("null".equals(q.columnText(1)),
                      "typeof != 'null', got: " + q.columnText(1));
            }
        }
    }

    static void testNumericTypes() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (i INTEGER, r REAL)");

            try (var s = db.prepare("INSERT INTO t VALUES(?, ?)")) {
                s.bindInt(1, Long.MAX_VALUE).bindDouble(2, Double.MIN_VALUE).stepAndDone();
            }

            try (var q = db.prepare("SELECT i, r FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == Long.MAX_VALUE,
                      "long max mismatch: " + q.columnInt(0));
                check(q.columnDouble(1) == Double.MIN_VALUE,
                      "double min mismatch: " + q.columnDouble(1));
            }
        }
    }

    // ── Metadatos ─────────────────────────────────────────────────────────────

    static void testColumnMetadata() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (id INTEGER, nombre TEXT, valor REAL)");
            db.exec("INSERT INTO t VALUES(1, 'test', 2.5)");

            try (var q = db.prepare("SELECT id, nombre, valor FROM t")) {
                check(q.columnCount() == 3, "columnCount != 3, got " + q.columnCount());
                check("id".equals(q.columnName(0)), "col[0] name: " + q.columnName(0));
                check("nombre".equals(q.columnName(1)), "col[1] name: " + q.columnName(1));
                check("valor".equals(q.columnName(2)), "col[2] name: " + q.columnName(2));
            }
        }
    }

    static void testColumnType() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (i INTEGER, r REAL, t TEXT, b BLOB, n TEXT)");
            try (var s = db.prepare("INSERT INTO t VALUES(?, ?, ?, ?, ?)")) {
                s.bindInt(1, 1)
                 .bindDouble(2, 1.5)
                 .bindText(3, "hello")
                 .bindBlob(4, new byte[]{1, 2})
                 .bindNull(5)
                 .stepAndDone();
            }
            try (var q = db.prepare("SELECT i, r, t, b, n FROM t")) {
                check(q.step(), "no row");
                // SQLite types: 1=INTEGER, 2=REAL, 3=TEXT, 4=BLOB, 5=NULL
                check(q.columnType(0) == 1, "INTEGER type expected 1, got " + q.columnType(0));
                check(q.columnType(1) == 2, "REAL type expected 2, got " + q.columnType(1));
                check(q.columnType(2) == 3, "TEXT type expected 3, got " + q.columnType(2));
                check(q.columnType(3) == 4, "BLOB type expected 4, got " + q.columnType(3));
                check(q.columnType(4) == 5, "NULL type expected 5, got " + q.columnType(4));
            }
        }
    }

    // ── Robustez ─────────────────────────────────────────────────────────────

    static void testErrorHandling() throws Exception {
        try (var db = SqliteConnection.memory()) {
            boolean threw = false;
            try {
                db.exec("INVALID SQL STATEMENT");
            } catch (SqliteException e) {
                threw = true;
                check(e.getMessage() != null && !e.getMessage().isBlank(),
                      "SqliteException has blank message");
            }
            check(threw, "expected SqliteException for invalid SQL");

            // La conexión debe seguir usable después del error
            db.exec("SELECT 1");
        }
    }

    static void testResetRebind() throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (v INTEGER)");

            try (var s = db.prepare("INSERT INTO t VALUES(?)")) {
                for (int i = 1; i <= 10; i++) {
                    s.bindInt(1, i).stepAndDone();
                    s.reset().clearBindings();
                }
            }

            try (var q = db.prepare("SELECT count(*), sum(v), min(v), max(v) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 10,  "count != 10");
                check(q.columnInt(1) == 55,  "sum != 55, got " + q.columnInt(1));
                check(q.columnInt(2) == 1,   "min != 1");
                check(q.columnInt(3) == 10,  "max != 10");
            }
        }
    }

    static void testMultipleOpenClose() throws Exception {
        for (int i = 0; i < 5; i++) {
            try (var db = SqliteConnection.memory()) {
                db.exec("CREATE TABLE t (v INTEGER)");
                try (var s = db.prepare("INSERT INTO t VALUES(?)")) {
                    s.bindInt(1, i).stepAndDone();
                }
            }
        }
    }
}
