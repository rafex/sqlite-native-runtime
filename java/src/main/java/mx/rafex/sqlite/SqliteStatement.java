package mx.rafex.sqlite;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;

/**
 * Prepared statement SQLite. Obtenido de {@link SqliteConnection#prepare(String)}.
 *
 * <p>Implementa {@link AutoCloseable} — úsalo en un try-with-resources para
 * garantizar que el statement se finaliza aunque ocurra una excepción.
 *
 * <h2>Uso típico</h2>
 * <pre>{@code
 * try (var stmt = conn.prepare("SELECT id, name FROM users WHERE active = ?")) {
 *     stmt.bindInt(1, 1L);
 *     while (stmt.step()) {
 *         long id   = stmt.columnInt(0);
 *         String nm = stmt.columnText(1);
 *     }
 * }
 * }</pre>
 *
 * <h2>Re-uso de statements</h2>
 * <pre>{@code
 * try (var stmt = conn.prepare("INSERT INTO t(x) VALUES(?)")) {
 *     for (String val : values) {
 *         stmt.bindText(1, val);
 *         stmt.stepAndDone();
 *         stmt.reset();
 *     }
 * }
 * }</pre>
 */
public final class SqliteStatement implements AutoCloseable {

    static final int SNR_ROW   =  1;
    static final int SNR_DONE  =  0;
    static final int SNR_ERROR = -1;

    /** Tipos de columna SQLite. */
    public static final int TYPE_INTEGER = 1;
    public static final int TYPE_FLOAT   = 2;
    public static final int TYPE_TEXT    = 3;
    public static final int TYPE_BLOB    = 4;
    public static final int TYPE_NULL    = 5;

    private final MemorySegment stmt;
    private boolean closed = false;

    SqliteStatement(MemorySegment stmt) {
        this.stmt = stmt;
    }

    // ── Bind (índice 1-based) ─────────────────────────────────────────────────

