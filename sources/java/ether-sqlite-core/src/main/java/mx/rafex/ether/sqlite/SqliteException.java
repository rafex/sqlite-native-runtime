package mx.rafex.ether.sqlite;

/**
 * Excepción lanzada por {@link SqliteConnection} y {@link SqliteStatement}
 * cuando una operación SQLite falla.
 *
 * <p>El mensaje incluye el error reportado por la capa nativa (FFM o JNI).
 */
public class SqliteException extends RuntimeException {

    public SqliteException(String message) {
        super(message);
    }

    public SqliteException(String message, Throwable cause) {
        super(message, cause);
    }
}
