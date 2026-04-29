//! Type-safe Normative Types runtime.
//!
//! [`TypedNT`] is the bridge between user-defined Rust structs and
//! the wire-level [`PvField`] / [`FieldDesc`] representation. End
//! users typically don't implement this trait by hand — the
//! `#[derive(NTScalar)]` proc-macro from `epics-macros-rs` generates
//! it from a struct definition like:
//!
//! ```ignore
//! #[derive(NTScalar)]
//! struct MotorPos {
//!     value: f64,
//!     #[nt(meta)] alarm: Alarm,
//!     #[nt(meta)] timestamp: TimeStamp,
//! }
//! ```
//!
//! ## Why this exists
//!
//! Without it, every `pvget` consumer has to walk the `PvField` tree
//! and pattern-match every leaf. With it, a `pvget_typed::<MotorPos>`
//! returns the struct directly — the wire ↔ struct mapping is fixed
//! at compile time, so a missing field or type mismatch surfaces as
//! a Rust type error or a [`TypedNTError`] at the boundary.
//!
//! Mirrors the role pvxs's `Value::as<T>()` plays in C++, but with
//! Rust's stricter type system enforcing field presence and shape
//! at the trait-bound level.
//!
//! ## Manual implementation
//!
//! Implementing this trait manually is supported when the derive
//! doesn't cover an exotic shape. Provide `descriptor()`,
//! `to_pv_field(&self)`, and `from_pv_field(&PvField)`. The default
//! [`Alarm`] / [`TimeStamp`] meta types are re-exported here for
//! convenience — `#[nt(meta)] alarm: Alarm` is the canonical NT
//! shape.

use crate::pvdata::{FieldDesc, PvField, PvStructure, ScalarValue};

/// Errors surfaced at the typed/untyped boundary.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TypedNTError {
    /// `from_pv_field` got a wrapper that didn't match the expected
    /// struct id (e.g. expecting `epics:nt/NTScalar:1.0` and seeing
    /// `epics:nt/NTTable:1.0`).
    #[error("wrong NT struct id: expected {expected:?}, got {got:?}")]
    WrongStructId { expected: String, got: String },
    /// A field declared in the descriptor was missing on the wire.
    #[error("missing field '{0}'")]
    MissingField(String),
    /// A field's wire shape didn't match the Rust type (e.g. expected
    /// `f64`, got `String`).
    #[error("wrong type for field '{field}': {detail}")]
    WrongType { field: String, detail: String },
}

/// A Rust type with a declared NT shape. Implemented automatically
/// by `#[derive(NTScalar)]` and friends.
pub trait TypedNT: Sized + Send + 'static {
    /// Wire-level descriptor (returned to clients on INIT, consulted
    /// on encode). Must be deterministic — every call returns an
    /// identical [`FieldDesc`] so type-cache references resolve
    /// across calls.
    fn descriptor() -> FieldDesc;

    /// Encode this value into a wire [`PvField`].
    fn to_pv_field(&self) -> PvField;

    /// Decode a wire [`PvField`] into the Rust type. Returns
    /// [`TypedNTError`] on mismatch — caller propagates as
    /// [`crate::error::PvaError::InvalidValue`] or similar.
    fn from_pv_field(field: &PvField) -> Result<Self, TypedNTError>;
}

/// Standard `alarm_t` meta sub-structure used by every NT shape.
/// Carry this in your `#[derive(NTScalar)]` struct via
/// `#[nt(meta)] alarm: Alarm`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Alarm {
    pub severity: i32,
    pub status: i32,
    pub message: String,
}

impl TypedNT for Alarm {
    fn descriptor() -> FieldDesc {
        Self::alarm_descriptor()
    }
    fn to_pv_field(&self) -> PvField {
        self.alarm_to_pv_field()
    }
    fn from_pv_field(field: &PvField) -> Result<Self, TypedNTError> {
        Self::alarm_from_pv_field(field)
    }
}

impl Alarm {
    /// Wire descriptor — same as [`crate::nt::meta::alarm_desc`].
    /// Inherent name; the `TypedNT::descriptor()` impl forwards to
    /// this so users can call it without bringing the trait into
    /// scope.
    pub fn alarm_descriptor() -> FieldDesc {
        crate::nt::meta::alarm_desc()
    }

