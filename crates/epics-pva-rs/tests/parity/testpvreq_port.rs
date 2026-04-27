//! Port of pvxs's `test/testpvreq.cpp::testPvRequest()`.
//!
//! Verifies our `pv_request::request_to_mask` translates a pvRequest
//! structure into the correct `BitSet` using pvData §5.4 bit numbering.

#![cfg(test)]

use epics_pva_rs::proto::BitSet;
use epics_pva_rs::pv_request::{request_to_mask, RequestMaskError};
use epics_pva_rs::pvdata::{FieldDesc, ScalarType};

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
