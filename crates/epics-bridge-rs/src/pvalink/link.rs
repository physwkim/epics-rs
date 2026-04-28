//! `PvaLink` — a single live PVA link bound to a remote PV.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::mpsc;

use epics_pva_rs::client::PvaClient;
use epics_pva_rs::pvdata::{PvField, PvStructure, ScalarValue};

use super::config::{LinkDirection, PvaLinkConfig};

#[derive(Debug, thiserror::Error)]
pub enum PvaLinkError {
    #[error("PVA error: {0}")]
    Pva(#[from] epics_pva_rs::error::PvaError),
    #[error("link is INP-only, write requested")]
    NotWritable,
    #[error("link is OUT-only, read requested")]
    NotReadable,
    #[error("field {0:?} not found in remote NT structure")]
    FieldNotFound(String),
    #[error("field {0:?} is not a scalar")]
    NotScalar(String),
}

pub type PvaLinkResult<T> = Result<T, PvaLinkError>;

/// A live PVA link.
///
/// Constructed once per record-link instance. For INP links the optional
/// monitor task spawns automatically; for OUT links the link just owns the
/// PvaClient and writes synchronously.
pub struct PvaLink {
    config: PvaLinkConfig,
    client: PvaClient,
    /// Latest received value (INP only — None until first event).
    latest: Arc<Mutex<Option<PvField>>>,
    /// Subscriber channel for record-side notification (INP monitor mode).
    #[allow(dead_code)]
    notify_tx: Option<mpsc::Sender<PvField>>,
}

impl PvaLink {
    /// Open a link against the configured PV.
    ///
    /// For INP+monitor links, this also spawns a background monitor task.
    pub async fn open(config: PvaLinkConfig) -> PvaLinkResult<Self> {
        let client = PvaClient::builder().timeout(Duration::from_secs(5)).build();

        let latest = Arc::new(Mutex::new(None));
        let mut notify_tx = None;

        if matches!(config.direction, LinkDirection::Inp) && config.monitor {
            let (tx, _rx) = mpsc::channel::<PvField>(64);
            notify_tx = Some(tx.clone());

            let pv_name = config.pv_name.clone();
            let latest_clone = latest.clone();
            let client_clone = client.clone();
            tokio::spawn(async move {
                let _ = client_clone
                    .pvmonitor(&pv_name, |value| {
                        *latest_clone.lock() = Some(value.clone());
                        let _ = tx.try_send(value.clone());
                    })
                    .await;
            });
        }

        Ok(Self {
            config,
            client,
            latest,
            notify_tx,
        })
    }

    pub fn config(&self) -> &PvaLinkConfig {
        &self.config
    }

    /// Read the current value of the linked field.
    ///
    /// In monitor mode this returns the cached latest value; otherwise it
    /// triggers a fresh GET.
    pub async fn read(&self) -> PvaLinkResult<PvField> {
        if matches!(self.config.direction, LinkDirection::Out) {
            return Err(PvaLinkError::NotReadable);
        }
        if self.config.monitor
            && let Some(v) = self.latest.lock().clone()
        {
            return Ok(extract_field(&v, &self.config.field));
        }
        let result = self.client.pvget_full(&self.config.pv_name).await?;
        Ok(extract_field(&result.value, &self.config.field))
    }

    /// Synchronous fast-path read: return the cached field if the
    /// monitor has delivered at least one event, without ever
    /// awaiting. Returns `None` for OUT links, non-monitor INPs,
    /// or pre-first-event INPs.
    ///
    /// Lets the record-link resolver path skip `block_on` on every
    /// process — the typical hot case where a monitor has already
    /// populated the cache. Mirrors pvxs `pvalink_lset.cpp::pvaLoadValue`
    /// (sync read of cached `current` slot).
    pub fn try_read_cached(&self) -> Option<PvField> {
        if matches!(self.config.direction, LinkDirection::Out) || !self.config.monitor {
            return None;
        }
        let v = self.latest.lock().clone()?;
        Some(extract_field(&v, &self.config.field))
    }

    /// Convenience: read the value as f64.
    pub async fn read_scalar_f64(&self) -> PvaLinkResult<f64> {
        let pv = self.read().await?;
        scalar_as_f64(&pv).ok_or_else(|| PvaLinkError::NotScalar(self.config.field.clone()))
    }

    /// Write a value to the linked PV (OUT direction only).
    pub async fn write(&self, value_str: &str) -> PvaLinkResult<()> {
        if matches!(self.config.direction, LinkDirection::Inp) {
            return Err(PvaLinkError::NotWritable);
        }
        self.client.pvput(&self.config.pv_name, value_str).await?;
        Ok(())
    }

    /// True when the link's monitor has received at least one update
    /// (i.e., the upstream PV is reachable and has emitted a value).
    /// Mirrors pvxs `pvaIsConnected` (pvalink_lset.cpp:186).
    pub fn is_connected(&self) -> bool {
        self.latest.lock().is_some()
    }