    pub fn alarm_to_pv_field(&self) -> PvField {
        let mut s = PvStructure::new("alarm_t");
        s.fields
            .push(("severity".into(), PvField::Scalar(ScalarValue::Int(self.severity))));
        s.fields
            .push(("status".into(), PvField::Scalar(ScalarValue::Int(self.status))));
        s.fields.push((
            "message".into(),
            PvField::Scalar(ScalarValue::String(self.message.clone())),
        ));
        PvField::Structure(s)
    }

    pub fn alarm_from_pv_field(field: &PvField) -> Result<Self, TypedNTError> {
        let s = match field {
            PvField::Structure(s) => s,
            _ => {
                return Err(TypedNTError::WrongType {
                    field: "alarm".into(),
                    detail: "expected structure".into(),
                });
            }
        };
        Ok(Self {
            severity: get_i32(s, "severity").unwrap_or(0),
            status: get_i32(s, "status").unwrap_or(0),
            message: get_str(s, "message").unwrap_or_default(),
        })
    }
}

/// Standard `time_t` meta sub-structure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeStamp {
    pub seconds_past_epoch: i64,
    pub nanoseconds: i32,
    pub user_tag: i32,
}

impl TypedNT for TimeStamp {
    fn descriptor() -> FieldDesc {
        Self::ts_descriptor()
    }
    fn to_pv_field(&self) -> PvField {
        self.ts_to_pv_field()
    }
    fn from_pv_field(field: &PvField) -> Result<Self, TypedNTError> {
        Self::ts_from_pv_field(field)
    }
}

impl TimeStamp {
    pub fn ts_descriptor() -> FieldDesc {
        crate::nt::meta::time_desc()
    }

    pub fn ts_to_pv_field(&self) -> PvField {
        let mut s = PvStructure::new("time_t");
        s.fields.push((
            "secondsPastEpoch".into(),
            PvField::Scalar(ScalarValue::Long(self.seconds_past_epoch)),
        ));
        s.fields.push((
            "nanoseconds".into(),
            PvField::Scalar(ScalarValue::Int(self.nanoseconds)),
        ));
        s.fields
            .push(("userTag".into(), PvField::Scalar(ScalarValue::Int(self.user_tag))));
        PvField::Structure(s)
    }

    pub fn ts_from_pv_field(field: &PvField) -> Result<Self, TypedNTError> {
        let s = match field {
            PvField::Structure(s) => s,
            _ => {
                return Err(TypedNTError::WrongType {
                    field: "timestamp".into(),
                    detail: "expected structure".into(),
                });
            }
        };
        Ok(Self {
            seconds_past_epoch: get_i64(s, "secondsPastEpoch").unwrap_or(0),
            nanoseconds: get_i32(s, "nanoseconds").unwrap_or(0),
            user_tag: get_i32(s, "userTag").unwrap_or(0),
        })
    }
}

// ── Field accessors used by both Alarm/TimeStamp and the generated
//    derive code. Public-but-not-re-exported so derive expansion can
//    reach them via `epics_pva_rs::nt::typed::__rt::*`. -----------

