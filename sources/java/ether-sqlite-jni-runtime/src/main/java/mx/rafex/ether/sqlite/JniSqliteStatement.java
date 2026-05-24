package mx.rafex.ether.sqlite;

import mx.rafex.ether.sqlite.jni.SqliteLibraryJni;

/**
 * Prepared statement SQLite via JNI. Obtenido de {@link JniSqliteConnection#prepare(String)}.
 *
 * <p>Implementa {@link AutoCloseable} — usar en try-with-resources.
 *
 * <h2>Threading</h2>
 * <p>No thread-safe para secuencias de llamadas. Usar desde un único hilo
 * a la vez, o sincronizar externamente.
 *
 * <h2>Uso típico</h2>
 * <pre>{@code
 * try (var stmt = conn.prepare("SELECT id, name FROM users WHERE active = ?")) {
 *     stmt.bindInt(1, 1L);
 *     while (stmt.step()) {
 *         long   id = stmt.columnInt(0);
 *         String nm = stmt.columnText(1);
 *     }
 * }
 * }</pre>
 */
public final class JniSqliteStatement implements SqliteStatement {

    static final int SNR_ROW   =  1;
    static final int SNR_DONE  =  0;
    static final int SNR_ERROR = -1;

    private final long stmt;       // puntero *mut StmtHandle como long
    private final Runnable onClose;
    private boolean closed = false;

    JniSqliteStatement(long stmt) {
        this.stmt = stmt;
        this.onClose = null;
    }

    JniSqliteStatement(long stmt, Runnable onClose) {
        this.stmt = stmt;
        this.onClose = onClose;
    }

    // ── Bind (índice 1-based) ─────────────────────────────────────────────────

    @Override
    public SqliteStatement bindNull(int idx) {
        checkOpen();
        if (SqliteLibraryJni.snrBindNull(stmt, idx) != 0) {
            throw new SqliteException("bindNull(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteStatement bindInt(int idx, long val) {
        checkOpen();
        if (SqliteLibraryJni.snrBindInt(stmt, idx, val) != 0) {
            throw new SqliteException("bindInt(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteStatement bindDouble(int idx, double val) {
        checkOpen();
        if (SqliteLibraryJni.snrBindDouble(stmt, idx, val) != 0) {
            throw new SqliteException("bindDouble(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteStatement bindText(int idx, String val) {
        checkOpen();
        if (SqliteLibraryJni.snrBindText(stmt, idx, val) != 0) {
            throw new SqliteException("bindText(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteStatement bindBlob(int idx, byte[] data) {
        checkOpen();
        if (SqliteLibraryJni.snrBindBlob(stmt, idx, data) != 0) {
            throw new SqliteException("bindBlob(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    @Override
    public int parameterIndex(String name) {
        checkOpen();
        if (name == null) throw new SqliteException("parameterIndex: name no puede ser null");
        return SqliteLibraryJni.snrBindParameterIndex(stmt, name);
    }

    // ── Step ─────────────────────────────────────────────────────────────────

    @Override
    public boolean step() {
        checkOpen();
        int rc = SqliteLibraryJni.snrStep(stmt);
        return switch (rc) {
            case SNR_ROW  -> true;
            case SNR_DONE -> false;
            default -> throw new SqliteException("step() falló: " + lastError());
        };
    }

    @Override
    public void stepAndDone() {
        checkOpen();
        int rc = SqliteLibraryJni.snrStep(stmt);
        if (rc == SNR_ROW) {
            throw new SqliteException("stepAndDone(): el statement devolvió filas inesperadas");
        }
        if (rc == SNR_ERROR) {
            throw new SqliteException("stepAndDone() falló: " + lastError());
        }
    }

    // ── Reset / clear ─────────────────────────────────────────────────────────

    @Override
    public SqliteStatement reset() {
        checkOpen();
        if (SqliteLibraryJni.snrStmtReset(stmt) != 0) {
            throw new SqliteException("reset() falló: " + lastError());
        }
        return this;
    }

    @Override
    public SqliteStatement clearBindings() {
        checkOpen();
        SqliteLibraryJni.snrStmtClearBindings(stmt);
        return this;
    }

    // ── Column (índice 0-based) ───────────────────────────────────────────────

    @Override
    public int columnCount() {
        checkOpen();
        return SqliteLibraryJni.snrColumnCount(stmt);
    }

    @Override
    public int columnType(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnType(stmt, col);
    }

    @Override
    public long columnInt(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnInt(stmt, col);
    }

    @Override
    public double columnDouble(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnDouble(stmt, col);
    }

    /**
     * Lee la columna {@code col} (0-based) como {@code String}.
     * Devuelve {@code null} si la columna es NULL.
     *
     * <p>Con JNI el JNI layer copia el valor a un Java String inmediatamente —
     * el resultado es safe incluso después del siguiente step/reset.
     */
    @Override
    public String columnText(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnText(stmt, col);
    }

    /**
     * Equivale a {@link #columnText(int)} en la implementación JNI —
     * ambos devuelven una copia Java independiente del ciclo de vida del statement.
     */
    @Override
    public String columnTextSafe(int col) {
        return columnText(col);
    }

    @Override
    public byte[] columnBlob(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnBlob(stmt, col);
    }

    @Override
    public int columnBytes(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnBytes(stmt, col);
    }

    @Override
    public String columnName(int col) {
        checkOpen();
        return SqliteLibraryJni.snrColumnName(stmt, col);
    }

    // ── AutoCloseable ────────────────────────────────────────────────────────

    @Override
    public void close() {
        if (!closed) {
            closed = true;
            SqliteLibraryJni.snrStmtClose(stmt);
            if (onClose != null) onClose.run();
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    private void checkOpen() {
        if (closed) throw new SqliteException("Statement ya cerrado");
    }

    static String lastError() {
        var msg = SqliteLibraryJni.snrLastError();
        return msg != null ? msg : "(sin error)";
    }
}
