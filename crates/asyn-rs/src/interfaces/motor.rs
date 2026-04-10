//! Motor interface definitions.

use crate::error::AsynResult;
use crate::user::AsynUser;

/// Motor axis status.
/// Fields match the C asynMotorController MotorStatus structure.
#[derive(Debug, Clone)]
pub struct MotorStatus {
    /// Current position in user coordinates.
    pub position: f64,
    /// Encoder position (if available).
    pub encoder_position: f64,
    /// Current velocity.
    pub velocity: f64,
    /// True if the last move has completed.
    pub done: bool,
    /// True if the motor is currently moving.
    pub moving: bool,
    /// True if a positive (raw) limit switch is active.
    pub high_limit: bool,
    /// True if a negative (raw) limit switch is active.
    pub low_limit: bool,
    /// True if the home switch is active.
    pub home: bool,
    /// True if the motor is powered on / closed-loop is active.
    pub powered: bool,
    /// True if a problem was detected (driver stopped polling).
    pub problem: bool,
    /// Direction of last motion (true = positive).
    pub direction: bool,
    /// True if encoder slip is detected.
    pub slip_stall: bool,
    /// True if communication error was detected.
    pub comms_error: bool,
    /// True if the axis has been homed.
    pub homed: bool,
    /// True if the controller supports closed-loop gain.
    pub gain_support: bool,
    /// True if an encoder is present.
    pub has_encoder: bool,
}

impl Default for MotorStatus {
    fn default() -> Self {
        Self {
            position: 0.0,
            encoder_position: 0.0,
            velocity: 0.0,
            done: true,
            moving: false,
            high_limit: false,
            low_limit: false,
            home: false,
            powered: true,
            problem: false,
            direction: false,
            slip_stall: false,
            comms_error: false,
            homed: false,
            gain_support: false,
            has_encoder: false,
        }
    }
}

/// Motor interface trait.
///
/// Provides motor axis control for motor-capable drivers.
pub trait AsynMotor: Send + Sync {
    /// Move to an absolute position.
    fn move_absolute(
        &mut self,
        user: &AsynUser,
        position: f64,
        velocity: f64,
        acceleration: f64,
    ) -> AsynResult<()>;

    /// Move at a constant velocity (jog).
    /// Default implementation uses move_absolute to a very large target.
    fn move_velocity(
        &mut self,
        user: &AsynUser,
        velocity: f64,
        acceleration: f64,
    ) -> AsynResult<()> {
        let target = if velocity >= 0.0 { 1e9 } else { -1e9 };
        self.move_absolute(user, target, velocity.abs(), acceleration)
    }

    /// Start a homing sequence.
    fn home(&mut self, user: &AsynUser, velocity: f64, forward: bool) -> AsynResult<()>;

    /// Stop motion.
    fn stop(&mut self, user: &AsynUser, acceleration: f64) -> AsynResult<()>;

    /// Set the current position without moving.
    fn set_position(&mut self, user: &AsynUser, position: f64) -> AsynResult<()>;

    /// Enable or disable closed-loop control.
    fn set_closed_loop(&mut self, _user: &AsynUser, _enable: bool) -> AsynResult<()> {
        Ok(())
    }

    /// Poll the motor for current status.
    fn poll(&mut self, user: &AsynUser) -> AsynResult<MotorStatus>;
}
