package mx.rafex.ether.sqlite.spike;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * Buffer de mensajes MQTT en memoria entre el subscriber y el inserter.
 *
 * <p>Diseño: {@link LinkedBlockingQueue} con capacidad máxima de {@code batchSize * 20}.
 * Si el inserter no puede mantener el ritmo, {@link #offer} descarta el mensaje y
 * el contador de dropped se incrementa (visible en métricas).
 */
public final class BatchBuffer {

    /**
     * Payload de un mensaje MQTT listo para insertar.
     *
     * @param topic      topic MQTT
     * @param payload    cuerpo del mensaje (texto UTF-8 o JSON)
     * @param receivedMs epoch ms en que el subscriber lo recibió
     */
    public record MqttPayload(String topic, String payload, long receivedMs) {}

    private final LinkedBlockingQueue<MqttPayload> queue;
    private final int    batchSize;
    private final long   flushMs;

    // contadores atómicos para métricas
    private volatile long totalReceived = 0;
    private volatile long totalDropped  = 0;

    public BatchBuffer(int batchSize, long flushMs) {
        this.batchSize = batchSize;
        this.flushMs   = flushMs;
        // capacidad = 20× batch; si se llena, empezamos a descartar
        this.queue = new LinkedBlockingQueue<>(batchSize * 20);
    }

    /**
     * Encola un mensaje. Si la cola está llena lo descarta (nunca bloquea).
     * Llamado desde el callback MQTT (virtual thread o netty thread).
     */
    public void offer(MqttPayload msg) {
        totalReceived++;
        if (!queue.offer(msg)) {
            totalDropped++;
        }
    }

    /**
     * Espera hasta {@code flushMs} por al menos un mensaje, luego drena
     * hasta {@code batchSize} mensajes. Devuelve lista vacía si timeout.
     * Llamado exclusivamente desde el virtual thread del inserter.
     */
    public List<MqttPayload> drain() throws InterruptedException {
        var batch = new ArrayList<MqttPayload>(batchSize);

        // Bloquea hasta el primer mensaje o hasta timeout
        MqttPayload first = queue.poll(flushMs, TimeUnit.MILLISECONDS);
        if (first == null) {
            return batch;  // timeout — lista vacía
        }
        batch.add(first);

        // Drena sin espera hasta completar el batch
        queue.drainTo(batch, batchSize - 1);
        return batch;
    }

    public int  size()         { return queue.size(); }
    public long received()     { return totalReceived; }
    public long dropped()      { return totalDropped; }
}
