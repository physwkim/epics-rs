//! Profile move support for coordinated trajectory execution.
//!
//! Matches the C asynMotorController profile move framework.

use std::time::Duration;

/// Profile time mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileTimeMode {
    #[default]
    Fixed = 0,
    Array = 1,
}

/// Profile move mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileMoveMode {
    #[default]
    Absolute = 0,
    Relative = 1,
}

/// Profile build/execute/readback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileState {
    #[default]
    Done = 0,
    Busy = 1,
}

/// Profile operation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileStatus {
    #[default]
    Undefined = 0,
    Success = 1,
    Failure = 2,
    Abort = 3,
    Timeout = 4,
}

/// Per-axis profile data.
#[derive(Debug, Clone, Default)]
pub struct AxisProfile {
    /// Whether this axis participates in the profile.
    pub use_axis: bool,
    /// Desired positions (set before build).
    pub positions: Vec<f64>,
    /// Actual readback positions (filled after readback).
    pub readbacks: Vec<f64>,
    /// Following errors (filled after readback).
    pub following_errors: Vec<f64>,
}

/// Controller-level profile configuration and state.
#[derive(Debug, Clone)]
pub struct ProfileController {
    pub num_axes: usize,
    pub max_points: usize,
    pub num_points: usize,
    pub time_mode: ProfileTimeMode,
    pub fixed_time: Duration,
    pub times: Vec<f64>,
    pub acceleration_time: f64,
    pub move_mode: ProfileMoveMode,
    pub axes: Vec<AxisProfile>,
    pub build_state: ProfileState,
    pub build_status: ProfileStatus,
    pub build_message: String,
    pub execute_state: ProfileState,
    pub execute_status: ProfileStatus,
    pub execute_message: String,
    pub readback_state: ProfileState,
    pub readback_status: ProfileStatus,
    pub readback_message: String,
}

impl ProfileController {
    /// Create a new profile controller for the given number of axes.
    pub fn new(num_axes: usize) -> Self {
        Self {
            num_axes,
            max_points: 0,
            num_points: 0,
            time_mode: ProfileTimeMode::Fixed,
            fixed_time: Duration::from_millis(100),
            times: Vec::new(),
            acceleration_time: 0.0,
            move_mode: ProfileMoveMode::Absolute,
            axes: (0..num_axes).map(|_| AxisProfile::default()).collect(),
            build_state: ProfileState::Done,
            build_status: ProfileStatus::Undefined,
            build_message: String::new(),
            execute_state: ProfileState::Done,
            execute_status: ProfileStatus::Undefined,
            execute_message: String::new(),
            readback_state: ProfileState::Done,
            readback_status: ProfileStatus::Undefined,
            readback_message: String::new(),
        }
    }

    /// Initialize profile with maximum number of points.
    pub fn initialize(&mut self, max_points: usize) {
        self.max_points = max_points;
        for axis in &mut self.axes {
            axis.positions = Vec::with_capacity(max_points);
            axis.readbacks = Vec::with_capacity(max_points);
            axis.following_errors = Vec::with_capacity(max_points);
        }
        self.times = Vec::with_capacity(max_points);
    }

    /// Set positions for a specific axis.
    pub fn set_axis_positions(&mut self, axis: usize, positions: Vec<f64>) {
        if axis < self.num_axes {
            self.num_points = positions.len().min(self.max_points);
            self.axes[axis].positions = positions;
        }
    }

    /// Set time array.
    pub fn set_times(&mut self, times: Vec<f64>) {
        self.times = times;
    }

    /// Mark build as started.
    pub fn start_build(&mut self) {
        self.build_state = ProfileState::Busy;
        self.build_status = ProfileStatus::Undefined;
        self.build_message.clear();
    }

    /// Mark build as complete.
    pub fn finish_build(&mut self, success: bool, message: &str) {
        self.build_state = ProfileState::Done;
        self.build_status = if success {
            ProfileStatus::Success
        } else {
            ProfileStatus::Failure
        };
        self.build_message = message.to_string();
    }

    /// Mark execute as started.
    pub fn start_execute(&mut self) {
        self.execute_state = ProfileState::Busy;
        self.execute_status = ProfileStatus::Undefined;
        self.execute_message.clear();
    }

    /// Mark execute as complete.
    pub fn finish_execute(&mut self, success: bool, message: &str) {
        self.execute_state = ProfileState::Done;
        self.execute_status = if success {
            ProfileStatus::Success
        } else {
            ProfileStatus::Failure
        };
        self.execute_message = message.to_string();
    }

    /// Mark execute as aborted.
    pub fn abort_execute(&mut self) {
        self.execute_state = ProfileState::Done;
        self.execute_status = ProfileStatus::Abort;
        self.execute_message = "Aborted".to_string();
    }

    /// Mark readback as started.
    pub fn start_readback(&mut self) {
        self.readback_state = ProfileState::Busy;
        self.readback_status = ProfileStatus::Undefined;
        self.readback_message.clear();
    }

    /// Mark readback as complete with data.
    pub fn finish_readback(
        &mut self,
        axis: usize,
        readbacks: Vec<f64>,
        following_errors: Vec<f64>,
    ) {
        if axis < self.num_axes {
            self.axes[axis].readbacks = readbacks;
            self.axes[axis].following_errors = following_errors;
        }
        self.readback_state = ProfileState::Done;
        self.readback_status = ProfileStatus::Success;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_controller_new() {
        let pc = ProfileController::new(4);
        assert_eq!(pc.num_axes, 4);
        assert_eq!(pc.axes.len(), 4);
        assert_eq!(pc.build_state, ProfileState::Done);
    }

    #[test]
    fn test_profile_initialize() {
        let mut pc = ProfileController::new(2);
        pc.initialize(1000);
        assert_eq!(pc.max_points, 1000);
    }

    #[test]
    fn test_profile_set_positions() {
        let mut pc = ProfileController::new(2);
        pc.initialize(100);
        pc.set_axis_positions(0, vec![1.0, 2.0, 3.0]);
        assert_eq!(pc.axes[0].positions, vec![1.0, 2.0, 3.0]);
        assert_eq!(pc.num_points, 3);
    }

    #[test]
    fn test_profile_build_state_machine() {
        let mut pc = ProfileController::new(1);
        pc.start_build();
        assert_eq!(pc.build_state, ProfileState::Busy);
        pc.finish_build(true, "OK");
        assert_eq!(pc.build_state, ProfileState::Done);
        assert_eq!(pc.build_status, ProfileStatus::Success);
    }

    #[test]
    fn test_profile_execute_abort() {
        let mut pc = ProfileController::new(1);
        pc.start_execute();
        assert_eq!(pc.execute_state, ProfileState::Busy);
        pc.abort_execute();
        assert_eq!(pc.execute_state, ProfileState::Done);
        assert_eq!(pc.execute_status, ProfileStatus::Abort);
    }

    #[test]
    fn test_profile_readback() {
        let mut pc = ProfileController::new(2);
        pc.initialize(10);
        pc.start_readback();
        pc.finish_readback(0, vec![1.0, 2.0], vec![0.01, 0.02]);
        assert_eq!(pc.axes[0].readbacks, vec![1.0, 2.0]);
        assert_eq!(pc.axes[0].following_errors, vec![0.01, 0.02]);
        assert_eq!(pc.readback_status, ProfileStatus::Success);
    }
}
