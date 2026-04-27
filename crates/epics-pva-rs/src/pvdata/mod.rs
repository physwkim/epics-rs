//! pvData runtime model: scalar tags/values, structures, and field
//! descriptors. Wire encoding/decoding lives in [`encode`].

mod field;
mod scalar;
mod structure;
mod value;

pub mod encode;

pub use field::FieldDesc;
pub use scalar::{ScalarType, ScalarValue};
pub use structure::{PvField, PvStructure, UnionItem, VariantValue};
pub use value::{FromScalarValue, IntoScalarValue, Value, ValueError};
