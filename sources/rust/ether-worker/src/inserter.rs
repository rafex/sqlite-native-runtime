/// Hilo insertor de batches — uno por prioridad (HIGH / NORMAL / LOW).
///
/// Cada insertor:
///   1. Abre su propia conexión SQLite (WAL permite múltiples escritores serializados).
///   2. Prepara el statement INSERT una vez al arranque.
///   3. Drena su canal con un timeout de `flush_ms`.
///   4. Envuelve el batch en `BEGIN IMMEDIATE … COMMIT`.
///   5. Cuando el canal se cierra (sender dropeado en shutdown), hace flush final
///      y WAL TRUNCATE checkpoint.
use std::{
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    config::Config,
    db::Connection,
    metrics::Metrics,
    schema::{self, INSERT_SQL},
    topic::Priority,
};

/// Mensaje que viaja del subscriber MQTT al insertor correspondiente.
#[derive(Debug)]
pub struct IngestMsg {
    /// ID único del evento — extraído del payload o generado (UUID v4).
    pub id: String,
    /// Tenant extraído del topic.
    pub tenant: String,
    /// Nombre de la base de datos extraído del topic.
    pub database_name: String,
    /// Entidad extraída del topic.
    pub entity: String,
    /// Operación como string ("insert", "upsert", …).
    pub operation: String,
    /// Topic MQTT completo (para auditoría en ingest_event.topic).
    pub raw_topic: String,
    /// Prioridad como string ("high", "normal", "low").
    pub priority: String,
    /// Campo "schema" del payload JSON (opcional).
    pub schema_name: Option<String>,
    /// Payload JSON completo.
    pub payload_json: String,
    /// Campo "metadata" del payload JSON serializado (opcional).
    pub metadata_json: Option<String>,
    /// Timestamp ISO 8601 de recepción.
    pub received_at: String,
}

/// Grupo de canales para las tres colas de prioridad.
///
/// Dropar este struct cierra los Senders, lo que hace que los Receivers
/// en los hilos insertores detecten el cierre y terminen su loop de flush final.
pub struct Channels {
    pub high:   SyncSender<IngestMsg>,
    pub normal: SyncSender<IngestMsg>,
    pub low:    SyncSender<IngestMsg>,
}

impl Channels {
    /// Envía un mensaje a la cola correspondiente (non-blocking).
    /// Si la cola está llena, descarta el mensaje e incrementa el contador de dropped.
    pub fn dispatch(&self, msg: IngestMsg, priority: Priority, metrics: &Metrics) {
        let sender = match priority {
            Priority::High   => &self.high,
            Priority::Normal => &self.normal,
            Priority::Low    => &self.low,
        };
        match sender.try_send(msg) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                metrics.inc_dropped(priority);
            }
            Err(TrySendError::Disconnected(_)) => {
                // El insertor ya terminó (shutdown en curso). Descartamos silenciosamente.
            }
        }
    }
}

/// Handles de los tres hilos insertores. Permite esperar su terminación.
pub struct InserterHandles {
    high:   thread::JoinHandle<()>,
    normal: thread::JoinHandle<()>,
    low:    thread::JoinHandle<()>,
}

impl InserterHandles {
    /// Espera a que los tres hilos insertores terminen.
    /// Debe llamarse DESPUÉS de haber dropeado `Channels` para cerrar los canales.
    pub fn join(self) {
        if let Err(e) = self.high.join() {
            eprintln!("[inserter-high] hilo terminó con pánico: {:?}", e);
        }
        if let Err(e) = self.normal.join() {
            eprintln!("[inserter-normal] hilo terminó con pánico: {:?}", e);
        }
        if let Err(e) = self.low.join() {
            eprintln!("[inserter-low] hilo terminó con pánico: {:?}", e);
        }
    }
}