/// Internal helpers consumed only by the `#[derive(NTScalar)]`
/// expansion. Stable surface, but operators of derive-generated
/// code don't import from here directly.
pub mod __rt {
    pub use crate::nt::typed::{Alarm, TimeStamp, TypedNT, TypedNTError};
    pub use crate::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};

    pub fn get_i32(s: &PvStructure, name: &str) -> Option<i32> {
        super::get_i32(s, name)
    }
    pub fn get_i64(s: &PvStructure, name: &str) -> Option<i64> {
        super::get_i64(s, name)
    }
    pub fn get_f32(s: &PvStructure, name: &str) -> Option<f32> {
        super::get_f32(s, name)
    }
    pub fn get_f64(s: &PvStructure, name: &str) -> Option<f64> {
        super::get_f64(s, name)
    }
    pub fn get_bool(s: &PvStructure, name: &str) -> Option<bool> {
        super::get_bool(s, name)
    }
    pub fn get_string(s: &PvStructure, name: &str) -> Option<String> {
        super::get_str(s, name)
    }

    pub fn missing(name: &str) -> TypedNTError {
        TypedNTError::MissingField(name.into())
    }

    pub fn wrong_type(field: &str, detail: &str) -> TypedNTError {
        TypedNTError::WrongType {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub fn wrong_struct_id(expected: &str, got: &str) -> TypedNTError {
        TypedNTError::WrongStructId {
            expected: expected.into(),
            got: got.into(),
        }
    }
}

fn get_i32(s: &PvStructure, name: &str) -> Option<i32> {
    match s.get_field(name)? {
        PvField::Scalar(ScalarValue::Int(v)) => Some(*v),
        PvField::Scalar(ScalarValue::Short(v)) => Some(*v as i32),
        PvField::Scalar(ScalarValue::Byte(v)) => Some(*v as i32),
        _ => None,
    }
}

fn get_i64(s: &PvStructure, name: &str) -> Option<i64> {
    match s.get_field(name)? {
        PvField::Scalar(ScalarValue::Long(v)) => Some(*v),
        PvField::Scalar(ScalarValue::Int(v)) => Some(*v as i64),
        _ => None,
    }
}

fn get_f32(s: &PvStructure, name: &str) -> Option<f32> {
    match s.get_field(name)? {
        PvField::Scalar(ScalarValue::Float(v)) => Some(*v),
        PvField::Scalar(ScalarValue::Double(v)) => Some(*v as f32),
        _ => None,
    }
}

fn get_f64(s: &PvStructure, name: &str) -> Option<f64> {
    match s.get_field(name)? {
        PvField::Scalar(ScalarValue::Double(v)) => Some(*v),
        PvField::Scalar(ScalarValue::Float(v)) => Some(*v as f64),
        PvField::Scalar(ScalarValue::Long(v)) => Some(*v as f64),
        PvField::Scalar(ScalarValue::Int(v)) => Some(*v as f64),
        _ => None,
    }
}

fn get_bool(s: &PvStructure, name: &str) -> Option<bool> {
    match s.get_field(name)? {
        PvField::Scalar(ScalarValue::Boolean(v)) => Some(*v),
        _ => None,
    }
}

fn get_str(s: &PvStructure, name: &str) -> Option<String> {
    match s.get_field(name)? {
        PvField::Scalar(ScalarValue::String(v)) => Some(v.clone()),
        _ => None,
    }
}

// ── Manual TypedNT impls for the primitive scalar wrappers.
//
// Most users will go through `#[derive(NTScalar)]` on a struct, but a
// bare scalar like `f64` is also useful when wrapping a single-value
// PV (e.g. `pvget_typed::<f64>`). The descriptor we emit is
// `epics:nt/NTScalar:1.0 { value: <T> }` — same as
// `NTScalar::new(<T>).build()` minus the optional meta substructures.

/// Build the `epics:nt/NTScalar:1.0 { value: <st> }` descriptor.
fn nt_scalar_root(value_field: FieldDesc) -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "epics:nt/NTScalar:1.0".into(),
        fields: vec![("value".into(), value_field)],
    }
}

fn nt_scalar_value(s: &PvStructure) -> Result<&PvField, TypedNTError> {
    if !(s.struct_id.is_empty() || s.struct_id == "epics:nt/NTScalar:1.0") {
        return Err(TypedNTError::WrongStructId {
            expected: "epics:nt/NTScalar:1.0".into(),
            got: s.struct_id.clone(),
        });
    }
    s.get_field("value")
        .ok_or_else(|| TypedNTError::MissingField("value".into()))
}

