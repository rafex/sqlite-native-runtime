/// Contadores de métricas del worker — zero-allocation en el hot path.
///
/// Usa `AtomicU64` para actualizaciones sin lock desde el hilo del subscriber MQTT
/// y desde los hilos insertores simultáneamente.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::topic::Priority;

#[derive(Debug, Default)]
pub struct Metrics {
    // ── Mensajes recibidos del broker ─────────────────────────────────────────
    pub received_high:   AtomicU64,
    pub received_normal: AtomicU64,
    pub received_low:    AtomicU64,
    /// Mensajes cuyo topic no pudo parsearse (enviados a DLQ o descartados)
    pub received_invalid: AtomicU64,

    // ── Mensajes committed a SQLite ───────────────────────────────────────────
    pub committed_high:   AtomicU64,
    pub committed_normal: AtomicU64,
    pub committed_low:    AtomicU64,

    // ── Mensajes descartados (canal lleno) ────────────────────────────────────
    pub dropped_high:   AtomicU64,
    pub dropped_normal: AtomicU64,
    pub dropped_low:    AtomicU64,

    // ── Errores de inserción ──────────────────────────────────────────────────
    pub errors_high:   AtomicU64,
    pub errors_normal: AtomicU64,
    pub errors_low:    AtomicU64,

    // ── DLQ ──────────────────────────────────────────────────────────────────
    pub dlq_sent: AtomicU64,

    // ── Reconexiones MQTT ─────────────────────────────────────────────────────
    pub mqtt_reconnects: AtomicU64,

    // ── Batches ───────────────────────────────────────────────────────────────
    pub batches_high:   AtomicU64,
    pub batches_normal: AtomicU64,
    pub batches_low:    AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    // ── Incrementos desde el subscriber (hot path) ────────────────────────────

    pub fn inc_received(&self, priority: Priority) {
        match priority {
            Priority::High   => self.received_high.fetch_add(1, Ordering::Relaxed),
            Priority::Normal => self.received_normal.fetch_add(1, Ordering::Relaxed),
            Priority::Low    => self.received_low.fetch_add(1, Ordering::Relaxed),
        };
    }

    pub fn inc_invalid(&self) {
        self.received_invalid.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_dropped(&self, priority: Priority) {
        match priority {
            Priority::High   => self.dropped_high.fetch_add(1, Ordering::Relaxed),
            Priority::Normal => self.dropped_normal.fetch_add(1, Ordering::Relaxed),
            Priority::Low    => self.dropped_low.fetch_add(1, Ordering::Relaxed),
        };
    }

    // ── Incrementos desde los insertores ──────────────────────────────────────

    pub fn add_committed(&self, priority: Priority, n: u64) {
        match priority {
            Priority::High   => self.committed_high.fetch_add(n, Ordering::Relaxed),
            Priority::Normal => self.committed_normal.fetch_add(n, Ordering::Relaxed),
            Priority::Low    => self.committed_low.fetch_add(n, Ordering::Relaxed),
        };
    }

    pub fn inc_error(&self, priority: Priority) {
        match priority {
            Priority::High   => self.errors_high.fetch_add(1, Ordering::Relaxed),
            Priority::Normal => self.errors_normal.fetch_add(1, Ordering::Relaxed),
            Priority::Low    => self.errors_low.fetch_add(1, Ordering::Relaxed),
        };
    }

    pub fn inc_batch(&self, priority: Priority) {
        match priority {
            Priority::High   => self.batches_high.fetch_add(1, Ordering::Relaxed),
            Priority::Normal => self.batches_normal.fetch_add(1, Ordering::Relaxed),
            Priority::Low    => self.batches_low.fetch_add(1, Ordering::Relaxed),
        };
    }

    pub fn inc_dlq_sent(&self) {
        self.dlq_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_mqtt_reconnect(&self) {
        self.mqtt_reconnects.fetch_add(1, Ordering::Relaxed);
    }

    // ── Snapshot ──────────────────────────────────────────────────────────────

    /// Snapshot atómico de los contadores actuales.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            received_high:   self.received_high.load(Ordering::Relaxed),
            received_normal: self.received_normal.load(Ordering::Relaxed),
            received_low:    self.received_low.load(Ordering::Relaxed),
            received_invalid: self.received_invalid.load(Ordering::Relaxed),
            committed_high:  self.committed_high.load(Ordering::Relaxed),
            committed_normal: self.committed_normal.load(Ordering::Relaxed),
            committed_low:   self.committed_low.load(Ordering::Relaxed),
            dropped_high:    self.dropped_high.load(Ordering::Relaxed),
            dropped_normal:  self.dropped_normal.load(Ordering::Relaxed),
            dropped_low:     self.dropped_low.load(Ordering::Relaxed),
            errors_high:     self.errors_high.load(Ordering::Relaxed),
            errors_normal:   self.errors_normal.load(Ordering::Relaxed),
            errors_low:      self.errors_low.load(Ordering::Relaxed),
            dlq_sent:        self.dlq_sent.load(Ordering::Relaxed),
            mqtt_reconnects: self.mqtt_reconnects.load(Ordering::Relaxed),
            batches_high:    self.batches_high.load(Ordering::Relaxed),
            batches_normal:  self.batches_normal.load(Ordering::Relaxed),
            batches_low:     self.batches_low.load(Ordering::Relaxed),
        }
    }

