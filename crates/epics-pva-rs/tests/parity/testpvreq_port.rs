//! Port of pvxs's `test/testpvreq.cpp::testPvRequest()`.
//!
//! Verifies our `pv_request::request_to_mask` translates a pvRequest
//! structure into the correct `BitSet` using pvData §5.4 bit numbering.

#![cfg(test)]

use epics_pva_rs::proto::BitSet;
use epics_pva_rs::pv_request::{request_to_mask, RequestMaskError};
use epics_pva_rs::pvdata::{FieldDesc, ScalarType};

/// Returns true iff `marked` and `mask` share at least one set bit.
/// Mirrors pvxs's `testmask(val, mask)` semantic.
fn intersects(marked: &BitSet, mask: &BitSet) -> bool {
    mask.iter().any(|i| marked.get(i))
}

/// NTScalar(String) descriptor: 10 bits total (root + value + alarm{3} +
/// timeStamp{3}).
fn nt_scalar_string() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "epics:nt/NTScalar:1.0".into(),
        fields: vec![
            ("value".into(), FieldDesc::Scalar(ScalarType::String)),
            (
                "alarm".into(),
                FieldDesc::Structure {
                    struct_id: "alarm_t".into(),
                    fields: vec![
                        ("severity".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("status".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("message".into(), FieldDesc::Scalar(ScalarType::String)),
                    ],
                },
            ),
            (
                "timeStamp".into(),
                FieldDesc::Structure {
                    struct_id: "time_t".into(),
                    fields: vec![
                        ("secondsPastEpoch".into(), FieldDesc::Scalar(ScalarType::Long)),
                        ("nanoseconds".into(), FieldDesc::Scalar(ScalarType::Int)),
                        ("userTag".into(), FieldDesc::Scalar(ScalarType::Int)),
                    ],
                },
            ),
        ],
    }
}

fn empty_request_field() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![(
            "field".into(),
            FieldDesc::Structure {
                struct_id: String::new(),
                fields: Vec::new(),
            },
        )],
    }
}

fn request_field_value() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![(
            "field".into(),
            FieldDesc::Structure {
                struct_id: String::new(),
                fields: vec![(
                    "value".into(),
                    FieldDesc::Structure {
                        struct_id: String::new(),
                        fields: Vec::new(),
                    },
                )],
            },
        )],
    }
}

fn request_field_alarm_status_and_timestamp() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![(
            "field".into(),
            FieldDesc::Structure {
                struct_id: String::new(),
                fields: vec![
                    (
                        "timeStamp".into(),
                        FieldDesc::Structure {
                            struct_id: String::new(),
                            fields: Vec::new(),
                        },
                    ),
                    (
                        "alarm".into(),
                        FieldDesc::Structure {
                            struct_id: String::new(),
                            fields: vec![(
                                "status".into(),
                                FieldDesc::Structure {
                                    struct_id: String::new(),
                                    fields: Vec::new(),
                                },
                            )],
                        },
                    ),
                ],
            },
        )],
    }
}

fn collect(bitset: &BitSet) -> Vec<usize> {
    bitset.iter().collect()
}

#[test]
fn pvxs_request_empty_field_selects_all_bits() {
    // pvxs: request `{ field {} }` → mask = {0,1,2,3,4,5,6,7,8,9}
    let value = nt_scalar_string();
    let req = empty_request_field();
    let mask = request_to_mask(&value, &req).unwrap();
    assert_eq!(collect(&mask), (0..10).collect::<Vec<_>>());
}

#[test]
fn pvxs_request_value_only_selects_root_and_value() {
    // pvxs: request `{ field { value {} } }` → mask = {0, 1}
    let value = nt_scalar_string();
    let req = request_field_value();
    let mask = request_to_mask(&value, &req).unwrap();
    assert_eq!(collect(&mask), vec![0, 1]);
}

#[test]
fn pvxs_request_alarm_status_and_timestamp_selects_subtrees() {
    // pvxs: request `{ field { timeStamp {}, alarm { status {} } } }`
    // → mask = {0, 2, 4, 6, 7, 8, 9}
    //   bit 0 root, 2=alarm, 4=alarm.status, 6=timeStamp,
    //   7..9 = timeStamp.{seconds,nanoseconds,userTag} (empty {} sub-selects all)
    let value = nt_scalar_string();
    let req = request_field_alarm_status_and_timestamp();
    let mask = request_to_mask(&value, &req).unwrap();
    assert_eq!(collect(&mask), vec![0, 2, 4, 6, 7, 8, 9]);
}

