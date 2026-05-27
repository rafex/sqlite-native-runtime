/// ether-worker — Worker MQTT → SQLite
///
/// Arquitectura:
///   - Hilo principal (tokio): suscriptor MQTT async, dispatcher a canales.
///   - 3 hilos OS bloqueantes: insertores (HIGH / NORMAL / LOW), cada uno con
///     su propia conexión SQLite en WAL.
///   - Tareas tokio: health publisher, metrics publisher, signal handler.
///
/// Shutdown:
///   SIGTERM o Ctrl-C → cerrar subscriber → drop Channels → insertores flushean
///   y hacen WAL TRUNCATE checkpoint → join → exit 0.
mod config;
mod db;
mod inserter;
mod metrics;
mod schema;
mod topic;

use std::{sync::Arc, time::Instant};

use rumqttc::{
    AsyncClient, Event, MqttOptions, Packet, QoS,
};
use tokio::time::{interval, Duration};

use config::Config;
use inserter::{Channels, IngestMsg};
use metrics::Metrics;
use topic::parse_topic;

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    let metrics = Metrics::new();
    let start = Instant::now();

    eprintln!(
        "[worker] ether-worker v{} arrancando",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!("[worker] config = {:?}", config);

    // ── Arrancar los tres hilos insertores ────────────────────────────────────
    let (channels, inserter_handles) =
        inserter::start(&config, Arc::clone(&metrics));
    let channels = Arc::new(channels);

    // ── Configurar cliente MQTT ───────────────────────────────────────────────
    let (broker_host, broker_port) = config.broker_host_port();
    let mut mqttopts =
        MqttOptions::new(&config.mqtt_client_id, &broker_host, broker_port);
    mqttopts.set_keep_alive(Duration::from_secs(30));
    mqttopts.set_clean_session(false); // sesión persistente → QoS=1 sin pérdida en reconexión
    mqttopts.set_max_packet_size(10 * 1024 * 1024, 10 * 1024 * 1024);

    if let (Some(user), Some(pass)) = (&config.mqtt_username, &config.mqtt_password) {
        mqttopts.set_credentials(user, pass);
    }

    // Cap del canal interno de rumqttc (mensajes pendientes de procesar por el loop)
    let (client, mut eventloop) = AsyncClient::new(mqttopts, 256);

    // ── Suscribir a todos los topics configurados ─────────────────────────────
    let qos = match config.mqtt_qos {
        0 => QoS::AtMostOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtLeastOnce,
    };
    for topic in &config.mqtt_topics {
        client.subscribe(topic, qos).await
            .unwrap_or_else(|e| eprintln!("[worker] subscribe '{}' falló: {e}", topic));
    }
    eprintln!(
        "[worker] MQTT connected — startup={}ms topics={:?}",
        start.elapsed().as_millis(),
        config.mqtt_topics
    );

    // ── Tareas de background ──────────────────────────────────────────────────
    tokio::spawn(metrics_publisher_task(
        client.clone(),
        Arc::clone(&metrics),
        config.worker_id.clone(),
        config.metrics_interval_ms,
        start,
    ));
    tokio::spawn(health_publisher_task(
        client.clone(),
        config.worker_id.clone(),
        config.health_interval_ms,
    ));

    // ── Canal de shutdown ─────────────────────────────────────────────────────
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Signal handler (SIGTERM + SIGINT)
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx.send(());
    });

    // ── Loop principal del event loop MQTT ───────────────────────────────────
    let ch = Arc::clone(&channels);
    let mt = Arc::clone(&metrics);
    let cfg = config.clone();

    loop {
        tokio::select! {
            // Evento MQTT
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        handle_publish(publish, &ch, &mt, &cfg);
                    }
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        eprintln!("[worker] MQTT conectado / reconectado");
                        mt.inc_mqtt_reconnect();
                        // Re-suscribir tras reconexión (clean_session=false debería
                        // restaurar las subscripciones, pero re-suscribir es idempotente)
                    }
                    Ok(_) => {} // ConnAck, PingResp, SubAck, etc. — ignorados
                    Err(e) => {
                        eprintln!("[worker] error en eventloop MQTT: {e}");
                        // rumqttc hace reconexión automática; continuamos el loop
                    }
                }
            }

            // Señal de shutdown recibida
            _ = &mut shutdown_rx => {
                eprintln!("[worker] señal de shutdown recibida — cerrando");
                break;
            }
        }
    }

    // ── Shutdown ordenado ─────────────────────────────────────────────────────
    // 1. Desuscribir del broker
    client.disconnect().await.ok();

    // 2. Métricas finales
    metrics.print_stats(start.elapsed());

    // 3. Cerrar los canales → los insertores detectan Disconnected, flushean y terminan
    drop(channels);

    // 4. Esperar a los insertores (flush + WAL checkpoint)
    inserter_handles.join();

    eprintln!("[worker] shutdown completo");
}

