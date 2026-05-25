import mx.rafex.ether.sqlite.FfmSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteException;

import java.io.File;
import java.nio.file.Files;

/**
 * Suite de pruebas para el binding Panama FFM (ether-sqlite-ffm-runtime).
 *
 * Compilar: javac -cp ether-sqlite-ffm-runtime-{VER}-fat.jar MainFfm.java
 * Ejecutar: java --enable-native-access=ALL-UNNAMED \
 *               -cp ether-sqlite-ffm-runtime-{VER}-fat.jar:. MainFfm
 */
public class MainFfm {

    private static int passed = 0;
    private static int failed = 0;

    @FunctionalInterface
    interface TestFn { void run() throws Exception; }

    public static void main(String[] args) {
        String mode = System.getProperty("ether.sqlite.test.mode", "FFM");
        System.out.println("┌──────────────────────────────────────────────┐");
        System.out.printf( "│  ether-sqlite-ffm — Release Test %-10s│%n", "[" + mode + "]");
        System.out.println("└──────────────────────────────────────────────┘");
        System.out.println();

        test("SQLite version",           MainFfm::testSqliteVersion);
        test("Open memory DB",           MainFfm::testOpenMemory);
        test("Open disk DB",             MainFfm::testOpenDisk);
        test("DDL exec",                 MainFfm::testDdl);
        test("Single insert + query",    MainFfm::testInsertQuery);
        test("Batch insert",             MainFfm::testBatchInsert);
        test("Named parameters",         MainFfm::testNamedParams);
        test("lastInsertRowid()",        MainFfm::testLastInsertRowid);
        test("changes()",                MainFfm::testChanges);
        test("Auto transaction",         MainFfm::testAutoTransaction);
        test("Manual transaction",       MainFfm::testManualTransaction);
        test("Transaction rollback",     MainFfm::testTransactionRollback);
        test("Savepoint commit",         MainFfm::testSavepointCommit);
        test("Savepoint rollback",       MainFfm::testSavepointRollback);
        test("WAL mode + checkpoint",    MainFfm::testWal);
        test("BLOB round-trip",          MainFfm::testBlob);
        test("NULL value",               MainFfm::testNullValue);
        test("All numeric types",        MainFfm::testNumericTypes);
        test("Column metadata",          MainFfm::testColumnMetadata);
        test("columnType()",             MainFfm::testColumnType);
        test("Error handling",           MainFfm::testErrorHandling);
        test("Reset + clearBindings",    MainFfm::testResetRebind);
        test("Multiple open/close",      MainFfm::testMultipleOpenClose);

        System.out.println();
        System.out.printf("  Results: %d passed, %d failed%n", passed, failed);
        System.out.println();
        if (failed > 0) {
            System.err.printf("FAIL — %d test(s) failed%n", failed);
            System.exit(1);
        }
        System.out.println("✓ All tests passed");
    }

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

    static SqliteConnection db() { return FfmSqliteConnection.memory(); }

    static void testSqliteVersion() throws Exception {
        try (var conn = db()) {
            String v = conn.sqliteVersion();
            check(v != null && v.startsWith("3"), "Expected SQLite 3.x, got: " + v);
            System.out.printf("       (SQLite %s)%n", v);
        }
    }

    static void testOpenMemory() throws Exception {
        try (var conn = FfmSqliteConnection.memory()) {
            check(conn != null, "db is null");
            conn.exec("SELECT 1");
        }
    }

    static void testOpenDisk() throws Exception {
        File tmp = Files.createTempFile("snr-ffm-disk-", ".db").toFile();
        try {
            try (var conn = FfmSqliteConnection.open(tmp.getAbsolutePath())) {
                conn.exec("CREATE TABLE t (x INTEGER)");
                conn.exec("INSERT INTO t VALUES(42)");
            }
            try (var conn = FfmSqliteConnection.open(tmp.getAbsolutePath())) {
                try (var q = conn.prepare("SELECT x FROM t")) {
                    check(q.step(), "no row after reopen");
                    check(q.columnInt(0) == 42, "persisted value != 42");
                }
            }
        } finally { tmp.delete(); }
    }

