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
 * <h2>Modelo de threading</h2>
 *
 * <p>Este objeto <strong>no es thread-safe para secuencias de llamadas</strong>.
 * Cada llamada individual está serializada internamente por el Mutex del handle Rust
 * y por {@code SQLITE_OPEN_FULLMUTEX}. Sin embargo, el lock se libera entre llamadas,
 * por lo que secuencias como {@code columnText()} seguido de {@code columnBytes()} no
 * son atómicas: un {@code step()} concurrente podría interponerse e invalidar el
 * puntero interno devuelto por {@code columnText()}.
 *
 * <p><strong>Regla:</strong> usa una instancia de {@code SqliteStatement} desde un
 * único hilo a la vez, o sincroniza externamente las secuencias de llamadas.
 * Con Project Loom, asegúrate de que ningún virtual thread distinto opere sobre
 * el mismo statement de forma concurrente.
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
    private final Runnable onClose;
    private boolean closed = false;

    SqliteStatement(MemorySegment stmt) {
        this.stmt = stmt;
        this.onClose = null;
    }

    SqliteStatement(MemorySegment stmt, Runnable onClose) {
        this.stmt = stmt;
        this.onClose = onClose;
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
     * Devuelve 0 si no existe. Lanza excepción si {@code name} es null.
     */
    public int parameterIndex(String name) {
        checkOpen();
        if (name == null) throw new SqliteException("parameterIndex: name no puede ser null");
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
     * <p><strong>Advertencia de threading:</strong> el puntero interno devuelto por
     * SQLite es válido solo hasta el siguiente {@link #step()}, {@link #reset()} o
     * {@link #close()}. No usar este método en secuencias concurrentes sobre el mismo
     * statement — ver la sección "Modelo de threading" en el Javadoc de la clase.
     *
     * <p>El segmento se acota al tamaño exacto reportado por {@code snr_column_bytes}
     * para que Panama FFI detecte accesos fuera de bounds.
     */
    public String columnText(int col) {
        checkOpen();
        var ptr = SqliteLibrary.snr_column_text(stmt, col);
        if (ptr == null || MemorySegment.NULL.equals(ptr)) return null;
        int byteLen = SqliteLibrary.snr_column_bytes(stmt, col);
        if (byteLen <= 0) return "";
        return ptr.reinterpret(byteLen + 1L).getString(0, StandardCharsets.UTF_8);
    }

    /**
     * Lee la columna {@code col} (0-based) como {@code String} usando una copia en heap Rust.
     * El valor es independiente del ciclo de vida del statement — seguro más allá del siguiente step.
     * Preferir {@link #columnText(int)} para columnas que se consumen inmediatamente.
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

    /**
     * Puntero raw al StmtHandle de Rust (para uso avanzado con SqliteLibrary directamente).
     *
     * @throws SqliteException si el statement ya fue cerrado — el handle liberado
     *         causaría use-after-free en Rust
     */
    public MemorySegment rawHandle() {
        checkOpen();
        return stmt;
    }

    // ── AutoCloseable ────────────────────────────────────────────────────────

    @Override
    public void close() {
        if (!closed) {
            closed = true;
            SqliteLibrary.snr_stmt_close(stmt);
            if (onClose != null) onClose.run();
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    private void checkOpen() {
        if (closed) throw new SqliteException("Statement ya cerrado");
    }

    /**
     * Lee el último error del hilo como String.
     * Usa snr_last_error_copy para ser seguro con Project Loom (M-1):
     * la copia se toma atómicamente y se libera tras la lectura.
     */
    static String lastError() {
        return readAndFreeString(SqliteLibrary.snr_last_error_copy());
    }

    /**
     * Lee un string null-terminado desde un puntero interno de SQLite o Rust.
     * No libera el puntero — solo para punteros cuyo ciclo de vida gestiona SQLite
     * o el thread-local de Rust.
     * Devuelve null si el puntero es NULL.
     *
     * <p>Panama FFI devuelve segmentos con byteSize=0 para punteros C; se requiere
     * reinterpret para que getString pueda escanear hasta el null-terminador.
     * Para columnas TEXT usar {@link #columnText(int)} que usa el bound exacto.
     */
    static String readInternalString(MemorySegment ptr) {
        if (ptr == null || MemorySegment.NULL.equals(ptr)) return null;
        return ptr.reinterpret(Long.MAX_VALUE).getString(0, StandardCharsets.UTF_8);
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
