/// Configuración del worker leída desde variables de entorno.
///
/// Todas las variables tienen defaults razonables para desarrollo local.
/// En producción se recomienda usar un EnvironmentFile de systemd.
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    // ── MQTT ──────────────────────────────────────────────────────────────────
    /// URL del broker: `tcp://host:port` o `ssl://host:port`
    pub mqtt_broker: String,
    /// Client ID MQTT. Debe ser único por instancia del worker.
    pub mqtt_client_id: String,
    /// Topics suscritos, separados por coma.
    /// Default: `db/+/+/+/+/insert,db/+/+/+/+/insert_batch,db/+/+/+/+/upsert`
    pub mqtt_topics: Vec<String>,
    /// QoS: 0, 1 o 2. Default: 1
    pub mqtt_qos: u8,
    /// Usuario MQTT (opcional)
    pub mqtt_username: Option<String>,
    /// Contraseña MQTT (opcional)
    pub mqtt_password: Option<String>,

    // ── Worker ────────────────────────────────────────────────────────────────
    /// Identificador del worker. Aparece en topics de control:
    ///   `worker/{worker_id}/health`
    ///   `worker/{worker_id}/metrics`
    pub worker_id: String,

    // ── SQLite ────────────────────────────────────────────────────────────────
    /// Ruta al archivo SQLite. El directorio padre debe existir.
    pub sqlite_db: String,
    /// Timeout de busy en ms cuando otra conexión tiene el write lock.
    pub busy_timeout_ms: i32,

    // ── Colas de prioridad ────────────────────────────────────────────────────
    pub batch_size_high:   usize,
    pub batch_size_normal: usize,
    pub batch_size_low:    usize,
    pub flush_ms_high:     u64,
    pub flush_ms_normal:   u64,
    pub flush_ms_low:      u64,
    /// Capacidad del canal interno (mensajes en vuelo entre subscriber e inserter)
    pub channel_cap_high:   usize,
    pub channel_cap_normal: usize,
    pub channel_cap_low:    usize,

    // ── DLQ ──────────────────────────────────────────────────────────────────
    /// Publicar mensajes fallidos en `db/dlq/{tenant}/{database}/{entity}/{op}`
    pub dlq_enabled: bool,
    /// Número de reintentos antes de enviar al DLQ
    pub dlq_max_retries: u32,

    // ── Health / Metrics ──────────────────────────────────────────────────────
    /// Intervalo de publicación en el topic de salud (ms)
    pub health_interval_ms: u64,
    /// Intervalo de publicación de métricas (ms)
    pub metrics_interval_ms: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let hostname = hostname();

        let mqtt_broker = env_str("MQTT_BROKER", "tcp://localhost:1883");
        let mqtt_client_id = env_str("MQTT_CLIENT_ID", &format!("ether-worker-{hostname}"));
        let worker_id = env_str("WORKER_ID", &format!("ether-worker-{hostname}"));

        let topics_raw = env_str(
            "MQTT_TOPICS",
            "db/+/+/+/+/insert,db/+/+/+/+/insert_batch,db/+/+/+/+/upsert",
        );
        let mqtt_topics = topics_raw
            .split(',')
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect();

        let mqtt_qos = env_u64("MQTT_QOS", 1).min(2) as u8;

        let mqtt_username = env::var("MQTT_USERNAME").ok().filter(|s| !s.is_empty());
        let mqtt_password = env::var("MQTT_PASSWORD").ok().filter(|s| !s.is_empty());

        let sqlite_db = env_str("SQLITE_DB", "/var/lib/ether-worker/ingest.db");

        Self {
            mqtt_broker,
            mqtt_client_id,
            mqtt_topics,
            mqtt_qos,
            mqtt_username,
            mqtt_password,
            worker_id,

            sqlite_db,
            busy_timeout_ms: env_u64("SQLITE_BUSY_TIMEOUT_MS", 5_000) as i32,

            batch_size_high:   env_usize("BATCH_SIZE_HIGH",   100),
            batch_size_normal: env_usize("BATCH_SIZE_NORMAL", 500),
            batch_size_low:    env_usize("BATCH_SIZE_LOW",    2_000),
            flush_ms_high:   env_u64("FLUSH_MS_HIGH",   50),
            flush_ms_normal: env_u64("FLUSH_MS_NORMAL", 200),
            flush_ms_low:    env_u64("FLUSH_MS_LOW",    1_000),
            channel_cap_high:   env_usize("CHANNEL_CAP_HIGH",   10_000),
            channel_cap_normal: env_usize("CHANNEL_CAP_NORMAL", 50_000),
            channel_cap_low:    env_usize("CHANNEL_CAP_LOW",    100_000),

            dlq_enabled:     env_bool("DLQ_ENABLED",     true),
            dlq_max_retries: env_u64("DLQ_MAX_RETRIES", 3) as u32,

            health_interval_ms:  env_u64("HEALTH_INTERVAL_MS",  30_000),
            metrics_interval_ms: env_u64("METRICS_INTERVAL_MS", 10_000),
        }
    }

    /// Descompone la URL del broker en (host, port) para rumqttc.
    /// Soporta `tcp://host:port`, `ssl://host:port`, `host:port` y `host`.
    pub fn broker_host_port(&self) -> (String, u16) {
        let s = self.mqtt_broker
            .trim_start_matches("tcp://")
            .trim_start_matches("ssl://")
            .trim_start_matches("mqtt://");
        if let Some(colon) = s.rfind(':') {
            let host = s[..colon].to_owned();
            let port = s[colon + 1..].parse::<u16>().unwrap_or(1883);
            (host, port)
        } else {
            (s.to_owned(), 1883)
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn hostname() -> String {
    env::var("HOSTNAME")
        .or_else(|_| {
            let mut buf = [0u8; 64];
            let n = unsafe {
                libc_gethostname(buf.as_mut_ptr() as *mut i8, buf.len())
            };
            if n == 0 {
                let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                std::str::from_utf8(&buf[..end])
                    .map(|s| s.to_owned())
                    .map_err(|_| ())
            } else {
                Err(())
            }
        })
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// Wrapper minimal para gethostname — evita añadir el crate `libc`.
/// SAFETY: buf debe tener al menos `len` bytes de capacidad.
#[allow(non_snake_case)]
unsafe fn libc_gethostname(name: *mut i8, len: usize) -> i32 {
    extern "C" {
        fn gethostname(name: *mut i8, len: usize) -> i32;
    }
    gethostname(name, len)
}

fn env_str(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key).as_deref() {
        Ok("true" | "1" | "yes") => true,
        Ok("false" | "0" | "no") => false,
        _ => default,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_host_port_tcp_url() {
        let cfg = Config { mqtt_broker: "tcp://localhost:1883".into(), ..default_cfg() };
        assert_eq!(cfg.broker_host_port(), ("localhost".into(), 1883));
    }

    #[test]
    fn broker_host_port_ssl_url() {
        let cfg = Config { mqtt_broker: "ssl://broker.example.com:8883".into(), ..default_cfg() };
        assert_eq!(cfg.broker_host_port(), ("broker.example.com".into(), 8883));
    }

    #[test]
    fn broker_host_port_bare() {
        let cfg = Config { mqtt_broker: "mybroker".into(), ..default_cfg() };
        assert_eq!(cfg.broker_host_port(), ("mybroker".into(), 1883));
    }

    #[test]
    fn broker_host_port_host_colon_port() {
        let cfg = Config { mqtt_broker: "mybroker:1884".into(), ..default_cfg() };
        assert_eq!(cfg.broker_host_port(), ("mybroker".into(), 1884));
    }

    #[test]
    fn topics_parsed_from_comma_list() {
        // Simular env directamente via from_env no es fácil en tests paralelos,
        // así que solo testamos el helper de parseo.
        let raw = "db/+/+/+/+/insert, db/+/+/+/+/upsert ,";
        let topics: Vec<String> = raw
            .split(',')
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect();
        assert_eq!(topics, vec!["db/+/+/+/+/insert", "db/+/+/+/+/upsert"]);
    }

    fn default_cfg() -> Config {
        Config {
            mqtt_broker:        "tcp://localhost:1883".into(),
            mqtt_client_id:     "test-worker".into(),
            mqtt_topics:        vec!["db/+/+/+/+/insert".into()],
            mqtt_qos:           1,
            mqtt_username:      None,
            mqtt_password:      None,
            worker_id:          "test-worker".into(),
            sqlite_db:          "/tmp/test.db".into(),
            busy_timeout_ms:    5_000,
            batch_size_high:    100,
            batch_size_normal:  500,
            batch_size_low:     2_000,
            flush_ms_high:      50,
            flush_ms_normal:    200,
            flush_ms_low:       1_000,
            channel_cap_high:   10_000,
            channel_cap_normal: 50_000,
            channel_cap_low:    100_000,
            dlq_enabled:        true,
            dlq_max_retries:    3,
            health_interval_ms: 30_000,
            metrics_interval_ms: 10_000,
        }
    }
}
