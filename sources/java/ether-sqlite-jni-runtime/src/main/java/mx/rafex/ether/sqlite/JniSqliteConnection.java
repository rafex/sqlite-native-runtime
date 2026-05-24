package mx.rafex.ether.sqlite;

import mx.rafex.ether.sqlite.jni.SqliteLibraryJni;

import java.util.concurrent.atomic.AtomicInteger;
import java.util.logging.Logger;

import static mx.rafex.ether.sqlite.SqliteConnection.WalCheckpointResult;
import static mx.rafex.ether.sqlite.SqliteConnection.WalMode;

/**
 * Conexión SQLite de alto nivel via JNI ({@link SqliteLibraryJni}).
 *
 * <p>Implementa {@link AutoCloseable} — usar en try-with-resources.
 *
 * <h2>Apertura</h2>
 * <pre>{@code
 * try (var db = JniSqliteConnection.open("/data/app.db")) {
 *     db.exec("PRAGMA journal_mode=WAL");
 *     // ...
 * }
 * }</pre>
 *
 * <h2>Base de datos en memoria</h2>
 * <pre>{@code
 * try (var db = JniSqliteConnection.memory()) {
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
 * <h2>WAL</h2>
 * <pre>{@code
 * db.enableWal();
 * WalCheckpointResult r = db.walCheckpoint(WalMode.TRUNCATE, null);
 * System.out.printf("WAL frames: %d, checkpointed: %d%n", r.walFrames(), r.checkpointed());
 * }</pre>
 *
 * <h2>GraalVM Native Image</h2>
 * <p>JNI es plenamente compatible con GraalVM native-image (Java 21+).
 */
public final class JniSqliteConnection implements SqliteConnection {

    // WalMode y WalCheckpointResult están definidos en la interface SqliteConnection (ether-sqlite-core)

    private static final Logger LOG = Logger.getLogger(JniSqliteConnection.class.getName());

    private final long handle;   // puntero *mut Handle como long
    private final AtomicInteger openStatements = new AtomicInteger(0);
    private boolean closed = false;

    private JniSqliteConnection(long handle) {
        this.handle = handle;
    }

    // ── Fábrica ───────────────────────────────────────────────────────────────

    /**
     * Abre (o crea) la base de datos en {@code path}.
     * Por defecto aplica {@code OPEN_NOFOLLOW} para rechazar symlinks.
     *
     * @throws SqliteException si no se puede abrir
     */
    public static SqliteConnection open(String path) {
        long h = SqliteLibraryJni.snrOpen(path, 0);
        if (h == 0) {
            throw new SqliteException("No se pudo abrir '%s': %s".formatted(path, lastError()));
        }
        LOG.fine(() -> "SQLite JNI abierto: %s (SQLite %s)".formatted(path, sqliteVersion()));
        return new JniSqliteConnection(h);
    }

    /**
     * Abre (o crea) la base de datos en {@code path} con flags explícitos.
     * Usar las constantes {@link SqliteLibraryJni#OPEN_READONLY}, {@link SqliteLibraryJni#OPEN_READWRITE},
     * {@link SqliteLibraryJni#OPEN_CREATE}, {@link SqliteLibraryJni#OPEN_NOFOLLOW}.
     */
    public static SqliteConnection open(String path, int flags) {
        long h = SqliteLibraryJni.snrOpen(path, flags);
        if (h == 0) {
            throw new SqliteException("No se pudo abrir '%s': %s".formatted(path, lastError()));
        }
        return new JniSqliteConnection(h);
    }

    /**
     * Abre una base de datos en memoria anónima ({@code :memory:}).
     * Los datos se pierden al cerrar la conexión.
     */
    public static SqliteConnection memory() {
        long h = SqliteLibraryJni.snrOpenMemory(null);
        if (h == 0) {
            throw new SqliteException("No se pudo abrir :memory:: " + lastError());
        }
        return new JniSqliteConnection(h);
    }

    /**
     * Abre una base de datos en memoria con nombre compartido.
     * Útil para compartir una BD en memoria entre múltiples conexiones en el mismo proceso.
     */
    public static SqliteConnection memory(String name) {
        long h = SqliteLibraryJni.snrOpenMemory(name);
        if (h == 0) {
            throw new SqliteException("No se pudo abrir memoria '%s': %s".formatted(name, lastError()));
        }
        return new JniSqliteConnection(h);
    }

    // ── Operaciones básicas ───────────────────────────────────────────────────

    @Override
    public SqliteConnection exec(String sql) {
        checkOpen();
        if (SqliteLibraryJni.snrExec(handle, sql) != 0) {
            throw new SqliteException("exec() falló [%s]: %s".formatted(sql, lastError()));
        }
        return this;
    }

