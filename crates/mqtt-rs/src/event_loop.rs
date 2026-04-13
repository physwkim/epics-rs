use std::collections::HashMap;
use std::time::Duration;

use asyn_rs::port_handle::PortHandle;
use asyn_rs::request::{ParamSetValue, RequestOp};
use asyn_rs::user::AsynUser;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions};
use tokio::sync::mpsc;

use crate::address::{TopicAddress, ValueType};
use crate::config::MqttConfig;
use crate::driver::PublishRequest;
use crate::payload::{DecodedValue, decode_payload};

/// Run the MQTT event loop.
///
/// This task:
/// 1. Connects to the MQTT broker and subscribes to all declared topics
/// 2. Dispatches incoming messages to the param cache via `PortHandle`
/// 3. Publishes outgoing messages from EPICS write operations
pub async fn mqtt_event_loop(
    config: MqttConfig,
    topics: Vec<String>,
    topic_map: HashMap<String, Vec<(usize, TopicAddress)>>,
    port_handle: PortHandle,
    mut publish_rx: mpsc::UnboundedReceiver<PublishRequest>,
    connected_param: usize,
) {
    let mut mqttoptions =
        MqttOptions::new(&config.client_id, &config.broker_host, config.broker_port);
    mqttoptions.set_keep_alive(Duration::from_secs(config.keep_alive_secs));
    mqttoptions.set_clean_session(config.clean_session);

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 256);

    // Initial subscription
    subscribe_all(&client, &topics, config.qos).await;

    loop {
        tokio::select! {
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Incoming::Publish(publish))) => {
                        handle_incoming_message(
                            &publish.topic,
                            &publish.payload,
                            &topic_map,
                            &port_handle,
                        );
                    }
                    Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                        tracing::info!("MQTT connected, subscribing to {} topics", topics.len());
                        port_handle.set_params_and_notify(0, vec![
                            ParamSetValue::Int32 { reason: connected_param, addr: 0, value: 1 },
                        ]);
                        subscribe_all(&client, &topics, config.qos).await;
                    }
                    Err(e) => {
                        tracing::error!("MQTT connection error: {e}");
                        port_handle.set_params_and_notify(0, vec![
                            ParamSetValue::Int32 { reason: connected_param, addr: 0, value: 0 },
                        ]);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    _ => {}
                }
            }
            Some(req) = publish_rx.recv() => {
                let qos: rumqttc::QoS = req.qos.into();
                if let Err(e) = client.publish(&req.topic, qos, req.retained, req.payload.as_bytes()).await {
                    tracing::warn!("MQTT publish to '{}' failed: {e}", req.topic);
                }
            }
        }
    }
}

async fn subscribe_all(client: &AsyncClient, topics: &[String], qos: crate::config::QoS) {
    let rqos: rumqttc::QoS = qos.into();
    for topic in topics {
        if let Err(e) = client.subscribe(topic, rqos).await {
            tracing::warn!("MQTT subscribe to '{topic}' failed: {e}");
        }
    }
}

fn handle_incoming_message(
    topic: &str,
    payload: &[u8],
    topic_map: &HashMap<String, Vec<(usize, TopicAddress)>>,
    port_handle: &PortHandle,
) {
    let payload_str = match std::str::from_utf8(payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Non-UTF8 payload on topic '{topic}': {e}");
            return;
        }
    };

    let subscribers = match topic_map.get(topic) {
        Some(subs) => subs,
        None => return,
    };

    let mut batch_updates = Vec::new();

    for (reason, addr) in subscribers {
        match decode_payload(payload_str, addr) {
            Ok(decoded) => {
                // ParamSetValue supports Int32, Float64, Octet, Float64Array.
                // UInt32Digital and Int32Array need individual writes.
                match decoded {
                    DecodedValue::Int32(v) => {
                        batch_updates.push(ParamSetValue::Int32 {
                            reason: *reason,
                            addr: 0,
                            value: v,
                        });
                    }
                    DecodedValue::Float64(v) => {
                        batch_updates.push(ParamSetValue::Float64 {
                            reason: *reason,
                            addr: 0,
                            value: v,
                        });
                    }
                    DecodedValue::String(v) => {
                        batch_updates.push(ParamSetValue::Octet {
                            reason: *reason,
                            addr: 0,
                            value: v,
                        });
                    }
                    DecodedValue::Float64Array(v) => {
                        batch_updates.push(ParamSetValue::Float64Array {
                            reason: *reason,
                            addr: 0,
                            value: v,
                        });
                    }
                    DecodedValue::UInt32(v) => {
                        // No ParamSetValue::UInt32Digital; use submit_no_wait
                        let user = AsynUser::new(*reason).with_addr(0);
                        port_handle.submit_no_wait(
                            RequestOp::UInt32DigitalWrite {
                                value: v,
                                mask: 0xFFFF_FFFF,
                            },
                            user,
                        );
                    }
                    DecodedValue::Int32Array(_v) => {
                        // No ParamSetValue::Int32Array; log a warning for now.
                        // Int32Array updates would need an asyn-rs extension.
                        tracing::debug!(
                            "Int32Array via set_params_and_notify not yet supported for topic '{topic}'"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    "Failed to decode '{}' on topic '{topic}': {e}",
                    addr.value_type.label(),
                );
            }
        }
    }

    if !batch_updates.is_empty() {
        port_handle.set_params_and_notify(0, batch_updates);
    }
}

impl ValueType {
    fn label(&self) -> &'static str {
        match self {
            ValueType::Int => "INT",
            ValueType::Float => "FLOAT",
            ValueType::Digital => "DIGITAL",
            ValueType::String => "STRING",
            ValueType::IntArray => "INTARRAY",
            ValueType::FloatArray => "FLOATARRAY",
        }
    }
}
