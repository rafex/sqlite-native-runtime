//! Configuración del worker leída de variables de entorno.
//!
//! Variables soportadas (igual que el spike Java):
//!
//! | Variable        | Default                  | Descripción                          |
//! |-----------------|--------------------------|--------------------------------------|
//! | MQTT_BROKER     | tcp://localhost:1883     | URL del broker                       |
//! | MQTT_CLIENT_ID  | snr-spike-rust-{pid}     | Client ID MQTT                       |
//! | MQTT_TOPICS     | benchmark/#              | Topics separados por coma            |
//! | MQTT_QOS        | 1                        | QoS: 0, 1 o 2                       |
//! | SQLITE_DB       | /tmp/snr-spike-rust.db   | Ruta del archivo SQLite              |
//! | BATCH_SIZE      | 500                      | Máximo mensajes por transacción      |
//! | FLUSH_MS        | 200                      | Máximo ms de espera antes de flush   |

#[derive(Debug, Clone)]
pub struct Config {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id:   String,
    pub topics:      Vec<String>,
    pub qos:         u8,
    pub sqlite_db:   String,
    pub batch_size:  usize,
    pub flush_ms:    u64,
}

impl Config {
    pub fn from_env() -> Self {
        let broker = env("MQTT_BROKER", "tcp://localhost:1883");
        let (host, port) = parse_broker(&broker);

        let pid = std::process::id();
        let client_id = env("MQTT_CLIENT_ID", &format!("snr-spike-rust-{pid}"));

        let topics = env("MQTT_TOPICS", "benchmark/#")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let qos        = env("MQTT_QOS",    "1").parse().unwrap_or(1);
        let sqlite_db  = env("SQLITE_DB",   "/tmp/snr-spike-rust.db");
        let batch_size = env("BATCH_SIZE",  "500").parse().unwrap_or(500);
        let flush_ms   = env("FLUSH_MS",    "200").parse().unwrap_or(200);

        Config { broker_host: host, broker_port: port, client_id, topics, qos,
                 sqlite_db, batch_size, flush_ms }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Parsea "tcp://host:port" → (host, port).
fn parse_broker(broker: &str) -> (String, u16) {
    let s = broker
        .trim_start_matches("tcp://")
        .trim_start_matches("mqtt://")
        .trim_start_matches("ssl://");

    if let Some(colon) = s.rfind(':') {
        let host = s[..colon].to_string();
        let port = s[colon + 1..].parse().unwrap_or(1883);
        (host, port)
    } else {
        (s.to_string(), 1883)
    }
}
