package mx.rafex.ether.sqlite;

/**
 * Prepared statement SQLite.
 *
 * <p>Implementa {@link AutoCloseable} — usar en try-with-resources.
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
public interface SqliteStatement extends AutoCloseable {

    // ── Tipos de columna SQLite ──────────────────────────────────────────────
    int TYPE_INTEGER = 1;
    int TYPE_FLOAT   = 2;
    int TYPE_TEXT    = 3;
    int TYPE_BLOB    = 4;
    int TYPE_NULL    = 5;

    // ── Bind (índice 1-based) ────────────────────────────────────────────────

    /** Enlaza NULL al parámetro {@code idx} (1-based). */
    SqliteStatement bindNull(int idx);

    /** Enlaza un {@code long} al parámetro {@code idx} (1-based). */
    SqliteStatement bindInt(int idx, long val);

    /** Enlaza un {@code int} al parámetro {@code idx} (1-based). */
    default SqliteStatement bindInt(int idx, int val) {
        return bindInt(idx, (long) val);
    }

    /** Enlaza un {@code double} al parámetro {@code idx} (1-based). */
    SqliteStatement bindDouble(int idx, double val);

    /**
     * Enlaza un {@code String} UTF-8 al parámetro {@code idx} (1-based).
     * Si {@code val} es {@code null}, enlaza NULL.
     */
    SqliteStatement bindText(int idx, String val);

    /**
     * Enlaza un blob al parámetro {@code idx} (1-based).
     * Si {@code data} es {@code null}, enlaza NULL.
     */
    SqliteStatement bindBlob(int idx, byte[] data);

    /**
     * Devuelve el índice (1-based) del parámetro con nombre {@code name}
     * (p.ej. {@code ":id"}, {@code "@nombre"}).
     * Devuelve 0 si no existe.
     */
    int parameterIndex(String name);

    // ── Step ─────────────────────────────────────────────────────────────────

    /**
     * Avanza el statement un paso.
     *
     * @return {@code true} si hay una fila disponible, {@code false} si terminó
     * @throws SqliteException si SQLite reporta un error
     */
    boolean step();

    /**
     * Ejecuta el statement y verifica que devuelve DONE (útil para INSERT/UPDATE/DELETE).
     *
     * @throws SqliteException si hay filas inesperadas o error
     */
    void stepAndDone();

    // ── Reset / clear ─────────────────────────────────────────────────────────

    /**
     * Resetea el statement para re-ejecutarlo.
     * Los bindings se conservan.
     */
    SqliteStatement reset();

    /** Limpia todos los parámetros (los establece a NULL). */
    SqliteStatement clearBindings();

    // ── Column (índice 0-based) ───────────────────────────────────────────────

    /** Número de columnas en el resultado actual. */
    int columnCount();

    /** Tipo SQLite de la columna {@code col} (0-based): TYPE_INTEGER, FLOAT, TEXT, BLOB, NULL. */
    int columnType(int col);

    /** Lee la columna {@code col} (0-based) como {@code long}. */
    long columnInt(int col);

    /** Lee la columna {@code col} (0-based) como {@code double}. */
    double columnDouble(int col);

    /**
     * Lee la columna {@code col} (0-based) como {@code String} UTF-8.
     * Devuelve {@code null} si la columna es NULL.
     */
    String columnText(int col);

    /**
     * Lee la columna {@code col} (0-based) como {@code String} usando una copia en heap.
     * Seguro más allá del siguiente step/reset/close.
     */
    String columnTextSafe(int col);

    /**
     * Lee la columna {@code col} (0-based) como {@code byte[]}.
     * Devuelve {@code null} si la columna es NULL.
     */
    byte[] columnBlob(int col);

    /** Tamaño en bytes del valor TEXT o BLOB de la columna {@code col} (0-based). */
    int columnBytes(int col);

    /**
     * Nombre de la columna {@code col} (0-based).
     * Devuelve {@code null} si el índice es inválido.
     */
    String columnName(int col);

    // ── AutoCloseable ────────────────────────────────────────────────────────

    @Override
    void close();
}