    /** Enlaza NULL al parámetro {@code idx} (1-based). */
    public SqliteStatement bindNull(int idx) {
        checkOpen();
        if (SqliteLibrary.snr_bind_null(stmt, idx) != 0) {
            throw new SqliteException("bindNull(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    /** Enlaza un {@code long} al parámetro {@code idx} (1-based). */
    public SqliteStatement bindInt(int idx, long val) {
        checkOpen();
        if (SqliteLibrary.snr_bind_int(stmt, idx, val) != 0) {
            throw new SqliteException("bindInt(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    /** Enlaza un {@code int} al parámetro {@code idx} (1-based). */
    public SqliteStatement bindInt(int idx, int val) {
        return bindInt(idx, (long) val);
    }

    /** Enlaza un {@code double} al parámetro {@code idx} (1-based). */
    public SqliteStatement bindDouble(int idx, double val) {
        checkOpen();
        if (SqliteLibrary.snr_bind_double(stmt, idx, val) != 0) {
            throw new SqliteException("bindDouble(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    /**
     * Enlaza un {@code String} UTF-8 al parámetro {@code idx} (1-based).
     * Si {@code val} es {@code null}, enlaza NULL.
     */
    public SqliteStatement bindText(int idx, String val) {
        checkOpen();
        if (SqliteLibrary.snr_bind_text(stmt, idx, val) != 0) {
            throw new SqliteException("bindText(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    /**
     * Enlaza un blob al parámetro {@code idx} (1-based).
     * Si {@code data} es {@code null}, enlaza NULL.
     */
    public SqliteStatement bindBlob(int idx, byte[] data) {
        checkOpen();
        if (SqliteLibrary.snr_bind_blob(stmt, idx, data) != 0) {
            throw new SqliteException("bindBlob(" + idx + ") falló: " + lastError());
        }
        return this;
    }

    /**
     * Devuelve el índice (1-based) del parámetro con nombre {@code name}
     * (p.ej. {@code ":id"}, {@code "@nombre"}).
     * Devuelve 0 si no existe.
     */
    public int parameterIndex(String name) {
        checkOpen();
        return SqliteLibrary.snr_bind_parameter_index(stmt, name);
    }

    // ── Step ─────────────────────────────────────────────────────────────────

    /**
     * Avanza el statement un paso.
     *
     * @return {@code true} si hay una fila disponible, {@code false} si terminó
     * @throws SqliteException si SQLite reporta un error
     */
    public boolean step() {
        checkOpen();
        int rc = SqliteLibrary.snr_step(stmt);
        return switch (rc) {
            case SNR_ROW  -> true;
            case SNR_DONE -> false;
            default -> throw new SqliteException("step() falló: " + lastError());
        };
    }

    /**
     * Ejecuta el statement y verifica que devuelve DONE (útil para INSERT/UPDATE/DELETE).
     * Lanza excepción si hay alguna fila inesperada o error.
     */
    public void stepAndDone() {
        checkOpen();
        int rc = SqliteLibrary.snr_step(stmt);
        if (rc == SNR_ROW) {
            throw new SqliteException("stepAndDone(): el statement devolvió filas inesperadas");
        }
        if (rc == SNR_ERROR) {
            throw new SqliteException("stepAndDone() falló: " + lastError());
        }
    }

    // ── Reset / clear ─────────────────────────────────────────────────────────

    /**
     * Resetea el statement para re-ejecutarlo.
     * Los bindings se conservan — llamar {@link #clearBindings()} si quieres limpiarlos.
     */
    public SqliteStatement reset() {
        checkOpen();
        if (SqliteLibrary.snr_stmt_reset(stmt) != 0) {
            throw new SqliteException("reset() falló: " + lastError());
        }
        return this;
    }

    /** Limpia todos los parámetros (los establece a NULL). */
    public SqliteStatement clearBindings() {
        checkOpen();
        SqliteLibrary.snr_stmt_clear_bindings(stmt);
        return this;
    }

    // ── Column (índice 0-based) ───────────────────────────────────────────────

    /** Número de columnas en el resultado actual. */
    public int columnCount() {
        checkOpen();
        return SqliteLibrary.snr_column_count(stmt);
    }

    /** Tipo SQLite de la columna {@code col} (0-based): TYPE_INTEGER, FLOAT, TEXT, BLOB, NULL. */
    public int columnType(int col) {
        checkOpen();
        return SqliteLibrary.snr_column_type(stmt, col);
    }

    /** Lee la columna {@code col} (0-based) como {@code long}. */
    public long columnInt(int col) {
        checkOpen();
        return SqliteLibrary.snr_column_int(stmt, col);
    }

    /** Lee la columna {@code col} (0-based) como {@code double}. */
    public double columnDouble(int col) {
        checkOpen();
        return SqliteLibrary.snr_column_double(stmt, col);
    }

    /**
     * Lee la columna {@code col} (0-based) como {@code String} UTF-8.
     * Devuelve {@code null} si la columna es NULL.
     *
     * <p>Lee desde el puntero interno de SQLite (sin asignación extra en heap Rust).
     * El puntero es válido hasta el siguiente {@link #step()}, {@link #reset()} o {@link #close()}.
     */
    public String columnText(int col) {
        checkOpen();
        var ptr = SqliteLibrary.snr_column_text(stmt, col);
        return readInternalString(ptr);
    }

    /**
     * Lee la columna {@code col} (0-based) como {@code String} usando una copia en heap Rust.
     * Usa esto si necesitas guardar el valor más allá del siguiente step/reset.
     * Equivalente a {@link #columnText(int)} en la práctica (Java copia el string de todos modos),
     * pero útil si quieres asegurarte de no depender del ciclo de vida interno de SQLite.
     */
    public String columnTextSafe(int col) {
        checkOpen();
        var ptr = SqliteLibrary.snr_column_text_owned(stmt, col);
        return readAndFreeString(ptr);
    }

    /**
     * Lee la columna {@code col} (0-based) como {@code byte[]}.
     * Devuelve {@code null} si la columna es NULL o el blob está vacío.
     */
    public byte[] columnBlob(int col) {
        checkOpen();
        var blobPtr = SqliteLibrary.snr_column_blob(stmt, col);
        if (blobPtr == null || MemorySegment.NULL.equals(blobPtr)) return null;
        int len = SqliteLibrary.snr_column_bytes(stmt, col);
        if (len <= 0) return new byte[0];
        return blobPtr.reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
    }

    /** Tamaño en bytes del valor TEXT o BLOB de la columna {@code col} (0-based). */
    public int columnBytes(int col) {
        checkOpen();
        return SqliteLibrary.snr_column_bytes(stmt, col);
    }

    /**
     * Nombre de la columna {@code col} (0-based).
     * Devuelve {@code null} si el índice es inválido.
     */
    public String columnName(int col) {
        checkOpen();
        var ptr = SqliteLibrary.snr_column_name(stmt, col);
        return readInternalString(ptr);
    }

    // ── Puntero raw ──────────────────────────────────────────────────────────

    /** Puntero raw al StmtHandle de Rust (para uso avanzado con SqliteLibrary directamente). */
    public MemorySegment rawHandle() {
        return stmt;
    }

    // ── AutoCloseable ────────────────────────────────────────────────────────

    @Override
    public void close() {
        if (!closed) {
            closed = true;
            SqliteLibrary.snr_stmt_close(stmt);
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    private void checkOpen() {
        if (closed) throw new SqliteException("Statement ya cerrado");
    }

    static String lastError() {
        return readInternalString(SqliteLibrary.snr_last_error());
    }

    /** Lee un string desde un puntero interno (no libera). Devuelve null si es NULL. */
    static String readInternalString(MemorySegment ptr) {
        if (ptr == null || MemorySegment.NULL.equals(ptr)) return null;
        var unbound = ptr.reinterpret(Integer.MAX_VALUE);
        long len = 0;
        while (unbound.get(ValueLayout.JAVA_BYTE, len) != 0) len++;
        if (len == 0) return "";
        return new String(unbound.asSlice(0, len).toArray(ValueLayout.JAVA_BYTE), StandardCharsets.UTF_8);
    }

    /** Lee un string desde un puntero transferido (llama snr_free_string). Devuelve null si es NULL. */
    static String readAndFreeString(MemorySegment ptr) {
        if (ptr == null || MemorySegment.NULL.equals(ptr)) return null;
        try {
            return readInternalString(ptr);
        } finally {
            SqliteLibrary.snr_free_string(ptr);
        }
    }
}
