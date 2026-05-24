package mx.rafex.ether.sqlite.jni;

import mx.rafex.ether.sqlite.SqliteException;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Bindings JNI de bajo nivel para {@code libether_sqlite_jni_runtime}.
 *
 * <p>Cada método {@code native} estático corresponde 1:1 a un símbolo
 * {@code Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_*} exportado
 * por el crate Rust. La carga de la librería ocurre una sola vez al
 * inicializar la clase.
 *
 * <h2>Handles opacos</h2>
 * <p>Los punteros Rust ({@code *mut Handle}, {@code *mut StmtHandle}) se
 * transportan como {@code long} en Java. El valor 0 indica null/error.
 *
 * <h2>WAL checkpoint</h2>
 * <p>{@link #snrWalCheckpoint} devuelve un {@code long} empaquetado:
 * {@code (walFrames << 32) | (checkpointed & 0xFFFFFFFFL)}, o {@code -1L}
 * en error. El caller desempaqueta con:
 * <pre>{@code
 * int walFrames    = (int)(result >> 32);
 * int checkpointed = (int)(result & 0xFFFFFFFFL);
 * }</pre>
 *
 * <h2>GraalVM Native Image</h2>
 * <p>JNI es plenamente compatible con GraalVM native-image (Java 21+).
 * No se necesita {@code --enable-preview} ni {@code --enable-native-access}.
 *
 * <p>Prefiere usar {@link mx.rafex.ether.sqlite.JniSqliteConnection} y
 * {@link mx.rafex.ether.sqlite.JniSqliteStatement} en lugar de esta clase.
 */
public final class SqliteLibraryJni {

    // ── Constantes de apertura ────────────────────────────────────────────────
    /** Abrir en modo solo-lectura. */
    public static final int OPEN_READONLY  = 0x00000001;
    /** Abrir en modo lectura-escritura (sin crear si no existe). */
    public static final int OPEN_READWRITE = 0x00000002;
    /** Crear la base de datos si no existe (combinar con {@link #OPEN_READWRITE}). */
    public static final int OPEN_CREATE    = 0x00000004;
    /** Rechazar symlinks al abrir (seguridad). */
    public static final int OPEN_NOFOLLOW  = 0x01000000;

    static {
        loadLibrary();
    }

    // ── Error ─────────────────────────────────────────────────────────────────

    /**
     * Último error del hilo actual. Devuelve {@code null} si no hay error.
     * El puntero interno lo gestiona Rust — no liberar.
     */
    public static native String snrLastError();

    // ── Conexión ──────────────────────────────────────────────────────────────

    /**
     * Abre la base de datos en {@code path}.
     *
     * @param flags combinación de {@link #OPEN_READONLY}, {@link #OPEN_READWRITE},
     *              {@link #OPEN_CREATE}, {@link #OPEN_NOFOLLOW}, o 0 para r/w+create+nofollow
     * @return handle opaco (long) o 0 en error
     */
    public static native long snrOpen(String path, int flags);

    /**
     * Abre una base de datos en memoria.
     *
     * @param name nombre de la BD en memoria, o {@code null} para {@code :memory:} anónima
     * @return handle opaco o 0 en error
     */
    public static native long snrOpenMemory(String name);

    /** Cierra la conexión y libera el handle. */
    public static native void snrClose(long handle);

    /** Verifica que la conexión responde. Devuelve 1 si OK, 0 en error. */
    public static native long snrPing(long handle);

    /** Versión de SQLite. La cadena la gestiona Java (copia). */
    public static native String snrSqliteVersion();

    /**
     * Ejecuta SQL sin resultado. Devuelve 0 en éxito, -1 en error.
     */
    public static native int snrExec(long handle, String sql);

    /** Rowid de la última inserción exitosa. */
    public static native long snrLastInsertRowid(long handle);

    /** Filas modificadas por la última operación DML. */
    public static native long snrChanges(long handle);

    /**
     * Configura el busy timeout en milisegundos.
     * Devuelve 0 en éxito, -1 en error.
     */
    public static native int snrSetBusyTimeout(long handle, int ms);

    /** Constante de flag: READONLY. */
    public static native int snrFlagReadonly();
    /** Constante de flag: READWRITE. */
    public static native int snrFlagReadwrite();
    /** Constante de flag: CREATE. */
    public static native int snrFlagCreate();

    // ── Prepared statements ───────────────────────────────────────────────────

    /**
     * Compila SQL en un prepared statement.
     *
     * @return handle opaco del statement o 0 en error
     */
    public static native long snrPrepare(long handle, String sql);

    /** Finaliza y libera el statement. */
    public static native void snrStmtClose(long stmt);

    /**
     * Resetea el statement (mantiene bindings).
     * Devuelve 0 en éxito, -1 en error.
     */
    public static native int snrStmtReset(long stmt);

    /**
     * Limpia todos los parámetros (los pone a NULL).
     * Devuelve 0 en éxito, -1 en error.
     */
    public static native int snrStmtClearBindings(long stmt);

    // ── Bind ─────────────────────────────────────────────────────────────────

    /** Enlaza NULL al parámetro {@code idx} (1-based). */
    public static native int snrBindNull(long stmt, int idx);

    /** Enlaza un {@code long} al parámetro {@code idx} (1-based). */
    public static native int snrBindInt(long stmt, int idx, long val);

    /** Enlaza un {@code double} al parámetro {@code idx} (1-based). */
    public static native int snrBindDouble(long stmt, int idx, double val);

    /**
     * Enlaza un {@code String} UTF-8 al parámetro {@code idx} (1-based).
     * Si {@code val} es {@code null}, enlaza NULL.
     */
    public static native int snrBindText(long stmt, int idx, String val);

    /**
     * Enlaza un blob al parámetro {@code idx} (1-based).
     * Si {@code data} es {@code null}, enlaza NULL.
     */
    public static native int snrBindBlob(long stmt, int idx, byte[] data);

    /**
     * Devuelve el índice (1-based) del parámetro con nombre {@code name}.
     * Devuelve 0 si no existe.
     */
    public static native int snrBindParameterIndex(long stmt, String name);

    // ── Step ─────────────────────────────────────────────────────────────────

    /**
     * Avanza el statement un paso.
     *
     * @return 1 si hay fila (ROW), 0 si terminó (DONE), -1 en error
     */
    public static native int snrStep(long stmt);

    // ── Column ───────────────────────────────────────────────────────────────

    /** Número de columnas en el resultado actual. */
    public static native int snrColumnCount(long stmt);

    /**
     * Tipo de la columna {@code col} (0-based):
     * 1=INTEGER, 2=FLOAT, 3=TEXT, 4=BLOB, 5=NULL.
     */
    public static native int snrColumnType(long stmt, int col);

    /** Lee la columna {@code col} (0-based) como {@code long}. */
    public static native long snrColumnInt(long stmt, int col);

    /** Lee la columna {@code col} (0-based) como {@code double}. */
    public static native double snrColumnDouble(long stmt, int col);

    /**
     * Lee la columna {@code col} (0-based) como {@code String}.
     * Devuelve {@code null} si la columna es NULL.
     * El JNI layer devuelve una copia Java — safe después del siguiente step.
     */
    public static native String snrColumnText(long stmt, int col);

    /**
     * Lee la columna {@code col} (0-based) como {@code byte[]}.
     * Devuelve {@code null} si la columna es NULL.
     */
    public static native byte[] snrColumnBlob(long stmt, int col);

    /** Bytes del valor TEXT o BLOB de la columna {@code col} (0-based). */
    public static native int snrColumnBytes(long stmt, int col);

    /**
     * Nombre de la columna {@code col} (0-based).
     * Devuelve {@code null} si el índice es inválido.
     */
    public static native String snrColumnName(long stmt, int col);

    // ── Transacciones ─────────────────────────────────────────────────────────

    /** Inicia BEGIN DEFERRED. Devuelve 0 en éxito. */
    public static native int snrBegin(long handle);

    /** Inicia BEGIN IMMEDIATE. Devuelve 0 en éxito. */
    public static native int snrBeginImmediate(long handle);

    /** Inicia BEGIN EXCLUSIVE. Devuelve 0 en éxito. */
    public static native int snrBeginExclusive(long handle);

    /** Confirma la transacción. Devuelve 0 en éxito. */
    public static native int snrCommit(long handle);

    /** Revierte la transacción. Devuelve 0 en éxito. */
    public static native int snrRollback(long handle);

    // ── Savepoints ────────────────────────────────────────────────────────────

    /** Crea un SAVEPOINT. Devuelve 0 en éxito. */
    public static native int snrSavepoint(long handle, String name);

    /** Libera (confirma) el savepoint. Devuelve 0 en éxito. */
    public static native int snrRelease(long handle, String name);

    /** Revierte hasta el savepoint. Devuelve 0 en éxito. */
    public static native int snrRollbackTo(long handle, String name);

    // ── WAL ──────────────────────────────────────────────────────────────────

    /**
     * Ejecuta un WAL checkpoint.
     *
     * @param handle handle de la conexión
     * @param dbName nombre de la BD adjunta, o {@code null} para "main"
     * @param mode   modo: 0=PASSIVE, 1=FULL, 2=RESTART, 3=TRUNCATE
     * @return long empaquetado {@code (walFrames << 32) | (checkpointed & 0xFFFFFFFFL)},
     *         o {@code -1L} en error
     */
    public static native long snrWalCheckpoint(long handle, String dbName, int mode);

    /**
     * Configura el auto-checkpoint de WAL.
     *
     * @param n frames de WAL tras los cuales se hace checkpoint; 0 para desactivar
     */
    public static native int snrWalAutocheckpoint(long handle, int n);

    /** Constante de modo checkpoint: PASSIVE. */
    public static native int snrCheckpointPassive();

    /** Constante de modo checkpoint: FULL. */
    public static native int snrCheckpointFull();

    /** Constante de modo checkpoint: RESTART. */
    public static native int snrCheckpointRestart();

    /** Constante de modo checkpoint: TRUNCATE. */
    public static native int snrCheckpointTruncate();

    // ── Carga de librería ────────────────────────────────────────────────────

    private static void loadLibrary() {
        var explicit = System.getProperty(
            "ether.sqlite.jni.lib",
            System.getenv().getOrDefault("ETHER_SQLITE_JNI_LIB", "")).trim();

        List<Path> candidates = new ArrayList<>();
        if (!explicit.isBlank()) {
            candidates.add(Path.of(explicit));
        }

        var home = System.getProperty("user.home", "");
        if (!home.isBlank()) {
            candidates.add(Path.of(home, ".local", "lib", "libether_sqlite_jni_runtime.dylib"));
            candidates.add(Path.of(home, ".local", "lib", "libether_sqlite_jni_runtime.so"));
        }
        candidates.add(Path.of("/usr/local/lib/libether_sqlite_jni_runtime.dylib"));
        candidates.add(Path.of("/opt/snr/lib/libether_sqlite_jni_runtime.dylib"));
        candidates.add(Path.of("/opt/homebrew/lib/libether_sqlite_jni_runtime.dylib"));
        candidates.add(Path.of("/usr/local/lib/libether_sqlite_jni_runtime.so"));
        candidates.add(Path.of("/opt/snr/lib/libether_sqlite_jni_runtime.so"));
        candidates.add(Path.of("libether_sqlite_jni_runtime.dylib"));
        candidates.add(Path.of("libether_sqlite_jni_runtime.so"));
        candidates.add(Path.of("lib/libether_sqlite_jni_runtime.dylib"));
        candidates.add(Path.of("lib/libether_sqlite_jni_runtime.so"));

        var tried = new StringBuilder();
        for (var candidate : candidates) {
            try {
                var abs = candidate.toAbsolutePath().normalize();
                if (!Files.exists(abs) || !Files.isRegularFile(abs)) {
                    tried.append("  ").append(abs).append(" (no existe)\n");
                    continue;
                }
                if (!candidate.isAbsolute()) {
                    System.err.println("[snr-jni] AVISO: cargando desde CWD: " + abs
                        + " — en producción define ether.sqlite.jni.lib o ETHER_SQLITE_JNI_LIB.");
                }
                System.load(abs.toString());
                return;
            } catch (Throwable t) {
                tried.append("  ").append(candidate.toAbsolutePath().normalize())
                    .append(" (").append(t.getClass().getSimpleName())
                    .append(": ").append(t.getMessage()).append(")\n");
            }
        }

        throw new IllegalStateException(
            "No se pudo cargar libether_sqlite_jni_runtime. " +
            "Define ether.sqlite.jni.lib o ETHER_SQLITE_JNI_LIB con la ruta al .so/.dylib.\n" +
            "Rutas intentadas:\n" + tried
        );
    }

    private SqliteLibraryJni() {}
}
