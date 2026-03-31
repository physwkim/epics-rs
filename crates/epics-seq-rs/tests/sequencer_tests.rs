//! Integration tests for epics-seq-rs: event flags, channel store, program builder, variables.
//!
//! These tests exercise the public API of the sequencer runtime without
//! requiring a live EPICS IOC or CA server.

use std::sync::Arc;

use epics_base_rs::types::EpicsValue;
use epics_seq_rs::channel_store::ChannelStore;
use epics_seq_rs::event_flag::EventFlagSet;
use epics_seq_rs::variables::{ChannelDef, ProgramMeta, ProgramVars};
use tokio::sync::Notify;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Minimal ProgramVars implementation for testing.
#[derive(Clone)]
struct TestVars {
    values: Vec<f64>,
}

impl ProgramVars for TestVars {
    fn get_channel_value(&self, ch_id: usize) -> EpicsValue {
        EpicsValue::Double(self.values.get(ch_id).copied().unwrap_or(0.0))
    }
    fn set_channel_value(&mut self, ch_id: usize, value: &EpicsValue) {
        if let Some(v) = value.to_f64() {
            if ch_id < self.values.len() {
                self.values[ch_id] = v;
            }
        }
    }
}

/// Minimal ProgramMeta implementation for testing.
struct TestMeta;

impl ProgramMeta for TestMeta {
    const NUM_CHANNELS: usize = 2;
    const NUM_EVENT_FLAGS: usize = 1;
    const NUM_STATE_SETS: usize = 1;

    fn channel_defs() -> Vec<ChannelDef> {
        vec![
            ChannelDef {
                var_name: "x".into(),
                pv_name: "{P}x".into(),
                monitored: true,
                sync_ef: Some(0),
            },
            ChannelDef {
                var_name: "y".into(),
                pv_name: "{P}y".into(),
                monitored: false,
                sync_ef: None,
            },
        ]
    }

    fn event_flag_sync_map() -> Vec<Vec<usize>> {
        vec![vec![0]]
    }
}

fn make_wakeups(n: usize) -> Vec<Arc<Notify>> {
    (0..n).map(|_| Arc::new(Notify::new())).collect()
}

// ===========================================================================
// 1. Event flag operations
// ===========================================================================

#[test]
fn ef_initial_state_is_clear() {
    let efs = EventFlagSet::new(4, vec![vec![]; 4], make_wakeups(1));
    for i in 0..4 {
        assert!(!efs.test(i), "flag {i} should start cleared");
    }
}

#[test]
fn ef_set_and_test() {
    let efs = EventFlagSet::new(3, vec![vec![]; 3], make_wakeups(2));
    efs.set(1);
    assert!(!efs.test(0));
    assert!(efs.test(1));
    assert!(!efs.test(2));
}

#[test]
fn ef_clear_returns_previous_value() {
    let efs = EventFlagSet::new(2, vec![vec![]; 2], make_wakeups(1));
    // Clear an already-clear flag -> false
    assert!(!efs.clear(0));
    // Set then clear -> true
    efs.set(0);
    assert!(efs.clear(0));
    // After clear, test -> false
    assert!(!efs.test(0));
    // Clear again -> false
    assert!(!efs.clear(0));
}

#[test]
fn ef_test_and_clear_atomic() {
    let efs = EventFlagSet::new(2, vec![vec![]; 2], make_wakeups(1));
    efs.set(0);
    efs.set(1);

    // test_and_clear returns old value and clears
    assert!(efs.test_and_clear(0));
    assert!(!efs.test(0));

    // Second call returns false
    assert!(!efs.test_and_clear(0));

    // Flag 1 should be unaffected
    assert!(efs.test(1));
}

#[test]
fn ef_out_of_range_returns_false() {
    let efs = EventFlagSet::new(1, vec![vec![]], make_wakeups(1));
    assert!(!efs.test(100));
    assert!(!efs.clear(100));
    assert!(!efs.test_and_clear(100));
}

#[test]
fn ef_set_wakes_all_state_sets() {
    let wakeups = make_wakeups(3);
    let efs = EventFlagSet::new(1, vec![vec![]], wakeups.clone());
    // After set, each wakeup should have a pending notification.
    efs.set(0);
    // We can verify by checking that notified() would return immediately
    // in a tokio runtime, but here we just verify the flag was set.
    assert!(efs.test(0));
}

#[test]
fn ef_synced_channels_mapping() {
    let sync_map = vec![vec![0, 2], vec![1]];
    let efs = EventFlagSet::new(2, sync_map, make_wakeups(1));
    assert_eq!(efs.synced_channels(0), &[0, 2]);
    assert_eq!(efs.synced_channels(1), &[1]);
    assert_eq!(efs.synced_channels(99), &[] as &[usize]);
}

#[test]
fn ef_num_flags() {
    let efs = EventFlagSet::new(5, vec![vec![]; 5], make_wakeups(1));
    assert_eq!(efs.num_flags(), 5);
}

// ===========================================================================
// 2. ChannelStore value management
// ===========================================================================