#[test]
fn pvxs_request_includes_skips_nonexistent_field_silently() {
    // pvxs: request `{ field { timeStamp {}, nonexistent {}, alarm { status {} } } }`
    // → same as above, ignoring nonexistent
    let value = nt_scalar_string();
    let req = FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![(
            "field".into(),
            FieldDesc::Structure {
                struct_id: String::new(),
                fields: vec![
                    (
                        "timeStamp".into(),
                        FieldDesc::Structure {
                            struct_id: String::new(),
                            fields: Vec::new(),
                        },
                    ),
                    (
                        "nonexistent".into(),
                        FieldDesc::Structure {
                            struct_id: String::new(),
                            fields: Vec::new(),
                        },
                    ),
                    (
                        "alarm".into(),
                        FieldDesc::Structure {
                            struct_id: String::new(),
                            fields: vec![(
                                "status".into(),
                                FieldDesc::Structure {
                                    struct_id: String::new(),
                                    fields: Vec::new(),
                                },
                            )],
                        },
                    ),
                ],
            },
        )],
    };
    let mask = request_to_mask(&value, &req).unwrap();
    assert_eq!(collect(&mask), vec![0, 2, 4, 6, 7, 8, 9]);
}

// ── testPvMask (pvxs testpvreq.cpp:102) ────────────────────────────
//
// pvxs builds an NTScalar(String) value, requests `field { value {} }`
// (mask = {0,1}), then walks through "marked"/"unmarked" states and
// checks whether the marked-bitset intersects the request mask. We
// don't have pvxs's per-field mark state on Value — so we drive the
// "marked" side directly with a BitSet.
//
// pvxs bit numbering for NTScalar(String):
//   0: root, 1: value, 2: alarm, 3: alarm.severity, 4: alarm.status,
//   5: alarm.message, 6: timeStamp, 7..9: timeStamp.{seconds,ns,userTag}

#[test]
fn pvxs_pv_mask_value_request_intersects_value_only() {
    let value = nt_scalar_string();
    let req = request_field_value();
    let mask = request_to_mask(&value, &req).unwrap();
    assert_eq!(collect(&mask), vec![0, 1]);

    // initially nothing marked → no intersection
    let mut marked = BitSet::new();
    assert!(!intersects(&marked, &mask));

    // mark alarm.status only (bit 4) → still no intersection
    marked.set(4);
    assert!(!intersects(&marked, &mask));

    // mark value (bit 1) → intersects
    marked.set(1);
    assert!(intersects(&marked, &mask));

    // unmark alarm.status → still intersects (value still set)
    marked.clear(4);
    assert!(intersects(&marked, &mask));

    // unmark all → no intersection
    marked = BitSet::new();
    assert!(!intersects(&marked, &mask));

    // mark all (root bit) → intersects (mask has root bit too)
    marked.set(0);
    assert!(intersects(&marked, &mask));
}

#[test]
fn pvxs_pv_mask_alarm_only_does_not_intersect_value_request() {
    let value = nt_scalar_string();
    let req = request_field_value();
    let mask = request_to_mask(&value, &req).unwrap(); // {0, 1}

    // marked = {alarm.severity, alarm.status, alarm.message} = bits 3,4,5
    let mut marked = BitSet::new();
    marked.set(3);
    marked.set(4);
    marked.set(5);
    assert!(!intersects(&marked, &mask));
}

#[test]
fn pvxs_pv_mask_empty_request_intersects_anything() {
    // empty `field {}` mask = all 10 bits
    let value = nt_scalar_string();
    let req = empty_request_field();
    let mask = request_to_mask(&value, &req).unwrap();

    let mut marked = BitSet::new();
    marked.set(7); // timeStamp.secondsPastEpoch
    assert!(intersects(&marked, &mask));
}

#[test]
fn pvxs_request_only_nonexistent_field_errors() {
    // pvxs: request `{ field { nonexistent {} } }` throws runtime_error
    let value = nt_scalar_string();
    let req = FieldDesc::Structure {
        struct_id: String::new(),
        fields: vec![(
            "field".into(),
            FieldDesc::Structure {
                struct_id: String::new(),
                fields: vec![(
                    "nonexistent".into(),
                    FieldDesc::Structure {
                        struct_id: String::new(),
                        fields: Vec::new(),
                    },
                )],
            },
        )],
    };
    let err = request_to_mask(&value, &req).unwrap_err();
    assert_eq!(err, RequestMaskError::EmptyMask);
}
