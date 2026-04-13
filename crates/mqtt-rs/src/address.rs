use asyn_rs::param::ParamType;

use crate::error::{MqttError, MqttResult};

/// Payload format: flat single-value or structured JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFormat {
    Flat,
    Json,
}

/// Expected value type of the MQTT payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Int,
    Float,
    Digital,
    String,
    IntArray,
    FloatArray,
}

/// Parsed topic address from a drvInfo string.
///
/// Format: `"FORMAT:TYPE topic/name [json.field.path]"`
///
/// Examples:
/// - `"FLAT:INT test/temperature"`
/// - `"JSON:FLOAT sensors/data humidity.relative"`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicAddress {
    pub format: PayloadFormat,
    pub value_type: ValueType,
    pub topic: String,
    pub json_field: Option<String>,
    /// When true, string values "1"/"on"/"true" → "ON", "0"/"off"/"false" → "OFF".
    /// Set by Z2M builders for state control topics. Generic MQTT leaves this false.
    pub normalize_on_off: bool,
}

impl TopicAddress {
    /// Parse a drvInfo string into a `TopicAddress`.
    ///
    /// Parsing strategy (supports topics with spaces, e.g. Korean device names):
    /// - First whitespace-delimited token: `FORMAT:TYPE`
    /// - **FLAT**: everything after `FORMAT:TYPE ` is the topic
    /// - **JSON**: last whitespace-delimited token is the json field,
    ///   everything between FORMAT:TYPE and the last token is the topic
    pub fn parse(drv_info: &str) -> MqttResult<Self> {
        // Split off FORMAT:TYPE (first token)
        let (format_type_str, rest) =
            drv_info.split_once(char::is_whitespace).ok_or_else(|| {
                MqttError::InvalidAddress(format!(
                    "expected at least 'FORMAT:TYPE topic', got: {drv_info:?}"
                ))
            })?;

        let rest = rest.trim_start();
        if rest.is_empty() {
            return Err(MqttError::InvalidAddress(
                "missing topic after FORMAT:TYPE".into(),
            ));
        }

        let (format, value_type) = Self::parse_format_type(format_type_str)?;

        let (topic, json_field) = match format {
            PayloadFormat::Flat => {
                // Everything remaining is the topic
                (rest.to_string(), None)
            }
            PayloadFormat::Json => {
                // Last token is the json field, everything before is the topic.
                // JSON field is a dot-separated path (no spaces).
                let (topic, field) = rest.rsplit_once(char::is_whitespace).ok_or_else(|| {
                    MqttError::InvalidAddress("JSON format requires 'topic field'".into())
                })?;
                let topic = topic.trim_end().to_string();
                if topic.is_empty() {
                    return Err(MqttError::InvalidAddress(
                        "empty topic before JSON field".into(),
                    ));
                }
                (topic, Some(field.to_string()))
            }
        };

        Self::validate_topic(&topic)?;

        Ok(Self {
            format,
            value_type,
            topic,
            json_field,
            normalize_on_off: false,
        })
    }

    /// Convert this address's value type to the corresponding asyn `ParamType`.
    pub fn param_type(&self) -> ParamType {
        match self.value_type {
            ValueType::Int => ParamType::Int32,
            ValueType::Float => ParamType::Float64,
            ValueType::Digital => ParamType::UInt32Digital,
            ValueType::String => ParamType::Octet,
            ValueType::IntArray => ParamType::Int32Array,
            ValueType::FloatArray => ParamType::Float64Array,
        }
    }

    /// Reconstruct the drvInfo string for use as a parameter name.
    pub fn to_drv_info(&self) -> String {
        let fmt = match self.format {
            PayloadFormat::Flat => "FLAT",
            PayloadFormat::Json => "JSON",
        };
        let typ = match self.value_type {
            ValueType::Int => "INT",
            ValueType::Float => "FLOAT",
            ValueType::Digital => "DIGITAL",
            ValueType::String => "STRING",
            ValueType::IntArray => "INTARRAY",
            ValueType::FloatArray => "FLOATARRAY",
        };
        match &self.json_field {
            Some(field) => format!("{fmt}:{typ} {} {field}", self.topic),
            None => format!("{fmt}:{typ} {}", self.topic),
        }
    }

    fn parse_format_type(s: &str) -> MqttResult<(PayloadFormat, ValueType)> {
        let (fmt_str, type_str) = s
            .split_once(':')
            .ok_or_else(|| MqttError::InvalidAddress(format!("missing ':' in {s:?}")))?;

        let format = match fmt_str.to_ascii_uppercase().as_str() {
            "FLAT" => PayloadFormat::Flat,
            "JSON" => PayloadFormat::Json,
            _ => {
                return Err(MqttError::UnsupportedType(format!(
                    "unknown format: {fmt_str:?}"
                )));
            }
        };

        let value_type = match type_str.to_ascii_uppercase().as_str() {
            "INT" => ValueType::Int,
            "FLOAT" => ValueType::Float,
            "DIGITAL" => ValueType::Digital,
            "STRING" => ValueType::String,
            "INTARRAY" => ValueType::IntArray,
            "FLOATARRAY" => ValueType::FloatArray,
            _ => {
                return Err(MqttError::UnsupportedType(format!(
                    "unknown type: {type_str:?}"
                )));
            }
        };

        Ok((format, value_type))
    }

