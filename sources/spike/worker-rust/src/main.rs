//! Spike C — worker MQTT → SQLite en Rust.
//!
//! # Diseño
//!
//! ```text
//! tokio async (subscriber MQTT)
//!   │  rumqttc eventloop
//!   │  SyncSender<MqttMsg> — try_send (non-blocking)
//!   ▼
//! std::sync::mpsc channel (bounded)
//!   │
//!   ▼
//! OS thread bloqueante (Inserter)
//!   │  recv_timeout(flush_ms)
//!   │  BEGIN → N inserts → COMMIT
//!   ▼
//! SQLite WAL (rusqlite bundled)
//! ```
//!
//! # Ventajas del binario único
//!
//! - SQLite compilado dentro del binario (`rusqlite` feature `bundled`)
//! - Sin `.so` externa, sin JVM, sin GraalVM en el servidor de destino
//! - ~5-10 MB en disco, ~4-10 MB RSS en steady state
//!
//! # Uso
//!
//! ```sh
//! cargo build --release
//! MQTT_BROKER=tcp://localhost:1883 SQLITE_DB=/tmp/bench.db ./target/release/snr-mqtt-worker
//! ```

mod config;
mod inserter;

use config::Config;
use inserter::{Inserter, MqttMsg};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use std::sync::mpsc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    let jvm_start = Instant::now();

    let config = Config::from_env();
    eprintln!("[worker] config = {config:?}");

    // ── 1. Canal subscriber → inserter ───────────────────────────────────────
    // SyncSender: try_send desde async (no bloquea el runtime tokio).
    // Capacidad = batch_size × 20 para absorber ráfagas.
    let channel_cap = config.batch_size * 20;
    let (tx, rx) = mpsc::sync_channel::<MqttMsg>(channel_cap);

    // ── 2. Inserter en thread OS bloqueante ───────────────────────────────────
    let db_path    = config.sqlite_db.clone();
    let batch_size = config.batch_size;
    let flush_ms   = config.flush_ms;

    let inserter_handle = std::thread::spawn(move || {
        let mut ins = Inserter::new(&db_path, batch_size, flush_ms)
            .expect("No se pudo abrir SQLite");
        ins.run(rx);
    });

    // ── 3. MQTT client ────────────────────────────────────────────────────────
    let qos = match config.qos {
        0 => QoS::AtMostOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtLeastOnce,
    };

    let mut mqtt_opts = MqttOptions::new(
        &config.client_id,
        &config.broker_host,
        config.broker_port,
    );
    mqtt_opts.set_keep_alive(std::time::Duration::from_secs(30));
    mqtt_opts.set_clean_session(true);
    // Buffer interno de mensajes entrantes
    mqtt_opts.set_max_packet_size(256 * 1024, 256 * 1024);

    let (client, mut eventloop) = AsyncClient::new(mqtt_opts, channel_cap);

    for topic in &config.topics {
        client
            .subscribe(topic, qos)
            .await
            .expect("subscribe failed");
    }

    let startup_ms = jvm_start.elapsed().as_millis();
    eprintln!(
        "[worker] MQTT connected — startup={}ms topics={:?}",
        startup_ms, config.topics
    );

    // ── 4. Event loop ─────────────────────────────────────────────────────────
    let mut received:  u64 = 0;
    let mut dropped:   u64 = 0;
    let start = Instant::now();

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                let received_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;

                let msg = MqttMsg {
                    topic:       p.topic.clone(),
                    payload:     String::from_utf8_lossy(&p.payload).into_owned(),
                    received_ms,
                };

                received += 1;

                // try_send: nunca bloquea el runtime.
                // Si el canal está lleno, el mensaje se descarta (backpressure medible).
                if tx.try_send(msg).is_err() {
                    dropped += 1;
                }

                if received % 10_000 == 0 {
                    let elapsed = start.elapsed().as_secs_f64().max(0.001);
                    eprintln!(
                        "[subscriber] received={} dropped={} tps={:.0}/s",
                        received, dropped, received as f64 / elapsed
                    );
                }
            }

            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                eprintln!("[worker] reconnected to broker");
            }

            Ok(_) => {} // PingResp, SubAck, etc.

            Err(e) => {
                eprintln!("[worker] MQTT connection error: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }

    // El inserter_handle se une implícitamente al terminar el proceso.
    // Para un shutdown limpio se necesitaría un signal handler (tokio-signal),
    // que está fuera del scope del spike.
}
