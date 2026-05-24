package mx.rafex.sqlite;

/**
 * Test de humo — ejecutar con:
 *
 * <pre>
 *   ETHER_SQLITE_LIB=/path/to/libether_sqlite_runtime.dylib \
 *   java --enable-native-access=ALL-UNNAMED -cp target/classes:target/test-classes \
 *        mx.rafex.sqlite.SmokeTest
 * </pre>
 */
public class SmokeTest {

    public static void main(String[] args) {
        System.out.println("=== sqlite-native-runtime smoke test ===");
        System.out.println("SQLite version: " + SqliteConnection.sqliteVersion());

        // Test 1: BD en memoria + DDL + INSERT + SELECT
        try (var db = SqliteConnection.memory()) {
            assert db.ping() : "ping() debe ser true";
            System.out.println("ping OK");

            db.enableWal();
            db.exec("""
                CREATE TABLE kv (
                    id   INTEGER PRIMARY KEY AUTOINCREMENT,
                    key  TEXT NOT NULL UNIQUE,
                    val  TEXT
                )
                """);

            // INSERT con prepared statement
            try (var ins = db.prepare("INSERT INTO kv(key, val) VALUES(?, ?)")) {
                ins.bindText(1, "color").bindText(2, "azul").stepAndDone();
                ins.reset();
                ins.bindText(1, "numero").bindText(2, null).stepAndDone();
            }

            long rowid = db.lastInsertRowid();
            System.out.println("lastInsertRowid = " + rowid);
            assert rowid == 2 : "Esperaba rowid=2, obtuvo " + rowid;

            // SELECT streaming
            System.out.println("SELECT resultado:");
            try (var q = db.prepare("SELECT id, key, val FROM kv ORDER BY id")) {
                while (q.step()) {
                    long   id  = q.columnInt(0);
                    String key = q.columnText(1);
                    String val = q.columnText(2);
                    System.out.printf("  id=%d  key=%s  val=%s%n", id, key, val);
                }
            }

            // Test transacción con rollback
            try {
                db.transaction(() -> {
                    db.exec("INSERT INTO kv(key, val) VALUES('tmp', 'x')");
                    throw new RuntimeException("forzar rollback");
                });
            } catch (RuntimeException ignored) {}

            long count;
            try (var q = db.prepare("SELECT COUNT(*) FROM kv")) {
                q.step();
                count = q.columnInt(0);
            }
            System.out.println("Filas tras rollback: " + count);
            assert count == 2 : "Esperaba 2 filas, obtuvo " + count;

            // Test savepoint
            db.savepoint("sp1");
            db.exec("INSERT INTO kv(key, val) VALUES('sp_key', 'sp_val')");
            db.rollbackTo("sp1");
            db.release("sp1");

            try (var q = db.prepare("SELECT COUNT(*) FROM kv")) {
                q.step();
                count = q.columnInt(0);
            }
            System.out.println("Filas tras rollback de savepoint: " + count);
            assert count == 2 : "Esperaba 2 filas, obtuvo " + count;

            // Test parámetro por nombre
            try (var q = db.prepare("SELECT val FROM kv WHERE key = :k")) {
                int idx = q.parameterIndex(":k");
                System.out.println("Índice de :k = " + idx);
                assert idx == 1 : "Esperaba idx=1";
                q.bindText(idx, "color");
                q.step();
                System.out.println("val para 'color' = " + q.columnText(0));
                assert "azul".equals(q.columnText(0));
            }

            // Test blob
            db.exec("CREATE TABLE blobs (data BLOB)");
            byte[] originalBytes = {0x01, 0x02, 0x03, (byte) 0xFF};
            try (var ins = db.prepare("INSERT INTO blobs(data) VALUES(?)")) {
                ins.bindBlob(1, originalBytes).stepAndDone();
            }
            try (var q = db.prepare("SELECT data FROM blobs")) {
                q.step();
                byte[] read = q.columnBlob(0);
                assert read != null && read.length == 4 : "blob length esperado 4";
                assert read[3] == (byte) 0xFF : "blob byte[3] esperado 0xFF";
                System.out.println("Blob OK, bytes = " + read.length);
            }

        }

        System.out.println("=== TODOS LOS TESTS PASARON ===");
    }
}
