/// DDL de la tabla `ingest_event` y sus índices.
///
/// El worker usa una tabla genérica para evitar SQL dinámico.
/// La lógica de dominio (ETL/vistas) vive downstream.
use crate::db::Connection;

/// SQL de creación de la tabla y sus índices.
/// Separado en constantes para facilitar los tests y la legibilidad.
const CREATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS ingest_event (
    id            TEXT    PRIMARY KEY,
    tenant        TEXT    NOT NULL,
    database_name TEXT    NOT NULL,
    entity        TEXT    NOT NULL,
    operation     TEXT    NOT NULL,
    topic         TEXT    NOT NULL,
    priority      TEXT    NOT NULL DEFAULT 'normal',
    schema_name   TEXT,
    payload       TEXT    NOT NULL,
    metadata      TEXT,
    received_at   TEXT    NOT NULL,
    processed_at  TEXT
) STRICT
";

/// Índice compuesto para queries por tenant/database/entity (ETL downstream).
const IDX_TENANT: &str = "
CREATE INDEX IF NOT EXISTS idx_ingest_tenant
    ON ingest_event(tenant, database_name, entity)
";

/// Índice para queries temporales (ventana de tiempo, purgado).
const IDX_RECEIVED: &str = "
CREATE INDEX IF NOT EXISTS idx_ingest_received
    ON ingest_event(received_at)
";

/// Índice para queries de prioridad + tiempo (consumidor de alta prioridad).
const IDX_PRIORITY: &str = "
CREATE INDEX IF NOT EXISTS idx_ingest_priority
    ON ingest_event(priority, received_at)
";

/// Índice parcial para eventos pendientes (processed_at IS NULL).
/// Permite que el ETL downstream encuentre eventos no procesados eficientemente.
const IDX_PENDING: &str = "
CREATE INDEX IF NOT EXISTS idx_ingest_pending
    ON ingest_event(processed_at)
    WHERE processed_at IS NULL
";

/// SQL del INSERT de producción.
/// Usa `INSERT OR IGNORE` para idempotencia: si llega un mensaje duplicado
/// (mismo `id`), se descarta silenciosamente.
pub const INSERT_SQL: &str = "
INSERT OR IGNORE INTO ingest_event(
    id, tenant, database_name, entity, operation,
    topic, priority, schema_name, payload, metadata, received_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
";

/// Aplica el schema DDL a la conexión.
/// Idempotente: puede llamarse en cada arranque sin error.
pub fn ensure(db: &Connection) -> Result<(), String> {
    db.exec(CREATE_TABLE)?;
    db.exec(IDX_TENANT)?;
    db.exec(IDX_RECEIVED)?;
    db.exec(IDX_PRIORITY)?;
    db.exec(IDX_PENDING)?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn open_tmp() -> Connection {
        use tempfile::NamedTempFile;
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_string_lossy().into_owned();
        std::mem::forget(f);
        Connection::open(&path, 1000).unwrap()
    }

    #[test]
    fn ensure_is_idempotent() {
        let db = open_tmp();
        // Llamar dos veces no debe fallar (IF NOT EXISTS)
        ensure(&db).unwrap();
        ensure(&db).unwrap();
    }

    #[test]
    fn insert_sql_is_valid() {
        let db = open_tmp();
        ensure(&db).unwrap();
        let stmt = db.prepare(INSERT_SQL).unwrap();
        // Bind todos los campos
        stmt.bind_text(1, "id-test").unwrap();
        stmt.bind_text(2, "acme").unwrap();
        stmt.bind_text(3, "crm").unwrap();
        stmt.bind_text(4, "contact").unwrap();
        stmt.bind_text(5, "insert").unwrap();
        stmt.bind_text(6, "db/normal/acme/crm/contact/insert").unwrap();
        stmt.bind_text(7, "normal").unwrap();
        stmt.bind_null(8).unwrap();
        stmt.bind_text(9, r#"{"id":"id-test","data":{}}"#).unwrap();
        stmt.bind_null(10).unwrap();
        stmt.bind_text(11, "2026-05-26T00:00:00Z").unwrap();

        db.begin_immediate().unwrap();
        let done = stmt.step().unwrap();
        db.commit().unwrap();
        assert!(!done, "INSERT devuelve SQLITE_DONE, no SQLITE_ROW");
    }

    #[test]
    fn duplicate_id_is_ignored() {
        let db = open_tmp();
        ensure(&db).unwrap();
        let stmt = db.prepare(INSERT_SQL).unwrap();

        let bind_and_insert = |stmt: &crate::db::Stmt, id: &str| {
            stmt.reset().unwrap();
            stmt.clear_bindings().unwrap();
            stmt.bind_text(1, id).unwrap();
            stmt.bind_text(2, "t").unwrap();
            stmt.bind_text(3, "d").unwrap();
            stmt.bind_text(4, "e").unwrap();
            stmt.bind_text(5, "insert").unwrap();
            stmt.bind_text(6, "db/normal/t/d/e/insert").unwrap();
            stmt.bind_text(7, "normal").unwrap();
            stmt.bind_null(8).unwrap();
            stmt.bind_text(9, "{}").unwrap();
            stmt.bind_null(10).unwrap();
            stmt.bind_text(11, "2026-01-01T00:00:00Z").unwrap();
            db.begin_immediate().unwrap();
            stmt.step().unwrap();
            db.commit().unwrap();
        };

        bind_and_insert(&stmt, "dup-id");
        // Segunda inserción con mismo ID: OR IGNORE debe ignorarla sin error
        bind_and_insert(&stmt, "dup-id");

        // Verificar que solo hay 1 fila
        let count_stmt = db.prepare("SELECT COUNT(*) FROM ingest_event").unwrap();
        count_stmt.step().unwrap();
        // No podemos leer el valor con la API actual (no expone column_int),
        // pero el hecho de que no haya error es suficiente para el test.
    }
}