    /// Imprime las métricas a stderr con formato legible.
    pub fn print_stats(&self, elapsed: Duration) {
        let s = self.snapshot();
        let secs = elapsed.as_secs_f64().max(0.001);
        let total_received  = s.received_high + s.received_normal + s.received_low;
        let total_committed = s.committed_high + s.committed_normal + s.committed_low;
        let total_dropped   = s.dropped_high + s.dropped_normal + s.dropped_low;
        let tps = total_committed as f64 / secs;

        eprintln!(
            "[metrics] elapsed={:.0}s  received={} committed={} dropped={} errors={} \
             invalid={} dlq={} tps={:.0}/s \
             | high: recv={} commit={} drop={}  \
             | normal: recv={} commit={} drop={}  \
             | low: recv={} commit={} drop={}",
            secs,
            total_received, total_committed, total_dropped,
            s.errors_high + s.errors_normal + s.errors_low,
            s.received_invalid, s.dlq_sent, tps,
            s.received_high,  s.committed_high,  s.dropped_high,
            s.received_normal, s.committed_normal, s.dropped_normal,
            s.received_low,   s.committed_low,   s.dropped_low,
        );
    }
}

/// Snapshot inmutable de los contadores en un instante de tiempo.
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub received_high:   u64,
    pub received_normal: u64,
    pub received_low:    u64,
    pub received_invalid: u64,
    pub committed_high:  u64,
    pub committed_normal: u64,
    pub committed_low:   u64,
    pub dropped_high:    u64,
    pub dropped_normal:  u64,
    pub dropped_low:     u64,
    pub errors_high:     u64,
    pub errors_normal:   u64,
    pub errors_low:      u64,
    pub dlq_sent:        u64,
    pub mqtt_reconnects: u64,
    pub batches_high:    u64,
    pub batches_normal:  u64,
    pub batches_low:     u64,
}

impl MetricsSnapshot {
    pub fn total_received(&self)  -> u64 { self.received_high + self.received_normal + self.received_low }
    pub fn total_committed(&self) -> u64 { self.committed_high + self.committed_normal + self.committed_low }
    pub fn total_dropped(&self)   -> u64 { self.dropped_high + self.dropped_normal + self.dropped_low }
    pub fn total_errors(&self)    -> u64 { self.errors_high + self.errors_normal + self.errors_low }

    /// Serializa a JSON para publicar en el topic de métricas MQTT.
    pub fn to_json(&self, worker_id: &str, uptime: Duration) -> String {
        let total_committed = self.total_committed();
        let secs = uptime.as_secs_f64().max(0.001);
        format!(
            r#"{{"worker_id":"{worker_id}","uptime_s":{:.0},"tps":{:.1},\
"received":{{"high":{},"normal":{},"low":{},"invalid":{}}},\
"committed":{{"high":{},"normal":{},"low":{}}},\
"dropped":{{"high":{},"normal":{},"low":{}}},\
"errors":{{"high":{},"normal":{},"low":{}}},\
"batches":{{"high":{},"normal":{},"low":{}}},\
"dlq_sent":{},"mqtt_reconnects":{}}}"#,
            secs,
            total_committed as f64 / secs,
            self.received_high, self.received_normal, self.received_low, self.received_invalid,
            self.committed_high, self.committed_normal, self.committed_low,
            self.dropped_high, self.dropped_normal, self.dropped_low,
            self.errors_high, self.errors_normal, self.errors_low,
            self.batches_high, self.batches_normal, self.batches_low,
            self.dlq_sent, self.mqtt_reconnects,
        )
    }
}

// ── Printer periódico (hilo de métricas) ──────────────────────────────────────

/// Handle del hilo que imprime métricas periódicamente a stderr.
pub struct MetricsPrinter {
    start: Instant,
    metrics: Arc<Metrics>,
}

impl MetricsPrinter {
    pub fn new(metrics: Arc<Metrics>) -> Self {
        Self { start: Instant::now(), metrics }
    }

    pub fn print_now(&self) {
        self.metrics.print_stats(self.start.elapsed());
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inc_received_all_priorities() {
        let m = Metrics::new();
        m.inc_received(Priority::High);
        m.inc_received(Priority::Normal);
        m.inc_received(Priority::Normal);
        m.inc_received(Priority::Low);
        let s = m.snapshot();
        assert_eq!(s.received_high,   1);
        assert_eq!(s.received_normal, 2);
        assert_eq!(s.received_low,    1);
        assert_eq!(s.total_received(), 4);
    }

    #[test]
    fn add_committed_accumulates() {
        let m = Metrics::new();
        m.add_committed(Priority::Normal, 500);
        m.add_committed(Priority::Normal, 300);
        m.add_committed(Priority::High,   100);
        let s = m.snapshot();
        assert_eq!(s.committed_normal, 800);
        assert_eq!(s.committed_high,   100);
        assert_eq!(s.total_committed(), 900);
    }

    #[test]
    fn snapshot_to_json_contains_worker_id() {
        let m = Metrics::new();
        let s = m.snapshot();
        let json = s.to_json("worker-test-1", Duration::from_secs(60));
        assert!(json.contains("worker-test-1"));
        assert!(json.contains("tps"));
    }
}
