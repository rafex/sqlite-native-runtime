package mx.rafex.ether.sqlite;

/**
 * Conexión SQLite de alto nivel.
 *
 * <p>Implementa {@link AutoCloseable} — usar en try-with-resources.
 *
 * <h2>Implementaciones disponibles</h2>
 * <ul>
 *   <li>{@code FfmSqliteConnection} — Panama FFM (Java 25 stable), GraalVM native-image OK</li>
 *   <li>{@code FfmJava21SqliteConnection} — Panama FFM (Java 21 preview), solo JAR</li>
 *   <li>{@code JniSqliteConnection} — JNI (Java 21+), GraalVM native-image OK ✅</li>
 * </ul>
 *
 * <h2>Transacciones (default methods)</h2>
 * <pre>{@code
 * db.transaction(() -> {
 *     try (var stmt = db.prepare("INSERT INTO t(x) VALUES(?)")) {
 *         stmt.bindText(1, "hola").stepAndDone();
 *     }
 * });
 * }</pre>
 *
 * <h2>Savepoints (default methods)</h2>
 * <pre>{@code
 * db.withSavepoint("sp1", () -> {
 *     // rollback automático si lanza excepción
 * });
 * }</pre>
 */
public interface SqliteConnection extends AutoCloseable {

    // ── Tipos anidados ────────────────────────────────────────────────────────

    /** Modo de WAL checkpoint. */
    enum WalMode {
        PASSIVE(0), FULL(1), RESTART(2), TRUNCATE(3);

        public final int value;

        WalMode(int v) { this.value = v; }
    }

    /**
     * Resultado de un WAL checkpoint.
     *
     * @param walFrames    número total de frames en el WAL antes del checkpoint
     * @param checkpointed número de frames efectivamente copiados al fichero principal
     */
    record WalCheckpointResult(int walFrames, int checkpointed) {}

    // ── Operaciones básicas ───────────────────────────────────────────────────

    /**
     * Ejecuta una o más sentencias SQL sin resultado (DDL, PRAGMA, etc.).
     *
     * @throws SqliteException si SQLite reporta un error
     */
    SqliteConnection exec(String sql);

    /**
     * Compila SQL en un prepared statement.
     * El statement DEBE cerrarse — usar en try-with-resources.
     *
     * @throws SqliteException si el SQL no es válido
     */
    SqliteStatement prepare(String sql);

    /** Rowid de la última inserción exitosa en esta conexión. */
    long lastInsertRowid();

    /** Filas modificadas por la última operación DML. */
    long changes();

    /**
     * Verifica que la conexión responde.
     *
     * @return {@code true} si OK
     */
    boolean ping();

    /**
     * Configura el busy timeout.
     *
     * @param ms milisegundos que SQLite esperará un lock antes de SQLITE_BUSY
     */
    SqliteConnection busyTimeout(int ms);

    // ── WAL ──────────────────────────────────────────────────────────────────

    /**
     * Activa el modo WAL y configura {@code synchronous=NORMAL}.
     * Llamar una vez tras abrir la conexión.
     */
    SqliteConnection enableWal();

    /**
     * Ejecuta un WAL checkpoint.
     *
     * @param mode    modo del checkpoint
     * @param dbName  nombre de la BD adjunta, o {@code null} para "main"
     * @return resultado con {@code walFrames} y {@code checkpointed}
     * @throws SqliteException en error
     */
    WalCheckpointResult walCheckpoint(WalMode mode, String dbName);

    /**
     * Configura el auto-checkpoint de WAL.
     *
     * @param n frames de WAL; 0 para desactivar
     */
    SqliteConnection walAutocheckpoint(int n);

    // ── Transacciones ─────────────────────────────────────────────────────────

    /** Inicia BEGIN DEFERRED. */
    SqliteConnection begin();

    /** Inicia BEGIN IMMEDIATE (reserva write lock). */
    SqliteConnection beginImmediate();

    /** Inicia BEGIN EXCLUSIVE. */
    SqliteConnection beginExclusive();

    /** Confirma la transacción activa. */
    SqliteConnection commit();

    /** Revierte la transacción activa. */
    SqliteConnection rollback();

    /**
     * Ejecuta {@code work} dentro de una transacción DEFERRED.
     * Si {@code work} lanza excepción, hace rollback automático.
     */
    default void transaction(Runnable work) {
        begin();
        try {
            work.run();
            commit();
        } catch (Throwable t) {
            try { rollback(); } catch (Throwable ignored) {}
            throw t;
        }
    }

    /**
     * Ejecuta {@code work} dentro de una transacción IMMEDIATE.
     */
    default void transactionImmediate(Runnable work) {
        beginImmediate();
        try {
            work.run();
            commit();
        } catch (Throwable t) {
            try { rollback(); } catch (Throwable ignored) {}
            throw t;
        }
    }

    // ── Savepoints ───────────────────────────────────────────────────────────

    /** Crea un SAVEPOINT con {@code name}. */
    SqliteConnection savepoint(String name);

    /** Libera (confirma) el SAVEPOINT {@code name}. */
    SqliteConnection release(String name);

    /** Revierte hasta el SAVEPOINT {@code name} (sin eliminarlo). */
    SqliteConnection rollbackTo(String name);

    /**
     * Ejecuta {@code work} dentro de un savepoint.
     * Si {@code work} lanza excepción, hace rollback al savepoint y lo libera.
     */
    default void withSavepoint(String name, Runnable work) {
        savepoint(name);
        try {
            work.run();
            release(name);
        } catch (Throwable t) {
            try { rollbackTo(name); } catch (Throwable ignored) {}
            try { release(name); }   catch (Throwable ignored) {}
            throw t;
        }
    }

    // ── AutoCloseable ────────────────────────────────────────────────────────

    @Override
    void close();
}