// ── Manejo de mensajes MQTT ───────────────────────────────────────────────────

fn handle_publish(
    publish: rumqttc::Publish,
    channels: &Channels,
    metrics: &Metrics,
    _config: &Config,
) {
    let raw_topic = publish.topic.clone();
    let payload_bytes = publish.payload;

    // Parsear payload como UTF-8
    let payload_str = match std::str::from_utf8(&payload_bytes) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("[worker] payload no es UTF-8 en topic '{raw_topic}' — enviando a DLQ");
            metrics.inc_invalid();
            return;
        }
    };

    // Parsear topic
    let parsed = match parse_topic(&raw_topic) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[worker] topic inválido '{raw_topic}': {e}");
            metrics.inc_invalid();
            // TODO Fase 2: publicar en DLQ si config.dlq_enabled
            return;
        }
    };

    let priority = parsed.priority;
    metrics.inc_received(priority);

    // Extraer campos del payload JSON
    let (id, schema_name, metadata_json) = extract_json_fields(payload_str);

    let received_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let msg = IngestMsg {
        id,
        tenant:        parsed.tenant,
        database_name: parsed.database_name,
        entity:        parsed.entity,
        operation:     parsed.operation.as_str().to_owned(),
        raw_topic,
        priority:      priority.as_str().to_owned(),
        schema_name,
        payload_json:  payload_str.to_owned(),
        metadata_json,
        received_at,
    };

    channels.dispatch(msg, priority, metrics);
}

/// Extrae `id`, `schema` y `metadata` del payload JSON.
/// Si el payload no es JSON válido o no tiene `id`, genera un UUID v4.
fn extract_json_fields(payload: &str) -> (String, Option<String>, Option<String>) {
    match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(v) => {
            let id = v["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let schema_name = v["schema"]
                .as_str()
                .map(str::to_owned);

            let metadata_json = v.get("metadata").map(|m| m.to_string());

            (id, schema_name, metadata_json)
        }
        Err(_) => {
            // Payload no es JSON: generar ID y almacenar raw como texto
            (uuid::Uuid::new_v4().to_string(), None, None)
        }
    }
}

// ── Tareas de background ──────────────────────────────────────────────────────

/// Publica métricas en JSON en `worker/{worker_id}/metrics` cada `interval_ms`.
async fn metrics_publisher_task(
    client: AsyncClient,
    metrics: Arc<Metrics>,
    worker_id: String,
    interval_ms: u64,
    start: Instant,
) {
    let mut ticker = interval(Duration::from_millis(interval_ms));
    ticker.tick().await; // primera tick inmediata al arranque
    loop {
        ticker.tick().await;
        let snapshot = metrics.snapshot();
        let json = snapshot.to_json(&worker_id, start.elapsed());
        metrics.print_stats(start.elapsed());

        let topic = format!("worker/{worker_id}/metrics");
        if let Err(e) = client.publish(&topic, QoS::AtMostOnce, false, json.as_bytes()).await {
            eprintln!("[worker] error publicando métricas: {e}");
        }
    }
}

/// Publica un heartbeat en `worker/{worker_id}/health` cada `interval_ms`.
async fn health_publisher_task(
    client: AsyncClient,
    worker_id: String,
    interval_ms: u64,
) {
    let mut ticker = interval(Duration::from_millis(interval_ms));
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let payload = format!(
            r#"{{"worker_id":"{worker_id}","status":"ok","ts":"{}"}}"#,
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
        );
        let topic = format!("worker/{worker_id}/health");
        if let Err(e) = client.publish(&topic, QoS::AtMostOnce, false, payload.as_bytes()).await {
            eprintln!("[worker] error publicando health: {e}");
        }
    }
}

/// Espera SIGTERM o SIGINT (Ctrl-C) y devuelve.
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        let mut sigint  = signal(SignalKind::interrupt()).expect("SIGINT handler");
        tokio::select! {
            _ = sigterm.recv() => eprintln!("[worker] SIGTERM recibido"),
            _ = sigint.recv()  => eprintln!("[worker] SIGINT recibido"),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.expect("Ctrl-C handler");
        eprintln!("[worker] Ctrl-C recibido");
    }
}
