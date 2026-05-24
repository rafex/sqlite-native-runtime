package mx.rafex.sqlite;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.logging.Logger;

/**
 * Conexión SQLite de alto nivel sobre {@link SqliteLibrary}.
 *
 * <p>Implementa {@link AutoCloseable} — úsalo en try-with-resources.
 *
 * <h2>Apertura</h2>
 * <pre>{@code
 * try (var db = SqliteConnection.open("/data/app.db")) {
 *     db.exec("PRAGMA journal_mode=WAL");
 *     // ...
 * }
 * }</pre>
 *
 * <h2>Base de datos en memoria</h2>
 * <pre>{@code
 * try (var db = SqliteConnection.memory()) {
 *     // BD efímera, ideal para tests
 * }
 * }</pre>
 *
 * <h2>Transacciones</h2>
 * <pre>{@code
 * db.transaction(() -> {
 *     try (var stmt = db.prepare("INSERT INTO t(x) VALUES(?)")) {
 *         stmt.bindText(1, "hola").stepAndDone();
 *     }
 * });
 * }</pre>
 *
 * <h2>Savepoints</h2>
 * <pre>{@code
 * db.withSavepoint("sp1", () -> {
 *     // operaciones que hacen rollback automático si lanzan excepción
 * });
 * }</pre>
 *
 * <h2>WAL</h2>
 * <pre>{@code
 * db.enableWal();
 * WalCheckpointResult r = db.walCheckpoint(WalMode.TRUNCATE, null);
 * System.out.printf("WAL frames: %d, checkpointed: %d%n", r.walFrames(), r.checkpointed());
 * }</pre>
 */
public final class SqliteConnection implements AutoCloseable {

    /** Modo de WAL checkpoint. */
    public enum WalMode {
        PASSIVE(0), FULL(1), RESTART(2), TRUNCATE(3);
        final int value;
        WalMode(int v) { this.value = v; }
    }

    /**
     * Resultado de un WAL checkpoint (R-4).
     *
     * @param walFrames    número total de frames en el WAL antes del checkpoint
     * @param checkpointed número de frames efectivamente copiados al fichero principal
     */
    public record WalCheckpointResult(int walFrames, int checkpointed) {}

    private static final Logger LOG = Logger.getLogger(SqliteConnection.class.getName());

    private final MemorySegment handle;
    private final AtomicInteger openStatements = new AtomicInteger(0);
    private boolean closed = false;

    private SqliteConnection(MemorySegment handle) {
        this.handle = handle;
    }

    // ── Fábrica ───────────────────────────────────────────────────────────────

    /**
     * Abre (o crea) la base de datos en {@code path}.
     * Por defecto aplica {@link SqliteLibrary#OPEN_NOFOLLOW} para rechazar symlinks.
     *
     * @throws SqliteException si no se puede abrir
     */
    public static SqliteConnection open(String path) {
        var h = SqliteLibrary.snr_open(path, 0);
        if (isNull(h)) {
            throw new SqliteException("No se pudo abrir '%s': %s".formatted(path, lastError()));
        }
        LOG.fine(() -> "SQLite abierto: %s (SQLite %s)".formatted(path, sqliteVersion()));
        return new SqliteConnection(h);
    }

    /**
     * Abre (o crea) la base de datos en {@code path} con flags explícitos.
     * Usar las constantes {@link SqliteLibrary#OPEN_READONLY}, {@link SqliteLibrary#OPEN_READWRITE},
     * {@link SqliteLibrary#OPEN_CREATE}, {@link SqliteLibrary#OPEN_NOFOLLOW}.
     */
    public static SqliteConnection open(String path, int flags) {
        var h = SqliteLibrary.snr_open(path, flags);
        if (isNull(h)) {
            throw new SqliteException("No se pudo abrir '%s': %s".formatted(path, lastError()));
        }
        return new SqliteConnection(h);
    }

    /**
     * Abre una base de datos en memoria anónima ({@code :memory:}).
     * Los datos se pierden al cerrar la conexión.
     */
    public static SqliteConnection memory() {
        var h = SqliteLibrary.snr_open_memory(null);
        if (isNull(h)) {
            throw new SqliteException("No se pudo abrir :memory:: " + lastError());
        }
        return new SqliteConnection(h);
    }

    /**
     * Abre una base de datos en memoria con nombre compartido.
     * Útil para compartir una BD en memoria entre múltiples conexiones en el mismo proceso.
     */
    public static SqliteConnection memory(String name) {
        var h = SqliteLibrary.snr_open_memory(name);
        if (isNull(h)) {
            throw new SqliteException("No se pudo abrir memoria '%s': %s".formatted(name, lastError()));
        }
        return new SqliteConnection(h);
    }

