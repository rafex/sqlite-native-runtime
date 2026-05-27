#!/usr/bin/env python3
"""
publisher.py — publica N mensajes MQTT para el benchmark del spike.

Uso:
    python3 publisher.py [COUNT] [BATCH_QOS]

Variables de entorno:
    MQTT_BROKER     host del broker  (default: localhost)
    MQTT_PORT       puerto           (default: 1883)
    MQTT_PUB_TOPIC  topic base       (default: benchmark/sensor)
    MQTT_PUB_QOS    QoS              (default: 1)
    PAYLOAD_SIZE    bytes aprox. del payload JSON (default: 120)

El publisher imprime a stdout (separado del worker que imprime a stderr):
    published=N elapsed=X.XXs tps=YYY/s
"""

import json
import math
import os
import sys
import time

try:
    import paho.mqtt.client as mqtt
except ImportError:
    print("ERROR: instala paho-mqtt:  pip install paho-mqtt", file=sys.stderr)
    sys.exit(1)

# ── Config ────────────────────────────────────────────────────────────────────

COUNT        = int(sys.argv[1]) if len(sys.argv) > 1 else 10_000
BROKER       = os.getenv("MQTT_BROKER",    "localhost")
PORT         = int(os.getenv("MQTT_PORT",  "1883"))
TOPIC_BASE   = os.getenv("MQTT_PUB_TOPIC", "benchmark/sensor")
QOS          = int(os.getenv("MQTT_PUB_QOS", "1"))
PAYLOAD_SIZE = int(os.getenv("PAYLOAD_SIZE", "120"))

# Número de sensores simulados (varía el topic para que el índice se ejercite)
NUM_SENSORS = 10

# ── Payload realista ─────────────────────────────────────────────────────────
# Rellena con padding hasta PAYLOAD_SIZE para simular payloads reales de IoT.

def make_payload(seq: int, sensor_id: str, ts_ms: int) -> str:
    base = {
        "sensor_id":   sensor_id,
        "seq":         seq,
        "ts":          ts_ms,
        "temperature": round(20.0 + math.sin(seq * 0.01) * 5, 2),
        "humidity":    round(60.0 + math.cos(seq * 0.01) * 10, 2),
        "pressure":    1013.25,
        "location":    "sala-a",
    }
    payload = json.dumps(base, separators=(',', ':'))
    # padding para alcanzar el tamaño objetivo
    if len(payload) < PAYLOAD_SIZE:
        pad = "x" * (PAYLOAD_SIZE - len(payload) - 6)
        base["_pad"] = pad
        payload = json.dumps(base, separators=(',', ':'))
    return payload

# ── Cliente MQTT ─────────────────────────────────────────────────────────────

published = 0
confirmed = 0

def on_publish(client, userdata, mid, reason_code=None, properties=None):
    global confirmed
    confirmed += 1

client = mqtt.Client(
    mqtt.CallbackAPIVersion.VERSION2,
    client_id=f"snr-publisher-{os.getpid()}",
)
client.on_publish = on_publish
client.connect(BROKER, PORT, keepalive=60)
client.loop_start()

# ── Publicar ─────────────────────────────────────────────────────────────────

print(f"[publisher] broker={BROKER}:{PORT} topic={TOPIC_BASE}/T-* count={COUNT} qos={QOS}",
      flush=True)

start = time.monotonic()

for i in range(COUNT):
    sensor_id = f"T-{i % NUM_SENSORS:02d}"
    topic     = f"{TOPIC_BASE}/{sensor_id}"
    ts_ms     = int(time.time() * 1000)
    payload   = make_payload(i, sensor_id, ts_ms)

    info = client.publish(topic, payload, qos=QOS)
    published += 1

    # progreso cada 10k
    if published % 10_000 == 0:
        elapsed = time.monotonic() - start
        print(f"[publisher] published={published} tps={published/elapsed:.0f}/s", flush=True)

# Espera confirmación de todos los mensajes QoS>=1
if QOS >= 1:
    deadline = time.monotonic() + 10
    while confirmed < published and time.monotonic() < deadline:
        time.sleep(0.05)

client.loop_stop()
client.disconnect()

elapsed = time.monotonic() - start
print(
    f"[publisher] DONE published={published} confirmed={confirmed} "
    f"elapsed={elapsed:.2f}s tps={published/elapsed:.0f}/s",
    flush=True,
)
