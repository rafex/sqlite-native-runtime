package mx.rafex.sqlite;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests unitarios para {@link SqliteException}.
 * Cubre ambos constructores.
 */
class SqliteExceptionTest {

    @Test
    void constructorMessage() {
        var ex = new SqliteException("algo salió mal");
        assertEquals("algo salió mal", ex.getMessage());
        assertNull(ex.getCause());
    }

    @Test
    void constructorMessageAndCause() {
        var cause = new RuntimeException("causa raíz");
        var ex = new SqliteException("error con causa", cause);
        assertEquals("error con causa", ex.getMessage());
        assertSame(cause, ex.getCause());
    }

    @Test
    void isRuntimeException() {
        assertInstanceOf(RuntimeException.class, new SqliteException("x"));
    }
}
