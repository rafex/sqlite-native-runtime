package mx.rafex.ether.sqlite;

/**
 * Excepción lanzada por {@link SqliteConnection} y {@link SqliteStatement}
 * cuando una operación SQLite falla.
 *
 * <p>El mensaje incluye el error reportado por {@code snr_last_error()}.
 */
public final class SqliteException extends RuntimeException {

    public SqliteException(String message) {
        super(message);
    }

    public SqliteException(String message, Throwable cause) {
        super(message, cause);
    }
}