    // ── Operaciones básicas ───────────────────────────────────────────────────

    /**
     * Ejecuta una o más sentencias SQL sin resultado (DDL, PRAGMA, etc.).
     *
     * @throws SqliteException si SQLite reporta un error
     */
    public SqliteConnection exec(String sql) {
        checkOpen();
        if (SqliteLibrary.snr_exec(handle, sql) != 0) {
            throw new SqliteException("exec() falló [%s]: %s".formatted(sql, lastError()));
        }
        return this;
    }

    /**
     * Compila SQL en un prepared statement.
     * El statement DEBE cerrarse — úsalo en try-with-resources.
     * Al cerrar el statement se decrementará automáticamente el contador de statements
     * abiertos de esta conexión.
     *
     * @throws SqliteException si el SQL no es válido
     */
    public SqliteStatement prepare(String sql) {
        checkOpen();
        var stmtHandle = SqliteLibrary.snr_prepare(handle, sql);
        if (isNull(stmtHandle)) {
            throw new SqliteException("prepare() falló [%s]: %s".formatted(sql, lastError()));
        }
        openStatements.incrementAndGet();
        return new SqliteStatement(stmtHandle, openStatements::decrementAndGet);
    }

    /** Rowid de la última inserción exitosa en esta conexión. */
    public long lastInsertRowid() {
        checkOpen();
        return SqliteLibrary.snr_last_insert_rowid(handle);
    }

    /** Filas modificadas por la última operación DML. */
    public long changes() {
        checkOpen();
        return SqliteLibrary.snr_changes(handle);
    }

    /**
     * Configura el busy timeout.
     *
     * @param ms milisegundos que SQLite esperará un lock antes de retornar SQLITE_BUSY
     */
    public SqliteConnection busyTimeout(int ms) {
        checkOpen();
        SqliteLibrary.snr_set_busy_timeout(handle, ms);
        return this;
    }

    /**
     * Verifica que la conexión responde.
     *
     * @return {@code true} si OK
     */
    public boolean ping() {
        checkOpen();
        return SqliteLibrary.snr_ping(handle) == 1L;
    }

    // ── WAL ──────────────────────────────────────────────────────────────────

    /**
     * Activa el modo WAL y configura {@code synchronous=NORMAL}.
     * Llamar una vez tras abrir la conexión.
     */
    public SqliteConnection enableWal() {
        exec("PRAGMA journal_mode=WAL");
        exec("PRAGMA synchronous=NORMAL");
        return this;
    }

    /**
     * Ejecuta un WAL checkpoint y devuelve el resultado de observabilidad (R-4).
     *
     * @param mode    modo del checkpoint
     * @param dbName  nombre de la BD adjunta, o {@code null} para "main"
     * @return resultado con {@code walFrames} y {@code checkpointed}
     * @throws SqliteException en error
     */
    public WalCheckpointResult walCheckpoint(WalMode mode, String dbName) {
        checkOpen();
        try (var arena = Arena.ofConfined()) {
            var outWalFrames    = arena.allocate(ValueLayout.JAVA_INT);
            var outCheckpointed = arena.allocate(ValueLayout.JAVA_INT);
            if (SqliteLibrary.snr_wal_checkpoint(handle, dbName, mode.value,
                    outWalFrames, outCheckpointed) != 0) {
                throw new SqliteException("walCheckpoint(%s) falló: %s".formatted(mode, lastError()));
            }
            return new WalCheckpointResult(
                outWalFrames.get(ValueLayout.JAVA_INT, 0),
                outCheckpointed.get(ValueLayout.JAVA_INT, 0));
        }
    }

    /**
     * Configura el auto-checkpoint de WAL.
     *
     * @param n frames de WAL; 0 para desactivar
     */
    public SqliteConnection walAutocheckpoint(int n) {
        checkOpen();
        SqliteLibrary.snr_wal_autocheckpoint(handle, n);
        return this;
    }

    // ── Transacciones ─────────────────────────────────────────────────────────

    /** Inicia BEGIN DEFERRED. */
    public SqliteConnection begin() {
        checkOpen();
        if (SqliteLibrary.snr_begin(handle) != 0) {
            throw new SqliteException("begin() falló: " + lastError());
        }
        return this;
    }