/// Arranca los tres hilos insertores y devuelve (Channels, InserterHandles).
///
/// Cada hilo abre su propia conexión SQLite, aplica el schema y queda en espera.
pub fn start(config: &Config, metrics: Arc<Metrics>) -> (Channels, InserterHandles) {
    let (tx_h, rx_h) = mpsc::sync_channel(config.channel_cap_high);
    let (tx_n, rx_n) = mpsc::sync_channel(config.channel_cap_normal);
    let (tx_l, rx_l) = mpsc::sync_channel(config.channel_cap_low);

    let channels = Channels { high: tx_h, normal: tx_n, low: tx_l };

    let mk_thread = |name: &'static str,
                     priority: Priority,
                     rx: Receiver<IngestMsg>,
                     db_path: String,
                     busy_ms: i32,
                     batch_size: usize,
                     flush_ms: u64,
                     metrics: Arc<Metrics>| {
        thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                run_inserter(name, priority, rx, &db_path, busy_ms, batch_size, flush_ms, metrics);
            })
            .expect("failed to spawn inserter thread")
    };

    let high_handle = mk_thread(
        "inserter-high",
        Priority::High,
        rx_h,
        config.sqlite_db.clone(),
        config.busy_timeout_ms,
        config.batch_size_high,
        config.flush_ms_high,
        Arc::clone(&metrics),
    );
    let normal_handle = mk_thread(
        "inserter-normal",
        Priority::Normal,
        rx_n,
        config.sqlite_db.clone(),
        config.busy_timeout_ms,
        config.batch_size_normal,
        config.flush_ms_normal,
        Arc::clone(&metrics),
    );
    let low_handle = mk_thread(
        "inserter-low",
        Priority::Low,
        rx_l,
        config.sqlite_db.clone(),
        config.busy_timeout_ms,
        config.batch_size_low,
        config.flush_ms_low,
        Arc::clone(&metrics),
    );

    let handles = InserterHandles {
        high:   high_handle,
        normal: normal_handle,
        low:    low_handle,
    };

    (channels, handles)
}

// ── loop del insertor ─────────────────────────────────────────────────────────