    static void testDdl() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, name TEXT, v REAL)");
        }
    }

    static void testInsertQuery() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, val REAL)");
            try (var s = conn.prepare("INSERT INTO t(name,val) VALUES(?,?)")) {
                s.bindText(1, "ffm-runtime").bindDouble(2, 3.14159).stepAndDone();
            }
            try (var q = conn.prepare("SELECT id,name,val FROM t")) {
                check(q.step(), "expected a row");
                check(q.columnInt(0) == 1, "id != 1");
                check("ffm-runtime".equals(q.columnText(1)), "name mismatch");
                check(Math.abs(q.columnDouble(2) - 3.14159) < 1e-9, "val mismatch");
                check(!q.step(), "expected only one row");
            }
        }
    }

    static void testBatchInsert() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (n INTEGER, label TEXT)");
            try (var s = conn.prepare("INSERT INTO t VALUES(?,?)")) {
                for (int i = 1; i <= 100; i++) {
                    s.bindInt(1, i).bindText(2, "item-" + i).stepAndDone();
                    s.reset().clearBindings();
                }
            }
            try (var q = conn.prepare("SELECT count(*), sum(n) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 100, "count != 100");
                check(q.columnInt(1) == 5050, "sum != 5050");
            }
        }
    }

    static void testNamedParams() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (name TEXT, price REAL)");
            try (var s = conn.prepare("INSERT INTO t VALUES(:name,:price)")) {
                s.bindText(s.parameterIndex(":name"), "laptop")
                 .bindDouble(s.parameterIndex(":price"), 999.99).stepAndDone();
            }
            try (var q = conn.prepare("SELECT price FROM t WHERE name = :n")) {
                q.bindText(q.parameterIndex(":n"), "laptop");
                check(q.step(), "no row");
                check(Math.abs(q.columnDouble(0) - 999.99) < 1e-9, "price mismatch");
            }
        }
    }

    static void testLastInsertRowid() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, x TEXT)");
            try (var s = conn.prepare("INSERT INTO t(x) VALUES(?)")) {
                s.bindText(1, "first").stepAndDone();
                check(conn.lastInsertRowid() == 1, "rowid != 1");
                s.reset(); s.bindText(1, "second").stepAndDone();
                check(conn.lastInsertRowid() == 2, "rowid != 2");
            }
        }
    }

    static void testChanges() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)");
            conn.exec("INSERT INTO t VALUES(1,10),(2,20),(3,30)");
            conn.exec("UPDATE t SET v = v*2 WHERE v > 10");
            check(conn.changes() == 2, "changes != 2");
            conn.exec("DELETE FROM t");
            check(conn.changes() == 3, "changes != 3");
        }
    }

    static void testAutoTransaction() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v INTEGER)");
            conn.transaction(() -> {
                try (var s = conn.prepare("INSERT INTO t VALUES(?)")) {
                    s.bindInt(1, 42).stepAndDone();
                    s.reset(); s.bindInt(1, 43).stepAndDone();
                }
            });
            try (var q = conn.prepare("SELECT count(*), sum(v) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 2, "count != 2");
                check(q.columnInt(1) == 85, "sum != 85");
            }
        }
    }

    static void testManualTransaction() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v INTEGER)");
            conn.beginImmediate();
            try { conn.exec("INSERT INTO t VALUES(99)"); conn.commit(); }
            catch (Exception e) { conn.rollback(); throw e; }
            try (var q = conn.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 1, "expected 1 row");
            }
        }
    }

    static void testTransactionRollback() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v INTEGER)");
            conn.exec("INSERT INTO t VALUES(1)");
            conn.begin(); conn.exec("INSERT INTO t VALUES(2)"); conn.rollback();
            try (var q = conn.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == 1, "rollback failed");
            }
        }
    }

    static void testSavepointCommit() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v INTEGER)");
            conn.exec("INSERT INTO t VALUES(1)");
            conn.withSavepoint("sp1", () -> conn.exec("INSERT INTO t VALUES(2)"));
            try (var q = conn.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row"); check(q.columnInt(0) == 2, "savepoint commit failed");
            }
        }
    }

    static void testSavepointRollback() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v INTEGER)");
            conn.exec("INSERT INTO t VALUES(1)");
            try {
                conn.withSavepoint("sp2", () -> {
                    conn.exec("INSERT INTO t VALUES(2)");
                    throw new RuntimeException("force rollback");
                });
            } catch (RuntimeException ignored) {}
            try (var q = conn.prepare("SELECT count(*) FROM t")) {
                check(q.step(), "no row"); check(q.columnInt(0) == 1, "savepoint rollback failed");
            }
        }
    }

    static void testWal() throws Exception {
        File tmp = Files.createTempFile("snr-ffm-wal-", ".db").toFile();
        try {
            try (var conn = FfmSqliteConnection.open(tmp.getAbsolutePath())) {
                conn.enableWal();
                conn.exec("CREATE TABLE t (v INTEGER)");
                conn.exec("INSERT INTO t VALUES(1),(2),(3)");
                conn.walAutocheckpoint(1000);
                conn.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
            }
        } finally {
            tmp.delete();
            new File(tmp.getAbsolutePath() + "-wal").delete();
            new File(tmp.getAbsolutePath() + "-shm").delete();
        }
    }

    static void testBlob() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (data BLOB)");
            byte[] original = {0, 1, 2, 127, (byte)128, (byte)255};
            try (var s = conn.prepare("INSERT INTO t VALUES(?)")) {
                s.bindBlob(1, original).stepAndDone();
            }
            try (var q = conn.prepare("SELECT data FROM t")) {
                check(q.step(), "no row");
                byte[] retrieved = q.columnBlob(0);
                check(retrieved.length == original.length, "blob length mismatch");
                for (int i = 0; i < original.length; i++)
                    check(retrieved[i] == original[i], "blob byte[" + i + "] mismatch");
            }
        }
    }

    static void testNullValue() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v TEXT)");
            try (var s = conn.prepare("INSERT INTO t VALUES(?)")) { s.bindNull(1).stepAndDone(); }
            try (var q = conn.prepare("SELECT v, typeof(v) FROM t")) {
                check(q.step(), "no row");
                check(q.columnType(0) == 5, "expected NULL type (5), got " + q.columnType(0));
                check("null".equals(q.columnText(1)), "typeof != 'null'");
            }
        }
    }

    static void testNumericTypes() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (i INTEGER, r REAL)");
            try (var s = conn.prepare("INSERT INTO t VALUES(?,?)")) {
                s.bindInt(1, Long.MAX_VALUE).bindDouble(2, Double.MIN_VALUE).stepAndDone();
            }
            try (var q = conn.prepare("SELECT i,r FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0) == Long.MAX_VALUE, "long max mismatch");
                check(q.columnDouble(1) == Double.MIN_VALUE, "double min mismatch");
            }
        }
    }

    static void testColumnMetadata() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (id INTEGER, nombre TEXT, valor REAL)");
            conn.exec("INSERT INTO t VALUES(1,'test',2.5)");
            try (var q = conn.prepare("SELECT id,nombre,valor FROM t")) {
                check(q.columnCount() == 3, "columnCount != 3");
                check("id".equals(q.columnName(0)), "col[0] name");
                check("nombre".equals(q.columnName(1)), "col[1] name");
                check("valor".equals(q.columnName(2)), "col[2] name");
            }
        }
    }

    static void testColumnType() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (i INTEGER, r REAL, t TEXT, b BLOB, n TEXT)");
            try (var s = conn.prepare("INSERT INTO t VALUES(?,?,?,?,?)")) {
                s.bindInt(1,1).bindDouble(2,1.5).bindText(3,"hi").bindBlob(4,new byte[]{1}).bindNull(5).stepAndDone();
            }
            try (var q = conn.prepare("SELECT i,r,t,b,n FROM t")) {
                check(q.step(), "no row");
                check(q.columnType(0)==1,"INTEGER"); check(q.columnType(1)==2,"REAL");
                check(q.columnType(2)==3,"TEXT");    check(q.columnType(3)==4,"BLOB");
                check(q.columnType(4)==5,"NULL");
            }
        }
    }

    static void testErrorHandling() throws Exception {
        try (var conn = db()) {
            boolean threw = false;
            try { conn.exec("INVALID SQL"); } catch (SqliteException e) {
                threw = true;
                check(e.getMessage() != null && !e.getMessage().isBlank(), "blank message");
            }
            check(threw, "expected SqliteException");
            conn.exec("SELECT 1");
        }
    }

    static void testResetRebind() throws Exception {
        try (var conn = db()) {
            conn.exec("CREATE TABLE t (v INTEGER)");
            try (var s = conn.prepare("INSERT INTO t VALUES(?)")) {
                for (int i = 1; i <= 10; i++) {
                    s.bindInt(1, i).stepAndDone(); s.reset().clearBindings();
                }
            }
            try (var q = conn.prepare("SELECT count(*),sum(v),min(v),max(v) FROM t")) {
                check(q.step(), "no row");
                check(q.columnInt(0)==10,"count"); check(q.columnInt(1)==55,"sum");
                check(q.columnInt(2)==1,"min");    check(q.columnInt(3)==10,"max");
            }
        }
    }

    static void testMultipleOpenClose() throws Exception {
        for (int i = 0; i < 5; i++) {
            try (var conn = FfmSqliteConnection.memory()) {
                conn.exec("CREATE TABLE t (v INTEGER)");
                try (var s = conn.prepare("INSERT INTO t VALUES(?)")) {
                    s.bindInt(1, i).stepAndDone();
                }
            }
        }
    }
}
