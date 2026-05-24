package mx.rafex.ether.sqlite;

import java.lang.foreign.AddressLayout;
import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Bindings de bajo nivel (Panama FFI) para {@code libether_sqlite_runtime}.
 *
 * <p>Cada método estático corresponde 1:1 a un símbolo {@code snr_*} exportado
 * por el crate Rust. La carga de la librería ocurre una sola vez al inicializar
 * la clase.
 *
 * <h2>Requisitos</h2>
 * <p>Java 22+ (bytecode target 22). Panama FFM API ({@code java.lang.foreign.*}) es
 * <em>estable</em> desde Java 22 (JEP 454). Compilado con GraalVM JDK 25 — no se requiere
 * {@code --enable-preview}.</p>
 *
 * <h2>GraalVM Native Image</h2>
 * <pre>
 *   --initialize-at-run-time=mx.rafex.ether.sqlite.SqliteLibrary
 *   --enable-native-access=ALL-UNNAMED
 * </pre>
 *
 * <h2>Gestión de memoria</h2>
 * <ul>
 *   <li>Funciones que devuelven {@code MemorySegment} (char*) transfieren propiedad —
 *       llamar {@link #snr_free_string} cuando termines.</li>
 *   <li>{@link #snr_last_error()} devuelve puntero interno — NO liberar.</li>
 *   <li>{@link #snr_last_error_copy()} devuelve copia — SÍ liberar con {@link #snr_free_string}.</li>
 *   <li>{@link #snr_column_text(MemorySegment, int)} devuelve puntero interno
 *       válido hasta el siguiente step/reset/close — leer inmediatamente.</li>
 *   <li>{@link #snr_column_text_owned(MemorySegment, int)} devuelve copia — SÍ liberar.</li>
 * </ul>
 *
 * <p>Prefiere usar {@link SqliteConnection} y {@link SqliteStatement} en lugar
 * de esta clase directamente.
 */
public final class SqliteLibrary {

    // ── Descriptores de tipos C ───────────────────────────────────────────────
    static final ValueLayout.OfLong   C_LONG   = ValueLayout.JAVA_LONG;
    static final ValueLayout.OfInt    C_INT    = ValueLayout.JAVA_INT;
    static final ValueLayout.OfDouble C_DOUBLE = ValueLayout.JAVA_DOUBLE;
    static final AddressLayout        C_PTR    = ValueLayout.ADDRESS;

    // ── Constantes de apertura (R-6) ─────────────────────────────────────────
    // Valores definidos en la especificación SQLite — estables desde SQLite 3.x.
    // Usar estas constantes en lugar de llamar snr_flag_*() para evitar el overhead
    // de un downcall FFI por una constante conocida en tiempo de compilación.

    /** Abrir en modo solo-lectura. */
    public static final int OPEN_READONLY  = 0x00000001;
    /** Abrir en modo lectura-escritura (sin crear si no existe). */
    public static final int OPEN_READWRITE = 0x00000002;
    /** Crear la base de datos si no existe (combinar con {@link #OPEN_READWRITE}). */
    public static final int OPEN_CREATE    = 0x00000004;
    /** Rechazar symlinks al abrir (seguridad). */
    public static final int OPEN_NOFOLLOW  = 0x01000000;

    private static final SymbolLookup LIB;
    private static final Linker LINKER = Linker.nativeLinker();

    static {
        LIB = loadLibrary();
    }

    // ── MethodHandles ─────────────────────────────────────────────────────────

    // Error / memoria
    private static final MethodHandle MH_LAST_ERROR      = find("snr_last_error",      FunctionDescriptor.of(C_PTR));
    private static final MethodHandle MH_LAST_ERROR_COPY = find("snr_last_error_copy", FunctionDescriptor.of(C_PTR));
    private static final MethodHandle MH_FREE_STRING     = find("snr_free_string",     FunctionDescriptor.ofVoid(C_PTR));

    // Conexión
    private static final MethodHandle MH_OPEN         = find("snr_open",         FunctionDescriptor.of(C_PTR, C_PTR, C_INT));
    private static final MethodHandle MH_OPEN_MEMORY  = find("snr_open_memory",  FunctionDescriptor.of(C_PTR, C_PTR));
    private static final MethodHandle MH_CLOSE        = find("snr_close",        FunctionDescriptor.ofVoid(C_PTR));
    private static final MethodHandle MH_PING         = find("snr_ping",         FunctionDescriptor.of(C_LONG, C_PTR));
    private static final MethodHandle MH_VERSION      = find("snr_sqlite_version", FunctionDescriptor.of(C_PTR));
    private static final MethodHandle MH_EXEC         = find("snr_exec",         FunctionDescriptor.of(C_INT, C_PTR, C_PTR));
    private static final MethodHandle MH_LAST_ROWID   = find("snr_last_insert_rowid", FunctionDescriptor.of(C_LONG, C_PTR));
    private static final MethodHandle MH_CHANGES      = find("snr_changes",      FunctionDescriptor.of(C_LONG, C_PTR));
    private static final MethodHandle MH_BUSY_TIMEOUT = find("snr_set_busy_timeout", FunctionDescriptor.of(C_INT, C_PTR, C_INT));

    // Prepared statements
    private static final MethodHandle MH_PREPARE           = find("snr_prepare",           FunctionDescriptor.of(C_PTR, C_PTR, C_PTR));
    private static final MethodHandle MH_STMT_CLOSE        = find("snr_stmt_close",        FunctionDescriptor.ofVoid(C_PTR));
    private static final MethodHandle MH_STMT_RESET        = find("snr_stmt_reset",        FunctionDescriptor.of(C_INT, C_PTR));
    private static final MethodHandle MH_STMT_CLEAR        = find("snr_stmt_clear_bindings", FunctionDescriptor.of(C_INT, C_PTR));

    // Bind
    private static final MethodHandle MH_BIND_NULL         = find("snr_bind_null",         FunctionDescriptor.of(C_INT, C_PTR, C_INT));
    private static final MethodHandle MH_BIND_INT          = find("snr_bind_int",          FunctionDescriptor.of(C_INT, C_PTR, C_INT, C_LONG));
    private static final MethodHandle MH_BIND_DOUBLE       = find("snr_bind_double",       FunctionDescriptor.of(C_INT, C_PTR, C_INT, C_DOUBLE));
    private static final MethodHandle MH_BIND_TEXT         = find("snr_bind_text",         FunctionDescriptor.of(C_INT, C_PTR, C_INT, C_PTR));
    private static final MethodHandle MH_BIND_BLOB         = find("snr_bind_blob",         FunctionDescriptor.of(C_INT, C_PTR, C_INT, C_PTR, C_INT));
    private static final MethodHandle MH_BIND_PARAM_INDEX  = find("snr_bind_parameter_index", FunctionDescriptor.of(C_INT, C_PTR, C_PTR));

    // Step
    private static final MethodHandle MH_STEP = find("snr_step", FunctionDescriptor.of(C_INT, C_PTR));

    // Column
    private static final MethodHandle MH_COL_COUNT       = find("snr_column_count",      FunctionDescriptor.of(C_INT, C_PTR));
    private static final MethodHandle MH_COL_TYPE        = find("snr_column_type",       FunctionDescriptor.of(C_INT, C_PTR, C_INT));
    private static final MethodHandle MH_COL_INT         = find("snr_column_int",        FunctionDescriptor.of(C_LONG, C_PTR, C_INT));
    private static final MethodHandle MH_COL_DOUBLE      = find("snr_column_double",     FunctionDescriptor.of(C_DOUBLE, C_PTR, C_INT));
    private static final MethodHandle MH_COL_TEXT        = find("snr_column_text",       FunctionDescriptor.of(C_PTR, C_PTR, C_INT));
    private static final MethodHandle MH_COL_TEXT_OWNED  = find("snr_column_text_owned", FunctionDescriptor.of(C_PTR, C_PTR, C_INT));
    private static final MethodHandle MH_COL_BLOB        = find("snr_column_blob",       FunctionDescriptor.of(C_PTR, C_PTR, C_INT));
    private static final MethodHandle MH_COL_BYTES       = find("snr_column_bytes",      FunctionDescriptor.of(C_INT, C_PTR, C_INT));
    private static final MethodHandle MH_COL_NAME        = find("snr_column_name",       FunctionDescriptor.of(C_PTR, C_PTR, C_INT));

    // Transacciones
    private static final MethodHandle MH_BEGIN           = find("snr_begin",           FunctionDescriptor.of(C_INT, C_PTR));
    private static final MethodHandle MH_BEGIN_IMMEDIATE = find("snr_begin_immediate", FunctionDescriptor.of(C_INT, C_PTR));
    private static final MethodHandle MH_BEGIN_EXCLUSIVE = find("snr_begin_exclusive", FunctionDescriptor.of(C_INT, C_PTR));
    private static final MethodHandle MH_COMMIT          = find("snr_commit",          FunctionDescriptor.of(C_INT, C_PTR));
    private static final MethodHandle MH_ROLLBACK        = find("snr_rollback",        FunctionDescriptor.of(C_INT, C_PTR));

    // Savepoints
    private static final MethodHandle MH_SAVEPOINT    = find("snr_savepoint",    FunctionDescriptor.of(C_INT, C_PTR, C_PTR));
    private static final MethodHandle MH_RELEASE      = find("snr_release",      FunctionDescriptor.of(C_INT, C_PTR, C_PTR));
    private static final MethodHandle MH_ROLLBACK_TO  = find("snr_rollback_to",  FunctionDescriptor.of(C_INT, C_PTR, C_PTR));

    // WAL — snr_wal_checkpoint recibe dos punteros de salida opcionales (R-4)
    private static final MethodHandle MH_WAL_CHECKPOINT    = find("snr_wal_checkpoint",
            FunctionDescriptor.of(C_INT, C_PTR, C_PTR, C_INT, C_PTR, C_PTR));
    private static final MethodHandle MH_WAL_AUTOCHECKPOINT = find("snr_wal_autocheckpoint",
            FunctionDescriptor.of(C_INT, C_PTR, C_INT));

    // ── API pública ───────────────────────────────────────────────────────────

    /**
     * Puntero interno del hilo al último error. <strong>NO liberar.</strong>
     *
     * <p><strong>Advertencia con Project Loom (virtual threads):</strong>
     * el puntero es válido solo hasta la siguiente llamada {@code snr_*} en el mismo
     * carrier thread OS. Si dos virtual threads comparten carrier, el error puede
     * sobreescribirse antes de ser leído. Usar {@link #snr_last_error_copy()} en
     * entornos con virtual threads.
     */
    public static MemorySegment snr_last_error() {
        try { return (MemorySegment) MH_LAST_ERROR.invokeExact(); }
        catch (Throwable t) { throw new SqliteException("snr_last_error falló", t); }
    }

    /**
     * Devuelve una <em>copia</em> en heap del último error del hilo.
     * Java <strong>DEBE</strong> liberar el resultado con {@link #snr_free_string} cuando termine.
     * Devuelve {@link MemorySegment#NULL} si no hay error.
     *
     * <p>Seguro con Project Loom: la copia se toma en el instante de la llamada,
     * evitando carreras con otras virtual threads en el mismo carrier OS.
     */
    public static MemorySegment snr_last_error_copy() {
        try { return (MemorySegment) MH_LAST_ERROR_COPY.invokeExact(); }
        catch (Throwable t) { throw new SqliteException("snr_last_error_copy falló", t); }
    }

    /** Libera un *mut c_char transferido por Rust. Llamar exactamente una vez. */
    public static void snr_free_string(MemorySegment ptr) {
        if (ptr == null || MemorySegment.NULL.equals(ptr)) return;
        try { MH_FREE_STRING.invokeExact(ptr); }
        catch (Throwable t) { throw new SqliteException("snr_free_string falló", t); }
    }

    /**
     * Abre la base de datos en {@code path}.
     *
     * @param path  ruta absoluta al archivo .db
     * @param flags combinación de {@link #OPEN_READONLY}, {@link #OPEN_READWRITE},
     *              {@link #OPEN_CREATE}, {@link #OPEN_NOFOLLOW},
     *              o 0 para read-write + create + nofollow (por defecto)
     * @return handle opaco o NULL en error
     */
    public static MemorySegment snr_open(String path, int flags) {
        try (var arena = Arena.ofConfined()) {
            return (MemorySegment) MH_OPEN.invokeExact(arena.allocateFrom(path), flags);
        } catch (Throwable t) { throw new SqliteException("snr_open falló", t); }
    }

    /**
     * Abre una base de datos en memoria.
     *
     * @param name nombre de la BD en memoria, o {@code null} para {@code :memory:} anónima
     * @return handle opaco o NULL en error
     */
    public static MemorySegment snr_open_memory(String name) {
        try (var arena = Arena.ofConfined()) {
            var ptr = name != null ? arena.allocateFrom(name) : MemorySegment.NULL;
            return (MemorySegment) MH_OPEN_MEMORY.invokeExact(ptr);
        } catch (Throwable t) { throw new SqliteException("snr_open_memory falló", t); }
    }

    /** Cierra la conexión y libera el handle. */
    public static void snr_close(MemorySegment handle) {
        try { MH_CLOSE.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_close falló", t); }
    }

    /** Verifica que la conexión responde. Devuelve 1 si OK, 0 en error. */
    public static long snr_ping(MemorySegment handle) {
        try { return (long) MH_PING.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_ping falló", t); }
    }

    /** Versión de SQLite. Java debe liberar con {@link #snr_free_string}. */
    public static MemorySegment snr_sqlite_version() {
        try { return (MemorySegment) MH_VERSION.invokeExact(); }
        catch (Throwable t) { throw new SqliteException("snr_sqlite_version falló", t); }
    }

    /** Ejecuta SQL sin resultado. Devuelve 0 en éxito, -1 en error. */
    public static int snr_exec(MemorySegment handle, String sql) {
        try (var arena = Arena.ofConfined()) {
            return (int) MH_EXEC.invokeExact(handle, arena.allocateFrom(sql));
        } catch (Throwable t) { throw new SqliteException("snr_exec falló", t); }
    }

    /** Rowid de la última inserción exitosa. */
    public static long snr_last_insert_rowid(MemorySegment handle) {
        try { return (long) MH_LAST_ROWID.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_last_insert_rowid falló", t); }
    }

    /** Filas modificadas por la última operación DML. */
    public static long snr_changes(MemorySegment handle) {
        try { return (long) MH_CHANGES.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_changes falló", t); }
    }

    /** Configura el busy timeout en milisegundos. Devuelve 0 en éxito. */
    public static int snr_set_busy_timeout(MemorySegment handle, int ms) {
        try { return (int) MH_BUSY_TIMEOUT.invokeExact(handle, ms); }
        catch (Throwable t) { throw new SqliteException("snr_set_busy_timeout falló", t); }
    }

    /** Compila SQL en un prepared statement. Cerrar con {@link #snr_stmt_close}. */
    public static MemorySegment snr_prepare(MemorySegment handle, String sql) {
        try (var arena = Arena.ofConfined()) {
            return (MemorySegment) MH_PREPARE.invokeExact(handle, arena.allocateFrom(sql));
        } catch (Throwable t) { throw new SqliteException("snr_prepare falló", t); }
    }

    /** Finaliza y libera el statement. */
    public static void snr_stmt_close(MemorySegment stmt) {
        try { MH_STMT_CLOSE.invokeExact(stmt); }
        catch (Throwable t) { throw new SqliteException("snr_stmt_close falló", t); }
    }

    /** Resetea el statement (mantiene bindings). Devuelve 0 en éxito. */
    public static int snr_stmt_reset(MemorySegment stmt) {
        try { return (int) MH_STMT_RESET.invokeExact(stmt); }
        catch (Throwable t) { throw new SqliteException("snr_stmt_reset falló", t); }
    }

    /** Limpia todos los parámetros (los pone a NULL). Devuelve 0 en éxito. */
    public static int snr_stmt_clear_bindings(MemorySegment stmt) {
        try { return (int) MH_STMT_CLEAR.invokeExact(stmt); }
        catch (Throwable t) { throw new SqliteException("snr_stmt_clear_bindings falló", t); }
    }

    /** Enlaza NULL al parámetro {@code idx} (1-based). */
    public static int snr_bind_null(MemorySegment stmt, int idx) {
        try { return (int) MH_BIND_NULL.invokeExact(stmt, idx); }
        catch (Throwable t) { throw new SqliteException("snr_bind_null falló", t); }
    }

    /** Enlaza un long al parámetro {@code idx} (1-based). */
    public static int snr_bind_int(MemorySegment stmt, int idx, long val) {
        try { return (int) MH_BIND_INT.invokeExact(stmt, idx, val); }
        catch (Throwable t) { throw new SqliteException("snr_bind_int falló", t); }
    }

    /** Enlaza un double al parámetro {@code idx} (1-based). */
    public static int snr_bind_double(MemorySegment stmt, int idx, double val) {
        try { return (int) MH_BIND_DOUBLE.invokeExact(stmt, idx, val); }
        catch (Throwable t) { throw new SqliteException("snr_bind_double falló", t); }
    }

    /** Enlaza un String UTF-8 al parámetro {@code idx} (1-based). Si {@code val} es null, enlaza NULL. */
    public static int snr_bind_text(MemorySegment stmt, int idx, String val) {
        try (var arena = Arena.ofConfined()) {
            var ptr = val != null ? arena.allocateFrom(val) : MemorySegment.NULL;
            return (int) MH_BIND_TEXT.invokeExact(stmt, idx, ptr);
        } catch (Throwable t) { throw new SqliteException("snr_bind_text falló", t); }
    }

    /**
     * Enlaza un blob al parámetro {@code idx} (1-based).
     *
     * @param data puntero a los bytes (puede ser NULL para enlazar NULL)
     * @param len  número de bytes
     */
    public static int snr_bind_blob(MemorySegment stmt, int idx, MemorySegment data, int len) {
        try {
            var ptr = data != null ? data : MemorySegment.NULL;
            return (int) MH_BIND_BLOB.invokeExact(stmt, idx, ptr, len);
        } catch (Throwable t) { throw new SqliteException("snr_bind_blob falló", t); }
    }

    /**
     * Enlaza un blob desde un array de bytes Java al parámetro {@code idx} (1-based).
     * El método copia los bytes al heap nativo usando una Arena confined temporal.
     */
    public static int snr_bind_blob(MemorySegment stmt, int idx, byte[] data) {
        if (data == null) return snr_bind_null(stmt, idx);
        try (var arena = Arena.ofConfined()) {
            var seg = arena.allocateFrom(ValueLayout.JAVA_BYTE, data);
            return snr_bind_blob(stmt, idx, seg, data.length);
        }
    }

    /**
     * Devuelve el índice (1-based) del parámetro con nombre {@code name}.
     * Devuelve 0 si no existe.
     */
    public static int snr_bind_parameter_index(MemorySegment stmt, String name) {
        try (var arena = Arena.ofConfined()) {
            return (int) MH_BIND_PARAM_INDEX.invokeExact(stmt, arena.allocateFrom(name));
        } catch (Throwable t) { throw new SqliteException("snr_bind_parameter_index falló", t); }
    }

    /**
     * Avanza el statement un paso.
     *
     * @return 1 si hay fila (SNR_ROW), 0 si terminó (SNR_DONE), -1 en error
     */
    public static int snr_step(MemorySegment stmt) {
        try { return (int) MH_STEP.invokeExact(stmt); }
        catch (Throwable t) { throw new SqliteException("snr_step falló", t); }
    }

    /** Número de columnas en el resultado actual. */
    public static int snr_column_count(MemorySegment stmt) {
        try { return (int) MH_COL_COUNT.invokeExact(stmt); }
        catch (Throwable t) { throw new SqliteException("snr_column_count falló", t); }
    }

    /** Tipo de la columna {@code col} (0-based): 1=INTEGER, 2=FLOAT, 3=TEXT, 4=BLOB, 5=NULL. */
    public static int snr_column_type(MemorySegment stmt, int col) {
        try { return (int) MH_COL_TYPE.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_type falló", t); }
    }

    /** Lee la columna {@code col} (0-based) como {@code long}. */
    public static long snr_column_int(MemorySegment stmt, int col) {
        try { return (long) MH_COL_INT.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_int falló", t); }
    }

    /** Lee la columna {@code col} (0-based) como {@code double}. */
    public static double snr_column_double(MemorySegment stmt, int col) {
        try { return (double) MH_COL_DOUBLE.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_double falló", t); }
    }

    /**
     * Lee la columna {@code col} como texto.
     * Devuelve puntero INTERNO de SQLite — válido SOLO hasta el siguiente
     * {@link #snr_step}, {@link #snr_stmt_reset} o {@link #snr_stmt_close}.
     * NO llamar {@link #snr_free_string} sobre este puntero.
     * Usar {@link SqliteStatement#columnText(int)} para obtener un String Java directamente.
     */
    public static MemorySegment snr_column_text(MemorySegment stmt, int col) {
        try { return (MemorySegment) MH_COL_TEXT.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_text falló", t); }
    }

    /**
     * Lee la columna {@code col} como texto y devuelve una copia en heap.
     * Java DEBE liberar el resultado con {@link #snr_free_string}.
     */
    public static MemorySegment snr_column_text_owned(MemorySegment stmt, int col) {
        try { return (MemorySegment) MH_COL_TEXT_OWNED.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_text_owned falló", t); }
    }

    /**
     * Puntero INTERNO al blob de la columna {@code col} (0-based).
     * Válido hasta el siguiente step/reset/close.
     */
    public static MemorySegment snr_column_blob(MemorySegment stmt, int col) {
        try { return (MemorySegment) MH_COL_BLOB.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_blob falló", t); }
    }

    /** Bytes del valor TEXT o BLOB de la columna {@code col} (0-based). */
    public static int snr_column_bytes(MemorySegment stmt, int col) {
        try { return (int) MH_COL_BYTES.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_bytes falló", t); }
    }

    /**
     * Nombre de la columna {@code col} (0-based).
     * Devuelve puntero interno — NO liberar, válido mientras el statement esté abierto.
     */
    public static MemorySegment snr_column_name(MemorySegment stmt, int col) {
        try { return (MemorySegment) MH_COL_NAME.invokeExact(stmt, col); }
        catch (Throwable t) { throw new SqliteException("snr_column_name falló", t); }
    }

    // ── Transacciones ─────────────────────────────────────────────────────────

    /** Inicia BEGIN DEFERRED. Devuelve 0 en éxito. */
    public static int snr_begin(MemorySegment handle) {
        try { return (int) MH_BEGIN.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_begin falló", t); }
    }

    /** Inicia BEGIN IMMEDIATE. Devuelve 0 en éxito. */
    public static int snr_begin_immediate(MemorySegment handle) {
        try { return (int) MH_BEGIN_IMMEDIATE.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_begin_immediate falló", t); }
    }

    /** Inicia BEGIN EXCLUSIVE. Devuelve 0 en éxito. */
    public static int snr_begin_exclusive(MemorySegment handle) {
        try { return (int) MH_BEGIN_EXCLUSIVE.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_begin_exclusive falló", t); }
    }

    /** Confirma la transacción. Devuelve 0 en éxito. */
    public static int snr_commit(MemorySegment handle) {
        try { return (int) MH_COMMIT.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_commit falló", t); }
    }

    /** Revierte la transacción. Devuelve 0 en éxito. */
    public static int snr_rollback(MemorySegment handle) {
        try { return (int) MH_ROLLBACK.invokeExact(handle); }
        catch (Throwable t) { throw new SqliteException("snr_rollback falló", t); }
    }

    /** Crea un SAVEPOINT. Devuelve 0 en éxito. */
    public static int snr_savepoint(MemorySegment handle, String name) {
        try (var arena = Arena.ofConfined()) {
            return (int) MH_SAVEPOINT.invokeExact(handle, arena.allocateFrom(name));
        } catch (Throwable t) { throw new SqliteException("snr_savepoint falló", t); }
    }

    /** Libera (confirma) el savepoint. Devuelve 0 en éxito. */
    public static int snr_release(MemorySegment handle, String name) {
        try (var arena = Arena.ofConfined()) {
            return (int) MH_RELEASE.invokeExact(handle, arena.allocateFrom(name));
        } catch (Throwable t) { throw new SqliteException("snr_release falló", t); }
    }

    /** Revierte hasta el savepoint (sin eliminarlo). Devuelve 0 en éxito. */
    public static int snr_rollback_to(MemorySegment handle, String name) {
        try (var arena = Arena.ofConfined()) {
            return (int) MH_ROLLBACK_TO.invokeExact(handle, arena.allocateFrom(name));
        } catch (Throwable t) { throw new SqliteException("snr_rollback_to falló", t); }
    }

    // ── WAL ──────────────────────────────────────────────────────────────────

    /**
     * Ejecuta un WAL checkpoint con punteros de salida para observabilidad (R-4).
     *
     * @param handle          handle de la conexión
     * @param dbName          nombre de la BD, o {@code null} para "main"
     * @param mode            modo del checkpoint (usar constantes {@code SNR_CHECKPOINT_*} o
     *                        los valores del enum {@link SqliteConnection.WalMode})
     * @param outWalFrames    segmento de 4 bytes que recibirá el total de frames en el WAL,
     *                        o {@link MemorySegment#NULL} si no se necesita
     * @param outCheckpointed segmento de 4 bytes que recibirá los frames checkpointed,
     *                        o {@link MemorySegment#NULL} si no se necesita
     * @return 0 en éxito, -1 en error
     */
    public static int snr_wal_checkpoint(MemorySegment handle, String dbName, int mode,
            MemorySegment outWalFrames, MemorySegment outCheckpointed) {
        try (var arena = Arena.ofConfined()) {
            var ptr = dbName != null ? arena.allocateFrom(dbName) : MemorySegment.NULL;
            var wf  = outWalFrames    != null ? outWalFrames    : MemorySegment.NULL;
            var ck  = outCheckpointed != null ? outCheckpointed : MemorySegment.NULL;
            return (int) MH_WAL_CHECKPOINT.invokeExact(handle, ptr, mode, wf, ck);
        } catch (Throwable t) { throw new SqliteException("snr_wal_checkpoint falló", t); }
    }

    /**
     * Configura el auto-checkpoint de WAL.
     *
     * @param n frames de WAL tras los cuales se hace checkpoint automático;
     *          0 para desactivar el auto-checkpoint
     */
    public static int snr_wal_autocheckpoint(MemorySegment handle, int n) {
        try { return (int) MH_WAL_AUTOCHECKPOINT.invokeExact(handle, n); }
        catch (Throwable t) { throw new SqliteException("snr_wal_autocheckpoint falló", t); }
    }

    // ── Carga de librería ────────────────────────────────────────────────────

    private static MethodHandle find(String symbol, FunctionDescriptor desc) {
        var addr = LIB.find(symbol)
            .orElseThrow(() -> new UnsatisfiedLinkError(
                "símbolo no encontrado en libether_sqlite_runtime: " + symbol));
        return LINKER.downcallHandle(addr, desc);
    }

    private static SymbolLookup loadLibrary() {
        var explicit = System.getProperty(
            "ether.sqlite.lib",
            System.getenv().getOrDefault("ETHER_SQLITE_LIB", "")).trim();

        List<Path> candidates = new ArrayList<>();
        if (!explicit.isBlank()) {
            candidates.add(Path.of(explicit));
        }

        // Rutas de usuario (XDG) — instalación sin sudo via el script de instalación
        var home = System.getProperty("user.home", "");
        if (!home.isBlank()) {
            candidates.add(Path.of(home, ".local", "lib", "libether_sqlite_runtime.dylib"));
            candidates.add(Path.of(home, ".local", "lib", "libether_sqlite_runtime.so"));
        }
        // macOS — rutas del sistema
        candidates.add(Path.of("/usr/local/lib/libether_sqlite_runtime.dylib"));
        candidates.add(Path.of("/opt/snr/lib/libether_sqlite_runtime.dylib"));
        candidates.add(Path.of("/opt/homebrew/lib/libether_sqlite_runtime.dylib"));
        // Linux — rutas del sistema
        candidates.add(Path.of("/usr/local/lib/libether_sqlite_runtime.so"));
        candidates.add(Path.of("/opt/snr/lib/libether_sqlite_runtime.so"));
        // Directorio de trabajo — solo para desarrollo local (I-1).
        // En producción define ether.sqlite.lib o ETHER_SQLITE_LIB con una ruta absoluta;
        // confiar en CWD permite plantar una .so/dylib maliciosa si el directorio
        // es escribible por un proceso no confiable.
        candidates.add(Path.of("libether_sqlite_runtime.dylib"));
        candidates.add(Path.of("libether_sqlite_runtime.so"));
        candidates.add(Path.of("lib/libether_sqlite_runtime.dylib"));
        candidates.add(Path.of("lib/libether_sqlite_runtime.so"));

        var tried = new StringBuilder();
        for (var candidate : candidates) {
            try {
                var abs = candidate.toAbsolutePath().normalize();
                if (!Files.exists(abs) || !Files.isRegularFile(abs)) {
                    tried.append("  ").append(abs).append(" (no existe)\n");
                    continue;
                }
                if (!candidate.isAbsolute()) {
                    System.err.println("[esr] AVISO: cargando desde CWD: " + abs
                        + " — en producción define ether.sqlite.lib o ETHER_SQLITE_LIB con ruta absoluta.");
                }
                return SymbolLookup.libraryLookup(abs, Arena.global());
            } catch (Throwable t) {
                tried.append("  ").append(candidate.toAbsolutePath().normalize())
                    .append(" (").append(t.getClass().getSimpleName())
                    .append(": ").append(t.getMessage()).append(")\n");
            }
        }

        throw new IllegalStateException(
            "No se pudo cargar libether_sqlite_runtime. " +
            "Define ether.sqlite.lib o ETHER_SQLITE_LIB con la ruta al .so/.dylib.\n" +
            "Rutas intentadas:\n" + tried
        );
    }

    private SqliteLibrary() {}
}