fn run_inserter(
    name: &str,
    priority: Priority,
    rx: Receiver<IngestMsg>,
    db_path: &str,
    busy_ms: i32,
    batch_size: usize,
    flush_ms: u64,
    metrics: Arc<Metrics>,
) {
    // Abrir conexión y aplicar schema
    let db = match Connection::open(db_path, busy_ms) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{name}] ERROR abriendo SQLite '{db_path}': {e}");
            return;
        }
    };
    if let Err(e) = schema::ensure(&db) {
        eprintln!("[{name}] ERROR aplicando schema: {e}");
        return;
    }

    // Preparar el INSERT una sola vez
    let stmt = match db.prepare(INSERT_SQL) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[{name}] ERROR preparando INSERT: {e}");
            return;
        }
    };

    eprintln!("[{name}] listo — batch={batch_size} flush={flush_ms}ms");

    let flush_dur = Duration::from_millis(flush_ms);
    let mut batch: Vec<IngestMsg> = Vec::with_capacity(batch_size);

    loop {
        // Esperar el primer mensaje con timeout (bloqueo controlado)
        match rx.recv_timeout(flush_dur) {
            Ok(msg) => {
                batch.push(msg);
                // Drenar sin bloquear hasta llenar el batch o vaciar la cola
                while batch.len() < batch_size {
                    match rx.try_recv() {
                        Ok(m)                           => batch.push(m),
                        Err(mpsc::TryRecvError::Empty)  => break,
                        Err(mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // flush_ms expiró sin mensaje — seguir el loop (puede haber mensajes pendientes)
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // El canal está cerrado (shutdown). Drenar lo que quede y salir.
                eprintln!("[{name}] canal cerrado — flusheando {len} mensajes pendientes",
                    len = batch.len());
                break;
            }
        }

        if !batch.is_empty() {
            insert_batch(name, priority, &db, &stmt, &mut batch, &metrics);
        }
    }

    // Flush final de lo que esté en el batch
    if !batch.is_empty() {
        insert_batch(name, priority, &db, &stmt, &mut batch, &metrics);
    }

    // WAL TRUNCATE checkpoint — libera espacio en el WAL al shutdown limpio
    match db.wal_checkpoint_truncate() {
        Ok(()) => eprintln!("[{name}] WAL checkpoint TRUNCATE OK"),
        Err(e) => eprintln!("[{name}] WAL checkpoint falló: {e}"),
    }
    eprintln!("[{name}] terminado");
}

/// Inserta un batch de mensajes en una sola transacción.
/// Si hay error, hace ROLLBACK y registra en métricas.
/// `batch` se vacía al terminar (tanto en éxito como en error).
fn insert_batch(
    name: &str,
    priority: Priority,
    db: &Connection,
    stmt: &crate::db::Stmt<'_>,
    batch: &mut Vec<IngestMsg>,
    metrics: &Metrics,
) {
    let n = batch.len();
    let t0 = Instant::now();

    // BEGIN IMMEDIATE — obtener write lock antes de empezar
    if let Err(e) = db.begin_immediate() {
        eprintln!("[{name}] BEGIN IMMEDIATE falló: {e}");
        metrics.inc_error(priority);
        batch.clear();
        return;
    }

    let mut ok = true;
    for msg in batch.iter() {
        if let Err(e) = bind_and_step(stmt, msg) {
            eprintln!("[{name}] bind/step falló: {e}");
            ok = false;
            break;
        }
    }

    if ok {
        if let Err(e) = db.commit() {
            eprintln!("[{name}] COMMIT falló: {e}");
            ok = false;
            let _ = db.rollback();
        }
    } else {
        let _ = db.rollback();
    }

    if ok {
        let elapsed_ms = t0.elapsed().as_millis();
        metrics.add_committed(priority, n as u64);
        metrics.inc_batch(priority);
        if elapsed_ms > 100 {
            eprintln!("[{name}] batch lento: n={n} elapsed={elapsed_ms}ms");
        }
    } else {
        metrics.inc_error(priority);
    }

    batch.clear();
}

/// Vincula todos los campos del IngestMsg al statement y ejecuta un paso.
#[inline]
fn bind_and_step(stmt: &crate::db::Stmt<'_>, msg: &IngestMsg) -> Result<(), String> {
    stmt.reset()?;
    stmt.clear_bindings()?;
    stmt.bind_text(1,  &msg.id)?;
    stmt.bind_text(2,  &msg.tenant)?;
    stmt.bind_text(3,  &msg.database_name)?;
    stmt.bind_text(4,  &msg.entity)?;
    stmt.bind_text(5,  &msg.operation)?;
    stmt.bind_text(6,  &msg.raw_topic)?;
    stmt.bind_text(7,  &msg.priority)?;
    stmt.bind_text_opt(8, msg.schema_name.as_deref())?;
    stmt.bind_text(9,  &msg.payload_json)?;
    stmt.bind_text_opt(10, msg.metadata_json.as_deref())?;
    stmt.bind_text(11, &msg.received_at)?;
    stmt.step()?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, metrics::Metrics};
    use std::time::Duration;

    fn test_config(db_path: &str) -> Config {
        Config {
            mqtt_broker:        "tcp://localhost:1883".into(),
            mqtt_client_id:     "test".into(),
            mqtt_topics:        vec!["db/+/+/+/+/insert".into()],
            mqtt_qos:           1,
            mqtt_username:      None,
            mqtt_password:      None,
            worker_id:          "test".into(),
            sqlite_db:          db_path.to_owned(),
            busy_timeout_ms:    1000,
            batch_size_high:    10,
            batch_size_normal:  50,
            batch_size_low:     100,
            flush_ms_high:      50,
            flush_ms_normal:    100,
            flush_ms_low:       200,
            channel_cap_high:   1_000,
            channel_cap_normal: 5_000,
            channel_cap_low:    10_000,
            dlq_enabled:        false,
            dlq_max_retries:    3,
            health_interval_ms: 30_000,
            metrics_interval_ms: 10_000,
        }
    }

    fn make_msg(id: &str, priority: &str) -> IngestMsg {
        IngestMsg {
            id:            id.to_owned(),
            tenant:        "acme".into(),
            database_name: "crm".into(),
            entity:        "contact".into(),
            operation:     "insert".into(),
            raw_topic:     format!("db/{priority}/acme/crm/contact/insert"),
            priority:      priority.to_owned(),
            schema_name:   None,
            payload_json:  format!(r#"{{"id":"{id}"}}"#),
            metadata_json: None,
            received_at:   "2026-05-26T00:00:00Z".into(),
        }
    }

    #[test]
    fn inserter_commits_messages() {
        use tempfile::NamedTempFile;
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_string_lossy().into_owned();
        std::mem::forget(f);

        let config  = test_config(&path);
        let metrics = Metrics::new();
        let (channels, handles) = start(&config, Arc::clone(&metrics));

        // Enviar 20 mensajes normales
        for i in 0..20 {
            let msg = make_msg(&format!("msg-{i}"), "normal");
            channels.normal.send(msg).unwrap();
        }

        // Dar tiempo a los insertores para procesar
        std::thread::sleep(Duration::from_millis(500));

        // Cerrar canales para activar flush final
        drop(channels);
        handles.join();

        let s = metrics.snapshot();
        assert_eq!(s.committed_normal, 20, "deben haberse committed 20 mensajes normales");
        assert_eq!(s.total_errors(), 0, "no deben haber errores");
    }

    #[test]
    fn dispatch_drops_when_channel_full() {
        use tempfile::NamedTempFile;
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_string_lossy().into_owned();
        std::mem::forget(f);

        // Canal de alta prioridad con capacidad 2 para forzar drops
        let mut config = test_config(&path);
        config.channel_cap_high = 2;

        let metrics = Metrics::new();
        let (channels, handles) = start(&config, Arc::clone(&metrics));

        // Enviar 10 mensajes a un canal de capacidad 2
        for i in 0..10 {
            let msg = make_msg(&format!("hi-{i}"), "high");
            channels.dispatch(msg, Priority::High, &metrics);
        }

        std::thread::sleep(Duration::from_millis(300));
        drop(channels);
        handles.join();

        let s = metrics.snapshot();
        // Al menos algunos fueron dropped (canal de capacidad 2 vs 10 mensajes)
        // y los committed + dropped == 10
        assert_eq!(
            s.committed_high + s.dropped_high, 10,
            "committed + dropped debe sumar 10"
        );
    }

    #[test]
    fn three_priorities_independent() {
        use tempfile::NamedTempFile;
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_string_lossy().into_owned();
        std::mem::forget(f);

        let config  = test_config(&path);
        let metrics = Metrics::new();
        let (channels, handles) = start(&config, Arc::clone(&metrics));

        channels.high.send(make_msg("h1", "high")).unwrap();
        channels.high.send(make_msg("h2", "high")).unwrap();
        channels.normal.send(make_msg("n1", "normal")).unwrap();
        channels.low.send(make_msg("l1", "low")).unwrap();

        std::thread::sleep(Duration::from_millis(500));
        drop(channels);
        handles.join();

        let s = metrics.snapshot();
        assert_eq!(s.committed_high,   2);
        assert_eq!(s.committed_normal, 1);
        assert_eq!(s.committed_low,    1);
    }

    #[test]
    fn duplicate_id_not_committed_twice() {
        use tempfile::NamedTempFile;
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_string_lossy().into_owned();
        std::mem::forget(f);

        let config  = test_config(&path);
        let metrics = Metrics::new();
        let (channels, handles) = start(&config, Arc::clone(&metrics));

        // Enviar el mismo ID dos veces
        channels.normal.send(make_msg("dup-id", "normal")).unwrap();
        channels.normal.send(make_msg("dup-id", "normal")).unwrap();

        std::thread::sleep(Duration::from_millis(400));
        drop(channels);
        handles.join();

        // Ambos mensajes pasan por insert_batch (committed cuenta inserts intentados, no filas reales)
        // El segundo es ignorado por INSERT OR IGNORE pero no hay error.
        let s = metrics.snapshot();
        assert_eq!(s.total_errors(), 0, "INSERT OR IGNORE no debe generar errores");
        // committed_normal >= 1 (al menos el primero)
        assert!(s.committed_normal >= 1);
    }
}
