package mx.rafex.ether.sqlite.spike;

import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.LongAdder;

/**
 * Métricas del worker — thread-safe, zero-allocation en el hot path.
 *
 * <p>Se imprime periódicamente a stderr (no stdout, para no mezclar con datos).
 */
public final class WorkerMetrics {

    private final LongAdder  msgsCommitted  = new LongAdder();
    private final LongAdder  batchCount     = new LongAdder();
    private final LongAdder  errorCount     = new LongAdder();
    private final AtomicLong lastBatchMs    = new AtomicLong(0);
    private final AtomicLong maxBatchMs     = new AtomicLong(0);

    private final long startMs = System.currentTimeMillis();

    /**
     * Registra un batch insertado correctamente.
     *
     * @param count      número de mensajes en el batch
     * @param durationMs duración de la transacción en ms
     */
    public void recordBatch(int count, long durationMs) {
        msgsCommitted.add(count);
        batchCount.increment();
        lastBatchMs.set(durationMs);
        maxBatchMs.accumulateAndGet(durationMs, Math::max);
    }

    /** Registra un batch que falló (ya se hizo ROLLBACK). */
    public void recordError() {
        errorCount.increment();
    }

    /**
     * Imprime un resumen a stderr.
     *
     * @param buffer referencia al buffer para obtener dropped y queue depth
     */
    public void printStats(BatchBuffer buffer) {
        long elapsed  = Math.max(1, (System.currentTimeMillis() - startMs) / 1_000);
        long committed = msgsCommitted.sum();
        long batches  = batchCount.sum();
        long errors   = errorCount.sum();
        long lastMs   = lastBatchMs.get();
        long maxMs    = maxBatchMs.get();
        long dropped  = buffer.dropped();
        int  qDepth   = buffer.size();
        double tps    = (double) committed / elapsed;

        System.err.printf(
            "[metrics] elapsed=%ds committed=%d batches=%d errors=%d dropped=%d" +
            " qDepth=%d tps=%.0f/s last_commit=%dms max_commit=%dms%n",
            elapsed, committed, batches, errors, dropped, qDepth, tps, lastMs, maxMs
        );
    }

    /** Imprime el resumen final con etiqueta de cierre. */
    public void printFinal(BatchBuffer buffer) {
        System.err.println("[metrics] ===== FINAL =====");
        printStats(buffer);
    }

    public long totalCommitted() { return msgsCommitted.sum(); }
    public long elapsedMs()      { return System.currentTimeMillis() - startMs; }
}
