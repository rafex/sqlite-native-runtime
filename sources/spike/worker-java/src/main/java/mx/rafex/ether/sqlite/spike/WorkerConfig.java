package mx.rafex.ether.sqlite.spike;

import java.util.Arrays;

/**
 * Configuración del worker — leída de variables de entorno.
 *
 * <p>Variables soportadas:
 * <pre>
 *   MQTT_BROKER       URL del broker  (default: tcp://localhost:1883)
 *   MQTT_CLIENT_ID    ID del cliente  (default: snr-spike-java-{pid})
 *   MQTT_TOPICS       Topics separados por coma  (default: benchmark/#)
 *   MQTT_QOS          QoS: 0, 1 o 2  (default: 1)
 *   SQLITE_DB         Ruta del archivo SQLite  (default: /tmp/snr-spike-java.db)
 *   BATCH_SIZE        Máximo mensajes por transacción  (default: 500)
 *   FLUSH_MS          Máximo ms de espera antes de flush  (default: 200)
 *   ETHER_SQLITE_JNI_LIB  Ruta a libether_sqlite_jni_runtime.so (auto-detectada si no se pone)
 * </pre>
 */
public record WorkerConfig(
        String  mqttBroker,
        String  brokerHost,
        int     brokerPort,
        String  clientId,
        String[] topics,
        int     qos,
        String  sqliteDb,
        int     batchSize,
        long    flushMs
) {

    public static WorkerConfig fromEnv() {
        String broker   = env("MQTT_BROKER",    "tcp://localhost:1883");
        String clientId = env("MQTT_CLIENT_ID", "snr-spike-java-" + ProcessHandle.current().pid());
        String topicsRaw = env("MQTT_TOPICS",   "benchmark/#");
        int    qos      = Integer.parseInt(env("MQTT_QOS",     "1"));
        String db       = env("SQLITE_DB",       "/tmp/snr-spike-java.db");
        int    batch    = Integer.parseInt(env("BATCH_SIZE",   "500"));
        long   flush    = Long.parseLong(env("FLUSH_MS",       "200"));

        String[] topics = Arrays.stream(topicsRaw.split(","))
                .map(String::trim)
                .filter(s -> !s.isBlank())
                .toArray(String[]::new);

        String[] hostPort = parseBroker(broker);

        return new WorkerConfig(broker, hostPort[0], Integer.parseInt(hostPort[1]),
                clientId, topics, qos, db, batch, flush);
    }

    /** Parsea "tcp://host:port" → ["host", "port"]. */
    private static String[] parseBroker(String broker) {
        String stripped = broker
                .replaceFirst("^tcp://",  "")
                .replaceFirst("^mqtt://", "")
                .replaceFirst("^ssl://",  "");
        int colon = stripped.lastIndexOf(':');
        if (colon > 0) {
            return new String[]{ stripped.substring(0, colon), stripped.substring(colon + 1) };
        }
        return new String[]{ stripped, "1883" };
    }

    private static String env(String key, String defaultVal) {
        String v = System.getenv(key);
        return (v != null && !v.isBlank()) ? v.trim() : defaultVal;
    }

    @Override
    public String toString() {
        return "WorkerConfig{broker='%s', clientId='%s', topics=%s, qos=%d, db='%s', batch=%d, flush=%dms}"
                .formatted(mqttBroker, clientId, Arrays.toString(topics), qos, sqliteDb, batchSize, flushMs);
    }
}