#[test]
fn store_default_values() {
    let store = ChannelStore::new(3);
    assert_eq!(store.num_channels(), 3);
    for i in 0..3 {
        let val = store.get(i).unwrap();
        assert_eq!(val, EpicsValue::Double(0.0));
    }
}

#[test]
fn store_set_and_get_double() {
    let store = ChannelStore::new(2);
    store.set(0, EpicsValue::Double(3.14));
    store.set(1, EpicsValue::Double(-1.0));
    assert_eq!(store.get(0).unwrap(), EpicsValue::Double(3.14));
    assert_eq!(store.get(1).unwrap(), EpicsValue::Double(-1.0));
}

#[test]
fn store_set_and_get_various_types() {
    let store = ChannelStore::new(4);
    store.set(0, EpicsValue::Long(42));
    store.set(1, EpicsValue::String("hello".into()));
    store.set(2, EpicsValue::Short(7));
    store.set(3, EpicsValue::Float(2.5));

    assert_eq!(store.get(0).unwrap(), EpicsValue::Long(42));
    assert_eq!(store.get(1).unwrap(), EpicsValue::String("hello".into()));
    assert_eq!(store.get(2).unwrap(), EpicsValue::Short(7));
    assert_eq!(store.get(3).unwrap(), EpicsValue::Float(2.5));
}

#[test]
fn store_overwrite() {
    let store = ChannelStore::new(1);
    store.set(0, EpicsValue::Double(1.0));
    store.set(0, EpicsValue::Double(2.0));
    assert_eq!(store.get(0).unwrap(), EpicsValue::Double(2.0));
}

#[test]
fn store_set_full_with_metadata() {
    let store = ChannelStore::new(1);
    store.set_full(0, EpicsValue::Long(99), 3, 2);
    let slot = store.get_full(0).unwrap();
    assert_eq!(slot.value, EpicsValue::Long(99));
    assert_eq!(slot.status, 3);
    assert_eq!(slot.severity, 2);
}

#[test]
fn store_invalid_channel_get_returns_none() {
    let store = ChannelStore::new(1);
    assert!(store.get(10).is_none());
    assert!(store.get_full(10).is_none());
}

#[test]
fn store_invalid_channel_set_is_noop() {
    let store = ChannelStore::new(1);
    store.set(99, EpicsValue::Double(1.0)); // should not panic
    store.set_full(99, EpicsValue::Double(1.0), 0, 0); // should not panic
}

