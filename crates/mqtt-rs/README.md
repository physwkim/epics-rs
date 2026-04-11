# mqtt-rs

MQTT driver for [epics-rs](https://github.com/epics-rs/epics-rs) — publish/subscribe MQTT topics as EPICS records.

An asyn PortDriver that bridges MQTT brokers to the EPICS record layer. Incoming MQTT messages update PVs via I/O Intr scanning; EPICS output record writes are published to the broker.

Inspired by [epicsMQTT](https://github.com/epics-modules/mqtt) (C++ MQTT support for EPICS using autoparamDriver + Paho). This is an independent Rust implementation, not a port.

## Features

- **FLAT payloads** — single values: `INT`, `FLOAT`, `DIGITAL`, `STRING`, `INTARRAY`, `FLOATARRAY`
- **JSON payloads** — extract nested fields via dot-path (e.g. `sensor.temperature`)
- **Bidirectional** — input records subscribe, output records publish
- **Auto-reconnect** — rumqttc handles broker reconnection transparently
- **Async event loop** — tokio-based, non-blocking MQTT I/O

## Topic Address Format

Records reference MQTT topics through the asyn drvInfo string:

```
FORMAT:TYPE topic/name [json.field.path]
```

| Field | Values |
|-------|--------|
| FORMAT | `FLAT`, `JSON` |
| TYPE | `INT`, `FLOAT`, `DIGITAL`, `STRING`, `INTARRAY`, `FLOATARRAY` |
| topic | MQTT topic (no wildcards) |
| json.field.path | Dot-separated path for JSON payloads (required for JSON, forbidden for FLAT) |

Examples:
```
FLAT:FLOAT sensors/temperature
FLAT:INT   sensors/counter
FLAT:STRING device/status
JSON:FLOAT sensors/environment humidity
JSON:INT   sensors/data reading.value
```

## Usage

### Database

```
record(ai, "$(P)Temperature") {
    field(DTYP, "asynFloat64")
    field(INP,  "@asyn($(PORT)) FLAT:FLOAT sensors/temperature")
    field(SCAN, "I/O Intr")
    field(EGU,  "degC")
    field(PREC, "2")
}

record(ao, "$(P)Setpoint") {
    field(DTYP, "asynFloat64")
    field(OUT,  "@asyn($(PORT)) FLAT:FLOAT actuators/setpoint")
}

record(ai, "$(P)Humidity") {
    field(DTYP, "asynFloat64")
    field(INP,  "@asyn($(PORT)) JSON:FLOAT sensors/environment humidity")
    field(SCAN, "I/O Intr")
}
```

### Startup Script

Topics must be declared with `mqttAddTopic` before `mqttDriverConfigure`:

```bash
# Register topics
mqttAddTopic("MQTT1", "FLAT:FLOAT sensors/temperature")
mqttAddTopic("MQTT1", "FLAT:FLOAT actuators/setpoint")
mqttAddTopic("MQTT1", "JSON:FLOAT sensors/environment humidity")

# Create driver (connects to broker, subscribes to topics)
mqttDriverConfigure("MQTT1", "mqtt://localhost:1883", "epics-client", 1)

# Load records
dbLoadRecords("db/mqtt.db", "P=TEST:,PORT=MQTT1")

iocInit()
```

### IOC Binary

```rust
use mqtt_rs::ioc::register_mqtt_commands;

let trace = Arc::new(TraceManager::new());
let handle = epics_base_rs::runtime::task::runtime_handle();

let mut app = IocApplication::new();
app = asyn_rs::adapter::register_asyn_device_support(app);
app = register_mqtt_commands(app, handle, trace);

app.startup_script("st.cmd")
    .run(epics_ca_rs::server::run_ca_ioc)
    .await
```

## iocsh Commands

| Command | Arguments | Description |
|---------|-----------|-------------|
| `mqttAddTopic` | `portName drvInfo` | Register a topic before driver creation |
| `mqttDriverConfigure` | `portName brokerUrl clientId [qos]` | Create driver, connect to broker |

**QoS values:** 0 = at most once, 1 = at least once (default), 2 = exactly once

## Supported Record Types

| asyn Interface | FLAT Types | JSON Types | Direction |
|---------------|------------|------------|-----------|
| asynInt32 | `FLAT:INT` | `JSON:INT` | Read / Write |
| asynFloat64 | `FLAT:FLOAT` | `JSON:FLOAT` | Read / Write |
| asynUInt32Digital | `FLAT:DIGITAL` | `JSON:DIGITAL` | Read / Write |
| asynOctet | `FLAT:STRING` | `JSON:STRING` | Read / Write |
| asynInt32Array | `FLAT:INTARRAY` | — | Read / Write |
| asynFloat64Array | `FLAT:FLOATARRAY` | — | Read / Write |

## Dependencies

- [rumqttc](https://crates.io/crates/rumqttc) — async MQTT client
- [serde_json](https://crates.io/crates/serde_json) — JSON parsing
- [asyn-rs](../asyn-rs/) — PortDriver framework

## Example

See [`examples/mqtt-ioc/`](../../examples/mqtt-ioc/) for a complete example IOC.