    /** Inicia BEGIN IMMEDIATE (reserva write lock). */
    public SqliteConnection beginImmediate() {
        checkOpen();
        if (SqliteLibrary.snr_begin_immediate(handle) != 0) {
            throw new SqliteException("beginImmediate() falló: " + lastError());
        }
        return this;
    }

    /** Inicia BEGIN EXCLUSIVE. */
    public SqliteConnection beginExclusive() {
        checkOpen();
        if (SqliteLibrary.snr_begin_exclusive(handle) != 0) {
            throw new SqliteException("beginExclusive() falló: " + lastError());
        }
        return this;
    }

    /** Confirma la transacción activa. */
    public SqliteConnection commit() {
        checkOpen();
        if (SqliteLibrary.snr_commit(handle) != 0) {
            throw new SqliteException("commit() falló: " + lastError());
        }
        return this;
    }

    /** Revierte la transacción activa. */
    public SqliteConnection rollback() {
        checkOpen();
        if (SqliteLibrary.snr_rollback(handle) != 0) {
            throw new SqliteException("rollback() falló: " + lastError());
        }
        return this;
    }

    /**
     * Ejecuta {@code work} dentro de una transacción DEFERRED.
     * Si {@code work} lanza excepción, hace rollback automático.
     *
     * @param work bloque de código a ejecutar
     */
    public void transaction(Runnable work) {
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
    public void transactionImmediate(Runnable work) {
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
    public SqliteConnection savepoint(String name) {
        checkOpen();
        if (SqliteLibrary.snr_savepoint(handle, name) != 0) {
            throw new SqliteException("savepoint('%s') falló: %s".formatted(name, lastError()));
        }
        return this;
    }

    /** Libera (confirma) el SAVEPOINT {@code name}. */
    public SqliteConnection release(String name) {
        checkOpen();
        if (SqliteLibrary.snr_release(handle, name) != 0) {
            throw new SqliteException("release('%s') falló: %s".formatted(name, lastError()));
        }
        return this;
    }

    /** Revierte hasta el SAVEPOINT {@code name} (sin eliminarlo). */
    public SqliteConnection rollbackTo(String name) {
        checkOpen();
        if (SqliteLibrary.snr_rollback_to(handle, name) != 0) {
            throw new SqliteException("rollbackTo('%s') falló: %s".formatted(name, lastError()));
        }
        return this;
    }

    /**
     * Ejecuta {@code work} dentro de un savepoint.
     * Si {@code work} lanza excepción, hace rollback al savepoint y lo libera.
     *
     * @param name nombre del savepoint
     * @param work bloque de código a ejecutar
     */
    public void withSavepoint(String name, Runnable work) {
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

    // ── Handle raw ───────────────────────────────────────────────────────────

    /**
     * Puntero raw al Handle de Rust (para uso avanzado con SqliteLibrary directamente).
     *
     * @throws SqliteException si la conexión ya fue cerrada — el handle liberado
     *         causaría use-after-free en Rust
     */
    public MemorySegment rawHandle() {
        checkOpen();
        return handle;
    }

    // ── AutoCloseable ────────────────────────────────────────────────────────

    /**
     * Cierra la conexión. Si hay statements abiertos creados por {@link #prepare},
     * emite un warning — esos statements mantienen el handle Rust vivo hasta que
     * se cierren, pero esta instancia Java ya no es utilizable.
     */
    @Override
    public void close() {
        if (!closed) {
            closed = true;
            int open = openStatements.get();
            if (open > 0) {
                LOG.warning("[esr] conexión cerrada con " + open
                    + " statement(s) aún abiertos — posible resource leak. "
                    + "Cierra siempre los SqliteStatement antes de SqliteConnection.");
            }
            SqliteLibrary.snr_close(handle);
        }
    }

    // ── Utilidades estáticas ──────────────────────────────────────────────────

    /** Versión de SQLite compilada en la librería nativa. */
    public static String sqliteVersion() {
        return SqliteStatement.readAndFreeString(SqliteLibrary.snr_sqlite_version());
    }

    /**
     * Último error del hilo actual reportado por Rust.
     * Usa {@code snr_last_error_copy()} para ser seguro con Project Loom (R-1).
     */
    public static String lastError() {
        return SqliteStatement.readAndFreeString(SqliteLibrary.snr_last_error_copy());
    }

    // ── Helpers privados ──────────────────────────────────────────────────────

    private void checkOpen() {
        if (closed) throw new SqliteException("Conexión ya cerrada");
    }

    private static boolean isNull(MemorySegment seg) {
        return seg == null || MemorySegment.NULL.equals(seg);
    }
}