#[test]
fn store_concurrent_access() {
    use std::thread;

    let store = Arc::new(ChannelStore::new(2));
    let s1 = store.clone();
    let s2 = store.clone();

    let writer = thread::spawn(move || {
        for i in 0..200 {
            s1.set(0, EpicsValue::Double(i as f64));
            s1.set(1, EpicsValue::Long(i));
        }
    });

    let reader = thread::spawn(move || {
        for _ in 0..200 {
            let _ = s2.get(0);
            let _ = s2.get(1);
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
    // No panic or data corruption.
}

// ===========================================================================
// 3. ProgramBuilder construction
// ===========================================================================

#[test]
fn program_builder_new() {
    use epics_seq_rs::program::ProgramBuilder;

    let vars = TestVars {
        values: vec![0.0; 2],
    };
    let builder = ProgramBuilder::<TestVars, TestMeta>::new("test_prog", vars);
    assert_eq!(builder.name, "test_prog");
    assert!(builder.macros.is_empty());
    assert!(builder.state_set_fns.is_empty());
}

#[test]
fn program_builder_macros() {
    use epics_seq_rs::program::ProgramBuilder;

    let vars = TestVars {
        values: vec![0.0; 2],
    };
    let builder =
        ProgramBuilder::<TestVars, TestMeta>::new("test_prog", vars).macros("P=TEST:,R=ai1");
    assert_eq!(builder.macros.get("P").unwrap(), "TEST:");
    assert_eq!(builder.macros.get("R").unwrap(), "ai1");
}

#[test]
fn program_builder_add_state_sets() {
    use epics_seq_rs::program::ProgramBuilder;

    let vars = TestVars {
        values: vec![0.0; 2],
    };
    let builder = ProgramBuilder::<TestVars, TestMeta>::new("test_prog", vars)
        .add_ss(Box::new(|_ctx| Box::pin(async { Ok(()) })))
        .add_ss(Box::new(|_ctx| Box::pin(async { Ok(()) })));
    assert_eq!(builder.state_set_fns.len(), 2);
}

// ===========================================================================
// 4. ProgramVars / ProgramMeta trait implementations
// ===========================================================================

#[test]
fn program_vars_get_set_roundtrip() {
    let mut vars = TestVars {
        values: vec![0.0, 0.0, 0.0],
    };
    vars.set_channel_value(0, &EpicsValue::Double(42.0));
    vars.set_channel_value(1, &EpicsValue::Long(7));
    vars.set_channel_value(2, &EpicsValue::Float(3.14));

    assert_eq!(vars.get_channel_value(0), EpicsValue::Double(42.0));
    assert_eq!(vars.get_channel_value(1), EpicsValue::Double(7.0));
    // Float -> Double conversion
    let val = vars.get_channel_value(2);
    if let EpicsValue::Double(v) = val {
        assert!((v - 3.14).abs() < 0.01);
    } else {
        panic!("expected Double, got {val:?}");
    }
}

#[test]
fn program_vars_out_of_range_set_is_noop() {
    let mut vars = TestVars {
        values: vec![0.0],
    };
    // Setting out-of-range should not panic
    vars.set_channel_value(99, &EpicsValue::Double(1.0));
    assert_eq!(vars.get_channel_value(0), EpicsValue::Double(0.0));
}

#[test]
fn program_vars_out_of_range_get_returns_zero() {
    let vars = TestVars {
        values: vec![1.0],
    };
    // Out-of-range channel returns 0.0
    assert_eq!(vars.get_channel_value(99), EpicsValue::Double(0.0));
}

#[test]
fn program_meta_constants() {
    assert_eq!(TestMeta::NUM_CHANNELS, 2);
    assert_eq!(TestMeta::NUM_EVENT_FLAGS, 1);
    assert_eq!(TestMeta::NUM_STATE_SETS, 1);
}

#[test]
fn program_meta_channel_defs() {
    let defs = TestMeta::channel_defs();
    assert_eq!(defs.len(), 2);
    assert_eq!(defs[0].var_name, "x");
    assert_eq!(defs[0].pv_name, "{P}x");
    assert!(defs[0].monitored);
    assert_eq!(defs[0].sync_ef, Some(0));
    assert_eq!(defs[1].var_name, "y");
    assert!(!defs[1].monitored);
    assert_eq!(defs[1].sync_ef, None);
}

#[test]
fn program_meta_event_flag_sync_map() {
    let map = TestMeta::event_flag_sync_map();
    assert_eq!(map.len(), 1);
    assert_eq!(map[0], vec![0]);
}

// ===========================================================================
// 5. ChannelDef construction
// ===========================================================================

#[test]
fn channel_def_construction() {
    let def = ChannelDef {
        var_name: "temperature".into(),
        pv_name: "$(P)temperature".into(),
        monitored: true,
        sync_ef: Some(2),
    };
    assert_eq!(def.var_name, "temperature");
    assert_eq!(def.pv_name, "$(P)temperature");
    assert!(def.monitored);
    assert_eq!(def.sync_ef, Some(2));
}

// ===========================================================================
// 6. Macro parsing (via program builder integration)
// ===========================================================================

#[test]
fn macro_parsing_empty() {
    use epics_seq_rs::program::ProgramBuilder;

    let vars = TestVars {
        values: vec![0.0; 2],
    };
    let builder = ProgramBuilder::<TestVars, TestMeta>::new("test", vars).macros("");
    assert!(builder.macros.is_empty());
}

#[test]
fn macro_parsing_multiple() {
    use epics_seq_rs::program::ProgramBuilder;

    let vars = TestVars {
        values: vec![0.0; 2],
    };
    let builder =
        ProgramBuilder::<TestVars, TestMeta>::new("test", vars).macros("A=1,B=hello,C=x:y");
    assert_eq!(builder.macros.len(), 3);
    assert_eq!(builder.macros.get("A").unwrap(), "1");
    assert_eq!(builder.macros.get("B").unwrap(), "hello");
    assert_eq!(builder.macros.get("C").unwrap(), "x:y");
}

// ===========================================================================
// 7. Integration: event flags + channel store (selective sync pattern)
// ===========================================================================

#[test]
fn ef_channel_sync_pattern() {
    // Simulate the pattern used in the sequencer runtime:
    // 1. Monitor updates channel store and sets event flag
    // 2. State set tests event flag and syncs channels

    let store = Arc::new(ChannelStore::new(2));
    let sync_map = vec![vec![0], vec![1]]; // ef0 -> ch0, ef1 -> ch1
    let efs = Arc::new(EventFlagSet::new(2, sync_map, make_wakeups(1)));

    // Simulate monitor update for channel 0
    store.set(0, EpicsValue::Double(100.0));
    efs.set(0);

    // State set tests ef0 -> true, syncs ch0
    assert!(efs.test(0));
    let synced = efs.synced_channels(0);
    assert_eq!(synced, &[0]);
    for &ch_id in synced {
        let val = store.get(ch_id).unwrap();
        assert_eq!(val, EpicsValue::Double(100.0));
    }

    // After clear, flag is gone
    efs.clear(0);
    assert!(!efs.test(0));

    // ef1 should still be clear
    assert!(!efs.test(1));
}

// ===========================================================================
// 8. Multi-flag independence
// ===========================================================================

#[test]
fn multiple_flags_independent() {
    let efs = EventFlagSet::new(4, vec![vec![]; 4], make_wakeups(1));

    efs.set(0);
    efs.set(2);

    assert!(efs.test(0));
    assert!(!efs.test(1));
    assert!(efs.test(2));
    assert!(!efs.test(3));

    efs.clear(0);
    assert!(!efs.test(0));
    assert!(efs.test(2));
}