    @Override
    public SqliteStatement prepare(String sql) {
        checkOpen();
        long stmtHandle = SqliteLibraryJni.snrPrepare(handle, sql);
        if (stmtHandle == 0) {
            throw new SqliteException("prepare() falló [%s]: %s".formatted(sql, lastError()));
        }
        openStatements.incrementAndGet();
        return new JniSqliteStatement(stmtHandle, openStatements::decrementAndGet);
    }

    @Override
    public long lastInsertRowid() {
        checkOpen();
        return SqliteLibraryJni.snrLastInsertRowid(handle);
    }

    @Override
    public long changes() {
        checkOpen();
        return SqliteLibraryJni.snrChanges(handle);
    }

    @Override
    public SqliteConnection busyTimeout(int ms) {
        checkOpen();
        SqliteLibraryJni.snrSetBusyTimeout(handle, ms);
        return this;
    }

    @Override
    public boolean ping() {
        checkOpen();
        return SqliteLibraryJni.snrPing(handle) == 1L;
    }

    // ── WAL ──────────────────────────────────────────────────────────────────

    @Override
    public SqliteConnection enableWal() {
        exec("PRAGMA journal_mode=WAL");
        exec("PRAGMA synchronous=NORMAL");
        return this;
    }

    @Override
    public WalCheckpointResult walCheckpoint(WalMode mode, String dbName) {
        checkOpen();
        long packed = SqliteLibraryJni.snrWalCheckpoint(handle, dbName, mode.value);
        if (packed == -1L) {
            throw new SqliteException("walCheckpoint(%s) falló: %s".formatted(mode, lastError()));
        }
        int walFrames    = (int) (packed >> 32);
        int checkpointed = (int) (packed & 0xFFFFFFFFL);
        return new WalCheckpointResult(walFrames, checkpointed);
    }

    @Override
    public SqliteConnection walAutocheckpoint(int n) {
        checkOpen();
        SqliteLibraryJni.snrWalAutocheckpoint(handle, n);
        return this;
    }

    // ── Transacciones ─────────────────────────────────────────────────────────

    @Override
    public SqliteConnection begin() {
        checkOpen();
        if (SqliteLibraryJni.snrBegin(handle) != 0) {
            throw new SqliteException("begin() falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteConnection beginImmediate() {
        checkOpen();
        if (SqliteLibraryJni.snrBeginImmediate(handle) != 0) {
            throw new SqliteException("beginImmediate() falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteConnection beginExclusive() {
        checkOpen();
        if (SqliteLibraryJni.snrBeginExclusive(handle) != 0) {
            throw new SqliteException("beginExclusive() falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteConnection commit() {
        checkOpen();
        if (SqliteLibraryJni.snrCommit(handle) != 0) {
            throw new SqliteException("commit() falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteConnection rollback() {
        checkOpen();
        if (SqliteLibraryJni.snrRollback(handle) != 0) {
            throw new SqliteException("rollback() falló: " + lastError());
        }
        return this;
    }

    // ── Savepoints ───────────────────────────────────────────────────────────

    @Override
    public SqliteConnection savepoint(String name) {
        checkOpen();
        if (SqliteLibraryJni.snrSavepoint(handle, name) != 0) {
            throw new SqliteException("savepoint('%s') falló: %s".formatted(name, lastError()));
        }
        return this;
    }

    @Override
    public SqliteConnection release(String name) {
        checkOpen();
        if (SqliteLibraryJni.snrRelease(handle, name) != 0) {
            throw new SqliteException("release('%s') falló: %s".formatted(name, lastError()));
        }
        return this;
    }

    @Override
    public SqliteConnection rollbackTo(String name) {
        checkOpen();
        if (SqliteLibraryJni.snrRollbackTo(handle, name) != 0) {
            throw new SqliteException("rollbackTo('%s') falló: %s".formatted(name, lastError()));
        }
        return this;
    }

    // ── AutoCloseable ────────────────────────────────────────────────────────

    @Override
    public void close() {
        if (!closed) {
            closed = true;
            int open = openStatements.get();
            if (open > 0) {
                LOG.warning("[snr-jni] conexión cerrada con " + open
                    + " statement(s) aún abiertos — posible resource leak. "
                    + "Cierra siempre los SqliteStatement antes de SqliteConnection.");
            }
            SqliteLibraryJni.snrClose(handle);
        }
    }

    // ── Utilidades estáticas ──────────────────────────────────────────────────

    /** Versión de SQLite compilada en la librería nativa. */
    public static String sqliteVersion() {
        return SqliteLibraryJni.snrSqliteVersion();
    }

    /**
     * Último error del hilo actual reportado por Rust.
     */
    public static String lastError() {
        var msg = SqliteLibraryJni.snrLastError();
        return msg != null ? msg : "(sin error)";
    }

    // ── Helpers privados ──────────────────────────────────────────────────────

    private void checkOpen() {
        if (closed) throw new SqliteException("Conexión ya cerrada");
    }
}
