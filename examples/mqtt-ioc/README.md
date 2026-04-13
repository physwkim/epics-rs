# MQTT IOC Example

Demonstrates the [mqtt-rs](../../crates/mqtt-rs/) driver with Channel Access.

Two usage modes:
- **Z2M builders** (`st.cmd`) — auto-create records for Zigbee2MQTT devices
- **Generic MQTT** (`db/mqtt.db`) — manual records for any MQTT broker/topic

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
cargo build --release -p mqtt-ioc --features ioc

./target/release/mqtt_ioc examples/mqtt-ioc/ioc/st.cmd
```

## Z2M Builder Example

The default `st.cmd` uses Z2M device type builders. Each line registers MQTT topics and creates EPICS records automatically:

```bash
mqttZ2mPlug("MQTT1",       "TEST:MQTT:", "LR:Plug",  "zigbee2mqtt/living room plug")
mqttZ2mTempSensor("MQTT1", "TEST:MQTT:", "LR:Sens",  "zigbee2mqtt/living room sensor")
mqttZ2mLight("MQTT1",      "TEST:MQTT:", "BR1:Desk", "zigbee2mqtt/desk light")
mqttDriverConfigure("MQTT1", "mqtt://localhost:1883", "epics-mqtt-ioc", 1, "TEST:MQTT:Connected")
iocInit()
```

### Test Z2M devices

```bash
# Monitor connection status
caget TEST:MQTT:Connected

# Read sensor values
caget TEST:MQTT:LR:Sens:Temp
caget TEST:MQTT:LR:Sens:Hum

# Control plug (accepts ON/OFF/1/0/on/off/true/false)
caput TEST:MQTT:LR:Plug:SetState ON
caput TEST:MQTT:LR:Plug:SetState 0

# Control light brightness
caput TEST:MQTT:BR1:Desk:SetBright 128
caput TEST:MQTT:BR1:Desk:SetState OFF

# Monitor power usage
camonitor TEST:MQTT:LR:Plug:Power TEST:MQTT:BR1:Plug:Power
```

## Generic MQTT Example

For non-Z2M topics, use `mqttAddTopic` + `dbLoadRecords` with a `.db` file. See `db/mqtt.db` for examples.

```bash
# Register topics
mqttAddTopic("MQTT1", "FLAT:FLOAT sensors/temperature")
mqttAddTopic("MQTT1", "JSON:FLOAT sensors/environment humidity")

# Create driver
mqttDriverConfigure("MQTT1", "mqtt://localhost:1883", "epics-client", 1)

# Load records from .db file
dbLoadRecords("db/mqtt.db", "P=TEST:,R=MQTT:,PORT=MQTT1")

iocInit()
```

### Test generic MQTT

```bash
# Publish a value, read via CA
mosquitto_pub -t sensors/temperature -m "25.3"
caget TEST:MQTT:Example:Temperature

# Publish JSON, read extracted field
mosquitto_pub -t sensors/environment -m '{"humidity": 65.2}'
caget TEST:MQTT:Example:Humidity

# Monitor updates
camonitor TEST:MQTT:Example:Temperature TEST:MQTT:Example:Humidity
```
