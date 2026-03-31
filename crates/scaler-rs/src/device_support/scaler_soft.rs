use std::sync::{Arc, Mutex};
use std::time::Instant;

use epics_base_rs::error::CaResult;

use super::scaler_asyn::ScalerDriver;
use crate::records::scaler::MAX_SCALER_CHANNELS;

/// Software scaler driver for testing and simulation.
///
/// Ported from `drvScalerSoft.c`. Implements `ScalerDriver` without
/// requiring real hardware. Counter values can be set externally
/// (e.g., from PV links or test code).
///
/// In the C implementation, this reads from template-based PV names.
/// In Rust, the counters are directly accessible for external updating.
pub struct SoftScalerDriver {
    num_channels: usize,
    counts: [u32; MAX_SCALER_CHANNELS],
    presets: [u32; MAX_SCALER_CHANNELS],
    gates: [bool; MAX_SCALER_CHANNELS],
    armed: bool,
    done_flag: bool,
    start_time: Option<Instant>,
    /// Shared reference for external counter updates.
    shared_counts: Arc<Mutex<[u32; MAX_SCALER_CHANNELS]>>,
}

impl SoftScalerDriver {
    pub fn new(num_channels: usize) -> Self {
        let num_channels = num_channels.min(MAX_SCALER_CHANNELS);
        Self {
            num_channels,
            counts: [0; MAX_SCALER_CHANNELS],
            presets: [0; MAX_SCALER_CHANNELS],
            gates: [false; MAX_SCALER_CHANNELS],
            armed: false,
            done_flag: false,
            start_time: None,
            shared_counts: Arc::new(Mutex::new([0; MAX_SCALER_CHANNELS])),
        }
    }

    /// Get a shared handle to the counter values for external updating.
    pub fn shared_counts(&self) -> Arc<Mutex<[u32; MAX_SCALER_CHANNELS]>> {
        Arc::clone(&self.shared_counts)
    }

    /// Check if any gated channel has reached its preset.
    fn check_presets(&self) -> bool {
        for i in 0..self.num_channels {
            if self.gates[i] && self.presets[i] > 0 && self.counts[i] >= self.presets[i] {
                return true;
            }
        }
        false
    }
}

impl ScalerDriver for SoftScalerDriver {
    fn reset(&mut self) -> CaResult<()> {
        self.counts = [0; MAX_SCALER_CHANNELS];
        self.done_flag = false;
        self.armed = false;
        self.start_time = None;
        let mut shared = self.shared_counts.lock().unwrap();
        *shared = [0; MAX_SCALER_CHANNELS];
        Ok(())
    }

    fn read(&mut self, counts: &mut [u32; MAX_SCALER_CHANNELS]) -> CaResult<()> {
        // Copy from shared state (externally updated)
        let shared = self.shared_counts.lock().unwrap();
        self.counts = *shared;

        // Check if any preset reached
        if self.armed && self.check_presets() {
            self.done_flag = true;
            self.armed = false;
        }

        *counts = self.counts;
        Ok(())
    }

    fn write_preset(&mut self, channel: usize, preset: u32) -> CaResult<()> {
        if channel < MAX_SCALER_CHANNELS {
            self.presets[channel] = preset;
            self.gates[channel] = preset > 0;
        }
        Ok(())
    }

    fn arm(&mut self, start: bool) -> CaResult<()> {
        self.armed = start;
        if start {
            self.done_flag = false;
            self.start_time = Some(Instant::now());
        } else {
            self.start_time = None;
        }
        Ok(())
    }

    fn done(&self) -> bool {
        self.done_flag
    }

    fn num_channels(&self) -> usize {
        self.num_channels
    }
}
