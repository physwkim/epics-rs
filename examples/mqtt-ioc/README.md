# MQTT IOC Example

Demonstrates the [mqtt-rs](../../crates/mqtt-rs/) driver with Channel Access.

Connects to an MQTT broker and exposes several record types: float, integer,
string, JSON field extraction, and digital bitmask.

## Prerequisites

An MQTT broker on `localhost:1883`:

```bash
# macOS
brew install mosquitto && mosquitto -v

# Docker
docker run -p 1883:1883 eclipse-mosquitto

# Linux
sudo apt install mosquitto && mosquitto -v
```

## Build and Run

```bash
cargo build --release -p mqtt-ioc

./target/release/mqtt_ioc examples/mqtt-ioc/ioc/st.cmd
```

## PV Reference

Default prefix: `TEST:MQTT:`

### Input Records (I/O Intr)

| PV | Type | MQTT Topic | Format |
|----|------|------------|--------|
| `Temperature` | ai | `sensors/temperature` | FLAT:FLOAT |
| `Counter` | longin | `sensors/counter` | FLAT:INT |
| `Status` | stringin | `device/status` | FLAT:STRING |
| `Humidity` | ai | `sensors/environment` | JSON:FLOAT (`humidity`) |
| `Pressure` | ai | `sensors/environment` | JSON:FLOAT (`pressure.value`) |
| `DigitalIn` | mbbiDirect | `sensors/digital` | FLAT:DIGITAL |
| `Setpoint_RBV` | ai | `actuators/setpoint` | FLAT:FLOAT |

### Output Records

| PV | Type | MQTT Topic | Format |
|----|------|------------|--------|
| `Setpoint` | ao | `actuators/setpoint` | FLAT:FLOAT |
| `Command` | stringout | `device/command` | FLAT:STRING |

## Test

```bash
# --- Flat float ---
# Publish temperature, read via CA
mosquitto_pub -t sensors/temperature -m "25.3"
caget TEST:MQTT:Temperature
# Expected: TEST:MQTT:Temperature 25.3

# Write setpoint from EPICS, observe on MQTT
caput TEST:MQTT:Setpoint 22.0
mosquitto_sub -t actuators/setpoint
# Expected: 22

# --- Flat integer ---
mosquitto_pub -t sensors/counter -m "42"
caget TEST:MQTT:Counter
# Expected: TEST:MQTT:Counter 42

# --- Flat string ---
mosquitto_pub -t device/status -m "RUNNING"
caget TEST:MQTT:Status
# Expected: TEST:MQTT:Status RUNNING

caput TEST:MQTT:Command "RESET"
mosquitto_sub -t device/command
# Expected: RESET

# --- JSON ---
mosquitto_pub -t sensors/environment \
  -m '{"humidity": 65.2, "pressure": {"value": 1013.25}}'

caget TEST:MQTT:Humidity
# Expected: TEST:MQTT:Humidity 65.2

caget TEST:MQTT:Pressure
# Expected: TEST:MQTT:Pressure 1013.25

# --- Monitor all updates ---
camonitor TEST:MQTT:Temperature TEST:MQTT:Humidity TEST:MQTT:Counter
```