    fn validate_topic(topic: &str) -> MqttResult<()> {
        if topic.is_empty() {
            return Err(MqttError::InvalidTopic("empty topic".into()));
        }
        if topic.contains('#') || topic.contains('+') {
            return Err(MqttError::InvalidTopic(format!(
                "wildcards not allowed in topic address: {topic:?}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flat_int() {
        let addr = TopicAddress::parse("FLAT:INT test/temperature").unwrap();
        assert_eq!(addr.format, PayloadFormat::Flat);
        assert_eq!(addr.value_type, ValueType::Int);
        assert_eq!(addr.topic, "test/temperature");
        assert_eq!(addr.json_field, None);
        assert_eq!(addr.param_type(), ParamType::Int32);
    }

    #[test]
    fn parse_flat_float() {
        let addr = TopicAddress::parse("FLAT:FLOAT sensors/pressure").unwrap();
        assert_eq!(addr.format, PayloadFormat::Flat);
        assert_eq!(addr.value_type, ValueType::Float);
        assert_eq!(addr.topic, "sensors/pressure");
    }

    #[test]
    fn parse_flat_string() {
        let addr = TopicAddress::parse("FLAT:STRING device/status").unwrap();
        assert_eq!(addr.value_type, ValueType::String);
        assert_eq!(addr.param_type(), ParamType::Octet);
    }

    #[test]
    fn parse_flat_arrays() {
        let addr = TopicAddress::parse("FLAT:INTARRAY data/counts").unwrap();
        assert_eq!(addr.value_type, ValueType::IntArray);
        assert_eq!(addr.param_type(), ParamType::Int32Array);

        let addr = TopicAddress::parse("FLAT:FLOATARRAY data/waveform").unwrap();
        assert_eq!(addr.value_type, ValueType::FloatArray);
        assert_eq!(addr.param_type(), ParamType::Float64Array);
    }

    #[test]
    fn parse_json_float() {
        let addr = TopicAddress::parse("JSON:FLOAT sensors/data humidity").unwrap();
        assert_eq!(addr.format, PayloadFormat::Json);
        assert_eq!(addr.value_type, ValueType::Float);
        assert_eq!(addr.topic, "sensors/data");
        assert_eq!(addr.json_field.as_deref(), Some("humidity"));
    }

    #[test]
    fn parse_json_nested_field() {
        let addr = TopicAddress::parse("JSON:INT sensors/data reading.value").unwrap();
        assert_eq!(addr.json_field.as_deref(), Some("reading.value"));
    }

    #[test]
    fn parse_case_insensitive() {
        let addr = TopicAddress::parse("flat:int test/topic").unwrap();
        assert_eq!(addr.format, PayloadFormat::Flat);
        assert_eq!(addr.value_type, ValueType::Int);
    }

    #[test]
    fn parse_roundtrip() {
        let original = "FLAT:INT test/temperature";
        let addr = TopicAddress::parse(original).unwrap();
        assert_eq!(addr.to_drv_info(), original);

        let original = "JSON:FLOAT sensors/data humidity";
        let addr = TopicAddress::parse(original).unwrap();
        assert_eq!(addr.to_drv_info(), original);
    }

    #[test]
    fn reject_empty_input() {
        assert!(TopicAddress::parse("").is_err());
    }

    #[test]
    fn reject_missing_topic() {
        assert!(TopicAddress::parse("FLAT:INT").is_err());
    }

    #[test]
    fn reject_missing_colon() {
        assert!(TopicAddress::parse("FLATINT test/topic").is_err());
    }

    #[test]
    fn reject_unknown_format() {
        assert!(TopicAddress::parse("XML:INT test/topic").is_err());
    }

    #[test]
    fn reject_unknown_type() {
        assert!(TopicAddress::parse("FLAT:BOOL test/topic").is_err());
    }

    #[test]
    fn reject_wildcard_topics() {
        assert!(TopicAddress::parse("FLAT:INT test/+/data").is_err());
        assert!(TopicAddress::parse("FLAT:INT test/#").is_err());
    }

    #[test]
    fn reject_json_without_field() {
        assert!(TopicAddress::parse("JSON:FLOAT sensors/data").is_err());
    }

    // --- Topics with spaces (e.g. Z2M Korean device names) ---

    #[test]
    fn parse_flat_topic_with_spaces() {
        let addr = TopicAddress::parse("FLAT:FLOAT zigbee2mqtt/living room plug").unwrap();
        assert_eq!(addr.format, PayloadFormat::Flat);
        assert_eq!(addr.value_type, ValueType::Float);
        assert_eq!(addr.topic, "zigbee2mqtt/living room plug");
        assert_eq!(addr.json_field, None);
    }

    #[test]
    fn parse_json_topic_with_spaces() {
        let addr = TopicAddress::parse("JSON:FLOAT zigbee2mqtt/living room plug power").unwrap();
        assert_eq!(addr.format, PayloadFormat::Json);
        assert_eq!(addr.topic, "zigbee2mqtt/living room plug");
        assert_eq!(addr.json_field.as_deref(), Some("power"));
    }

    #[test]
    fn parse_json_topic_with_spaces_nested_field() {
        let addr =
            TopicAddress::parse("JSON:FLOAT zigbee2mqtt/desk light update.installed_version")
                .unwrap();
        assert_eq!(addr.topic, "zigbee2mqtt/desk light");
        assert_eq!(addr.json_field.as_deref(), Some("update.installed_version"));
    }

    #[test]
    fn parse_flat_topic_with_multiple_spaces() {
        let addr = TopicAddress::parse("FLAT:STRING zigbee2mqtt/my cool device name").unwrap();
        assert_eq!(addr.topic, "zigbee2mqtt/my cool device name");
    }

    #[test]
    fn roundtrip_topic_with_spaces() {
        let original = "FLAT:FLOAT zigbee2mqtt/living room plug";
        let addr = TopicAddress::parse(original).unwrap();
        assert_eq!(addr.to_drv_info(), original);

        let original = "JSON:INT zigbee2mqtt/bedroom plug device_temperature";
        let addr = TopicAddress::parse(original).unwrap();
        assert_eq!(addr.to_drv_info(), original);
    }
}