macro_rules! impl_typed_nt_scalar {
    ($t:ty, $st:ident, $sv:ident) => {
        impl TypedNT for $t {
            fn descriptor() -> FieldDesc {
                nt_scalar_root(FieldDesc::Scalar(crate::pvdata::ScalarType::$st))
            }
            fn to_pv_field(&self) -> PvField {
                let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
                s.fields
                    .push(("value".into(), PvField::Scalar(ScalarValue::$sv(*self))));
                PvField::Structure(s)
            }
            fn from_pv_field(field: &PvField) -> Result<Self, TypedNTError> {
                match field {
                    PvField::Scalar(ScalarValue::$sv(v)) => Ok(*v),
                    PvField::Structure(s) => match nt_scalar_value(s)? {
                        PvField::Scalar(ScalarValue::$sv(v)) => Ok(*v),
                        other => Err(TypedNTError::WrongType {
                            field: "value".into(),
                            detail: format!("expected {} scalar, got {other:?}", stringify!($st)),
                        }),
                    },
                    other => Err(TypedNTError::WrongType {
                        field: "<root>".into(),
                        detail: format!("expected NTScalar wrapper, got {other:?}"),
                    }),
                }
            }
        }
    };
}

impl_typed_nt_scalar!(f64, Double, Double);
impl_typed_nt_scalar!(f32, Float, Float);
impl_typed_nt_scalar!(i64, Long, Long);
impl_typed_nt_scalar!(i32, Int, Int);
impl_typed_nt_scalar!(i16, Short, Short);
impl_typed_nt_scalar!(i8, Byte, Byte);
impl_typed_nt_scalar!(u64, ULong, ULong);
impl_typed_nt_scalar!(u32, UInt, UInt);
impl_typed_nt_scalar!(u16, UShort, UShort);
impl_typed_nt_scalar!(u8, UByte, UByte);
impl_typed_nt_scalar!(bool, Boolean, Boolean);

impl TypedNT for String {
    fn descriptor() -> FieldDesc {
        nt_scalar_root(FieldDesc::Scalar(crate::pvdata::ScalarType::String))
    }
    fn to_pv_field(&self) -> PvField {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::String(self.clone()))));
        PvField::Structure(s)
    }
    fn from_pv_field(field: &PvField) -> Result<Self, TypedNTError> {
        match field {
            PvField::Scalar(ScalarValue::String(v)) => Ok(v.clone()),
            PvField::Structure(s) => match nt_scalar_value(s)? {
                PvField::Scalar(ScalarValue::String(v)) => Ok(v.clone()),
                other => Err(TypedNTError::WrongType {
                    field: "value".into(),
                    detail: format!("expected String scalar, got {other:?}"),
                }),
            },
            other => Err(TypedNTError::WrongType {
                field: "<root>".into(),
                detail: format!("expected NTScalar wrapper, got {other:?}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_round_trip() {
        let v: f64 = 3.14;
        let field = v.to_pv_field();
        let back = f64::from_pv_field(&field).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn f64_descriptor_shape() {
        match f64::descriptor() {
            FieldDesc::Structure { struct_id, fields } => {
                assert_eq!(struct_id, "epics:nt/NTScalar:1.0");
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].0, "value");
            }
            other => panic!("expected Structure descriptor, got {other:?}"),
        }
    }

    #[test]
    fn alarm_round_trip() {
        let a = Alarm {
            severity: 2,
            status: 7,
            message: "hi".into(),
        };
        let field = TypedNT::to_pv_field(&a);
        let back = <Alarm as TypedNT>::from_pv_field(&field).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn from_wrong_struct_id_rejected() {
        let mut s = PvStructure::new("epics:nt/NTTable:1.0");
        s.fields.push((
            "value".into(),
            PvField::Scalar(ScalarValue::Double(1.0)),
        ));
        let err = f64::from_pv_field(&PvField::Structure(s)).unwrap_err();
        assert!(matches!(err, TypedNTError::WrongStructId { .. }));
    }

    #[test]
    fn missing_value_rejected() {
        let s = PvStructure::new("epics:nt/NTScalar:1.0");
        let err = f64::from_pv_field(&PvField::Structure(s)).unwrap_err();
        assert!(matches!(err, TypedNTError::MissingField(_)));
    }

    #[test]
    fn wrong_scalar_type_rejected() {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields.push((
            "value".into(),
            PvField::Scalar(ScalarValue::String("oops".into())),
        ));
        let err = f64::from_pv_field(&PvField::Structure(s)).unwrap_err();
        assert!(matches!(err, TypedNTError::WrongType { .. }));
    }
}