    /// Best-effort alarm message for the linked PV. Returns the
    /// `alarm.message` field of the latest cached NT structure, or
    /// `None` when unavailable. Mirrors pvxs `pvaGetAlarmMsg`
    /// (pvalink_lset.cpp:536) at the message-string level (severity
    /// / status are reported alongside via the standard NT alarm
    /// substructure — surface-able via `latest_value()`).
    pub fn alarm_message(&self) -> Option<String> {
        let v = self.latest.lock().clone()?;
        let PvField::Structure(s) = v else {
            return None;
        };
        let alarm = s.get_field("alarm")?;
        let PvField::Structure(a) = alarm else {
            return None;
        };
        match a.get_field("message")? {
            PvField::Scalar(ScalarValue::String(m)) => Some(m.clone()),
            _ => None,
        }
    }

    /// Latest cached NT value, if any. Returned as the raw [`PvField`]
    /// so callers can pull whichever sub-field they need (alarm,
    /// timeStamp, value, etc.). pvxs `pvaGetTimeStampTag`
    /// (pvalink_lset.cpp:571) lives on top of this.
    pub fn latest_value(&self) -> Option<PvField> {
        self.latest.lock().clone()
    }

    /// Latest `(seconds, nanoseconds)` from the NT timeStamp slot, if
    /// the cached value carries one. Mirrors pvxs
    /// `pvaGetTimeStampTag`.
    pub fn time_stamp(&self) -> Option<(i64, i32)> {
        let v = self.latest.lock().clone()?;
        let PvField::Structure(s) = v else {
            return None;
        };
        let ts = s.get_field("timeStamp")?;
        let PvField::Structure(t) = ts else {
            return None;
        };
        let secs = match t.get_field("secondsPastEpoch")? {
            PvField::Scalar(ScalarValue::Long(v)) => *v,
            PvField::Scalar(ScalarValue::ULong(v)) => *v as i64,
            _ => return None,
        };
        let nsec = match t.get_field("nanoseconds")? {
            PvField::Scalar(ScalarValue::Int(v)) => *v,
            PvField::Scalar(ScalarValue::UInt(v)) => *v as i32,
            _ => return None,
        };
        Some((secs, nsec))
    }
}

/// Walk a dotted field path through a [`PvField`] and return the leaf value.
fn extract_field(root: &PvField, path: &str) -> PvField {
    if path.is_empty() {
        return root.clone();
    }
    let mut cursor = root.clone();
    for segment in path.split('.') {
        cursor = match cursor {
            PvField::Structure(s) => s.get_field(segment).cloned().unwrap_or(PvField::Null),
            other => return other,
        };
    }
    cursor
}

fn scalar_as_f64(field: &PvField) -> Option<f64> {
    match field {
        PvField::Scalar(sv) => Some(scalar_value_to_f64(sv)),
        PvField::Structure(s) => s.get_value().map(scalar_value_to_f64),
        _ => None,
    }
}

fn scalar_value_to_f64(v: &ScalarValue) -> f64 {
    match v {
        ScalarValue::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        ScalarValue::Byte(x) => *x as f64,
        ScalarValue::UByte(x) => *x as f64,
        ScalarValue::Short(x) => *x as f64,
        ScalarValue::UShort(x) => *x as f64,
        ScalarValue::Int(x) => *x as f64,
        ScalarValue::UInt(x) => *x as f64,
        ScalarValue::Long(x) => *x as f64,
        ScalarValue::ULong(x) => *x as f64,
        ScalarValue::Float(x) => *x as f64,
        ScalarValue::Double(x) => *x,
        ScalarValue::String(s) => s.parse().unwrap_or(0.0),
    }
}

// Suppress unused warning for fields used only via accessors.
#[allow(dead_code)]
fn _suppress(_: &PvStructure) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_top_level_value() {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Double(1.5))));
        let root = PvField::Structure(s);
        let v = extract_field(&root, "value");
        match v {
            PvField::Scalar(ScalarValue::Double(d)) => assert_eq!(d, 1.5),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn extract_nested_field() {
        let mut alarm = PvStructure::new("alarm_t");
        alarm
            .fields
            .push(("severity".into(), PvField::Scalar(ScalarValue::Int(2))));
        let mut root = PvStructure::new("epics:nt/NTScalar:1.0");
        root.fields
            .push(("alarm".into(), PvField::Structure(alarm)));
        let value = extract_field(&PvField::Structure(root), "alarm.severity");
        assert!(matches!(value, PvField::Scalar(ScalarValue::Int(2))));
    }

    #[test]
    fn missing_field_returns_null() {
        let s = PvStructure::new("epics:nt/NTScalar:1.0");
        let v = extract_field(&PvField::Structure(s), "nope");
        assert!(matches!(v, PvField::Null));
    }
}
