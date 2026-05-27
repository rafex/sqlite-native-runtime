//! Inserter de mensajes MQTT en SQLite — corre en un thread OS bloqueante.
//!
//! Patrón: recibe mensajes por `std::sync::mpsc::Receiver`,
//! acumula hasta `batch_size` o `flush_ms`, luego hace BEGIN → N inserts → COMMIT.

use rusqlite::{Connection, params};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// Un mensaje listo para insertar.
#[derive(Debug)]
pub struct MqttMsg {
    pub topic:       String,
    pub payload:     String,
    pub received_ms: i64,
}

/// Estado del inserter.
pub struct Inserter {
    conn:       Connection,
    batch_size: usize,
    flush_dur:  Duration,

    // métricas
    pub committed: u64,
    pub batches:   u64,
    pub errors:    u64,
    start:         Instant,
}

impl Inserter {
    /// Abre la BD, activa WAL y crea el schema si no existe.
    pub fn new(db_path: &str, batch_size: usize, flush_ms: u64) -> rusqlite::Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous  = NORMAL;
            PRAGMA busy_timeout = 5000;
            CREATE TABLE IF NOT EXISTS mqtt_messages (
                id          INTEGER PRIMARY KEY,
                topic       TEXT    NOT NULL,
                payload     TEXT,
                received_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_mqtt_topic ON mqtt_messages(topic);
            CREATE INDEX IF NOT EXISTS idx_mqtt_ts    ON mqtt_messages(received_ms);
        ")?;

        Ok(Inserter {
            conn,
            batch_size,
            flush_dur: Duration::from_millis(flush_ms),
            committed: 0,
            batches:   0,
            errors:    0,
            start:     Instant::now(),
        })
    }

    /// Loop principal: drena el canal hasta que se cierra.
    pub fn run(&mut self, rx: Receiver<MqttMsg>) {
        loop {
            let mut batch = Vec::with_capacity(self.batch_size);

            // Espera el primer mensaje con timeout
            match rx.recv_timeout(self.flush_dur) {
                Ok(msg) => {
                    batch.push(msg);
                    // Drena sin espera hasta completar el batch
                    while batch.len() < self.batch_size {
                        match rx.try_recv() {
                            Ok(m)  => batch.push(m),
                            Err(_) => break,
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // nada en este ventana — continuar
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // canal cerrado: el subscriber terminó
                    break;
                }
            }

            if batch.is_empty() {
                continue;
            }

            let t = Instant::now();
            match self.insert_batch(&batch) {
                Ok(()) => {
                    let ms = t.elapsed().as_millis();
                    self.committed += batch.len() as u64;
                    self.batches   += 1;

                    if self.batches % 20 == 0 {
                        let elapsed = self.start.elapsed().as_secs_f64().max(0.001);
                        eprintln!(
                            "[inserter] committed={} batches={} tps={:.0}/s commit={}ms",
                            self.committed, self.batches,
                            self.committed as f64 / elapsed,
                            ms
                        );
                    }
                }
                Err(e) => {
                    self.errors += 1;
                    eprintln!("[inserter] ERROR batch={} err={e}", batch.len());
                }
            }
        }

        // resumen final
        let elapsed = self.start.elapsed().as_secs_f64().max(0.001);
        eprintln!(
            "[inserter] FINAL committed={} batches={} errors={} tps={:.0}/s",
            self.committed, self.batches, self.errors,
            self.committed as f64 / elapsed
        );
    }

    /// Inserta un batch en una única transacción.
    fn insert_batch(&self, batch: &[MqttMsg]) -> rusqlite::Result<()> {
        // unchecked_transaction: no verifica autocommit mode.
        // Para el spike es correcto — siempre corremos sin transacción activa.
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO mqtt_messages(topic, payload, received_ms) VALUES(?1, ?2, ?3)",
            )?;
            for msg in batch {
                stmt.execute(params![msg.topic, msg.payload, msg.received_ms])?;
            }
        }
        tx.commit()
    }
}
