/// Parseo y validación del topic MQTT.
///
/// Patrón de producción:
///   `db/{priority}/{tenant}/{database}/{entity}/{operation}`
///
/// Ejemplo:
///   `db/high/acme/crm/contact/insert`
use std::fmt;

/// Prioridad de procesamiento. Determina qué cola y qué batch/flush config usar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Priority {
    High,
    Normal,
    Low,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::High   => "high",
            Priority::Normal => "normal",
            Priority::Low    => "low",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for Priority {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "high"   => Ok(Priority::High),
            "normal" => Ok(Priority::Normal),
            "low"    => Ok(Priority::Low),
            other    => Err(format!("prioridad desconocida: '{other}'")),
        }
    }
}

/// Operación que se aplicará sobre la entidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Insert,
    InsertBatch,
    Upsert,
    Update,
    Delete,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Operation::Insert      => "insert",
            Operation::InsertBatch => "insert_batch",
            Operation::Upsert      => "upsert",
            Operation::Update      => "update",
            Operation::Delete      => "delete",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for Operation {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "insert"       => Ok(Operation::Insert),
            "insert_batch" => Ok(Operation::InsertBatch),
            "upsert"       => Ok(Operation::Upsert),
            "update"       => Ok(Operation::Update),
            "delete"       => Ok(Operation::Delete),
            other          => Err(format!("operación desconocida: '{other}'")),
        }
    }
}

/// Resultado del parseo de un topic MQTT de producción.
#[derive(Debug, Clone)]
pub struct ParsedTopic {
    pub priority:      Priority,
    pub tenant:        String,
    pub database_name: String,
    pub entity:        String,
    pub operation:     Operation,
}

/// Parsea un topic con el patrón `db/{priority}/{tenant}/{database}/{entity}/{operation}`.
///
/// Devuelve `Err(String)` si el topic no cumple el patrón o contiene valores inválidos.
/// El mensaje de error describe la causa y puede usarse para el DLQ.
pub fn parse_topic(topic: &str) -> Result<ParsedTopic, String> {
    // Dividir en exactamente 6 segmentos. splitn(7, '/') con límite 7 permite detectar
    // topics con más de 6 segmentos (devolverían el 7.º segmento en el último slot).
    let parts: Vec<&str> = topic.splitn(7, '/').collect();

    if parts.len() != 6 {
        return Err(format!(
            "topic inválido: se esperaban 6 segmentos separados por '/', se encontraron {}. Topic: '{topic}'",
            parts.len()
        ));
    }

    if parts[0] != "db" {
        return Err(format!(
            "topic inválido: debe comenzar con 'db/', comenzó con '{}'. Topic: '{topic}'",
            parts[0]
        ));
    }

    let priority  = Priority::try_from(parts[1])
        .map_err(|e| format!("{e}. Topic: '{topic}'"))?;
    let tenant        = validate_segment(parts[2], "tenant",   topic)?;
    let database_name = validate_segment(parts[3], "database", topic)?;
    let entity        = validate_segment(parts[4], "entity",   topic)?;
    let operation = Operation::try_from(parts[5])
        .map_err(|e| format!("{e}. Topic: '{topic}'"))?;

    Ok(ParsedTopic { priority, tenant, database_name, entity, operation })
}

/// Valida que un segmento no esté vacío y no contenga caracteres problemáticos.
fn validate_segment(seg: &str, field: &str, topic: &str) -> Result<String, String> {
    if seg.is_empty() {
        return Err(format!(
            "topic inválido: el campo '{field}' está vacío. Topic: '{topic}'"
        ));
    }
    // Rechazar wildcards MQTT en posiciones que deben ser valores concretos
    if seg == "+" || seg == "#" {
        return Err(format!(
            "topic inválido: '{field}' no puede ser un wildcard MQTT ('{seg}'). Topic: '{topic}'"
        ));
    }
    Ok(seg.to_owned())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_insert() {
        let t = parse_topic("db/normal/acme/crm/contact/insert").unwrap();
        assert_eq!(t.priority,      Priority::Normal);
        assert_eq!(t.tenant,        "acme");
        assert_eq!(t.database_name, "crm");
        assert_eq!(t.entity,        "contact");
        assert_eq!(t.operation,     Operation::Insert);
    }

    #[test]
    fn parse_high_upsert() {
        let t = parse_topic("db/high/tenant-x/db1/order/upsert").unwrap();
        assert_eq!(t.priority,  Priority::High);
        assert_eq!(t.operation, Operation::Upsert);
    }

    #[test]
    fn parse_low_delete() {
        let t = parse_topic("db/low/my_tenant/analytics/event/delete").unwrap();
        assert_eq!(t.priority,  Priority::Low);
        assert_eq!(t.operation, Operation::Delete);
    }

    #[test]
    fn parse_insert_batch() {
        let t = parse_topic("db/normal/acme/inventory/product/insert_batch").unwrap();
        assert_eq!(t.operation, Operation::InsertBatch);
    }

    #[test]
    fn error_wrong_prefix() {
        let err = parse_topic("mqtt/high/t/d/e/insert").unwrap_err();
        assert!(err.contains("debe comenzar con 'db/'"));
    }

    #[test]
    fn error_too_few_segments() {
        let err = parse_topic("db/high/tenant/db/entity").unwrap_err();
        assert!(err.contains("6 segmentos"));
    }

    #[test]
    fn error_too_many_segments() {
        let err = parse_topic("db/high/t/d/e/insert/extra").unwrap_err();
        assert!(err.contains("6 segmentos"));
    }

    #[test]
    fn error_unknown_priority() {
        let err = parse_topic("db/critical/t/d/e/insert").unwrap_err();
        assert!(err.contains("prioridad desconocida"));
    }

    #[test]
    fn error_unknown_operation() {
        let err = parse_topic("db/normal/t/d/e/truncate").unwrap_err();
        assert!(err.contains("operación desconocida"));
    }

    #[test]
    fn error_wildcard_in_tenant() {
        let err = parse_topic("db/normal/+/db/entity/insert").unwrap_err();
        assert!(err.contains("wildcard MQTT"));
    }

    #[test]
    fn error_empty_entity() {
        let err = parse_topic("db/normal/tenant/db//insert").unwrap_err();
        assert!(err.contains("entity") && err.contains("vacío"));
    }

    #[test]
    fn priority_display() {
        assert_eq!(format!("{}", Priority::High),   "high");
        assert_eq!(format!("{}", Priority::Normal), "normal");
        assert_eq!(format!("{}", Priority::Low),    "low");
    }

    #[test]
    fn operation_as_str() {
        assert_eq!(Operation::InsertBatch.as_str(), "insert_batch");
        assert_eq!(Operation::Update.as_str(),      "update");
    }
}
