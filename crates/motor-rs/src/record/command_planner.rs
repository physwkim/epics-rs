use super::*;

impl MotorRecord {
    /// Plan and start a motion from a user write.
    pub fn plan_motion(&mut self, src: CommandSource) -> ProcessEffects {
        let mut effects = ProcessEffects::default();

        // SPMG, STOP, and SYNC always processed regardless of command gate
        match src {
            CommandSource::Spmg
            | CommandSource::Stop
            | CommandSource::Sync
            | CommandSource::Set
            | CommandSource::Cnen => {}
            _ => {
                if !self.can_accept_command() {
                    return effects;
                }
            }
        }

        match src {
            CommandSource::Val | CommandSource::Dval | CommandSource::Rval => {
                // Check for retarget if motion is in progress
                if self.stat.phase != MotionPhase::Idle {
                    let action = self.handle_retarget(self.pos.dval);
                    match action {
                        RetargetAction::Ignore => {
                            return effects;
                        }
                        RetargetAction::StopAndReplan => {
                            // Cancel any pending backlash/retry state
                            self.internal.backlash_pending = false;
                            self.retry.rcnt = 0;
                            self.internal.pending_retarget = Some(self.pos.dval);
                            self.stat.mip.insert(MipFlags::STOP);
                            effects.commands.push(MotorCommand::Stop {
                                acceleration: self.vel.accl,
                            });
                            effects.request_poll = true;
                            effects.suppress_forward_link = true;
                            return effects;
                        }
                        RetargetAction::ExtendMove => {
                            // C: same-direction target changes are simply accepted.
                            // The new DVAL is stored and the motor continues to the
                            // current target; backlash/retry will be re-evaluated
                            // when the current move completes.
                            self.internal.ldvl = self.pos.dval;
                            // Re-evaluate backlash for new target
                            self.internal.backlash_pending =
                                self.needs_backlash_for_move(self.pos.dval, self.pos.drbv);
                            effects.suppress_forward_link = true;
                            return effects;
                        }
                    }
                }
                self.plan_absolute_move(&mut effects);
            }
            CommandSource::Rlv => {
                // Relative move: VAL += RLV
                self.pos.val += self.pos.rlv;
                self.pos.rlv = 0.0;
                // Cascade from VAL
                if let Ok((dval, rval, off)) = coordinate::cascade_from_val(
                    self.pos.val,
                    self.conv.dir,
                    self.pos.off,
                    self.conv.foff,
                    self.conv.mres,
                    false,
                    self.pos.dval,
                ) {
                    self.pos.dval = dval;
                    self.pos.rval = rval;
                    self.pos.off = off;
                }
                self.plan_absolute_move(&mut effects);
            }
            CommandSource::Stop => {
                self.handle_stop(&mut effects);
            }
            CommandSource::Jogf | CommandSource::Jogr => {
                let forward = src == CommandSource::Jogf;
                let starting = if forward {
                    self.ctrl.jogf
                } else {
                    self.ctrl.jogr
                };
                if starting {
                    self.start_jog(forward, &mut effects);
                } else {
                    self.stop_jog(&mut effects);
                }
            }
            CommandSource::Homf | CommandSource::Homr => {
                let forward = src == CommandSource::Homf;
                self.start_home(forward, &mut effects);
            }
            CommandSource::Twf | CommandSource::Twr => {
                let forward = src == CommandSource::Twf;
                self.handle_tweak(forward, &mut effects);
            }
            CommandSource::Spmg => {
                self.handle_spmg_change(&mut effects);
            }
            CommandSource::Sync => {
                self.sync_positions();
            }
            CommandSource::Set => {
                // SET mode: recalculate RBV from new offset, then issue SetPosition
                self.pos.rbv = coordinate::dial_to_user(self.pos.drbv, self.conv.dir, self.pos.off);
                self.pos.diff = self.pos.dval - self.pos.drbv;
                self.pos.rdif = self.pos.val - self.pos.rbv;
                // Convert dial to raw steps for the driver (C: dval / mres)
                if let Ok(raw) = coordinate::dial_to_raw(self.pos.dval, self.conv.mres) {
                    effects.commands.push(MotorCommand::SetPosition {
                        position: raw as f64,
                    });
                }
            }
            CommandSource::Cnen => {
                effects.commands.push(MotorCommand::SetClosedLoop {
                    enable: self.ctrl.cnen,
                });
            }
        }

        effects
    }

    /// Plan an absolute move to current DVAL.
    pub(crate) fn plan_absolute_move(&mut self, effects: &mut ProcessEffects) {
        // Check soft limits (disabled when dhlm == dllm)
        if self.limits.dhlm != self.limits.dllm {
            // C: DLLM > DHLM means limits are inverted => always violation
            if self.limits.dllm > self.limits.dhlm {
                self.limits.lvio = true;
                tracing::warn!("limit violation: inverted limits dllm={:.4} > dhlm={:.4}",
                    self.limits.dllm, self.limits.dhlm);
                return;
            }

            let target_outside = self.pos.dval > self.limits.dhlm
                || self.pos.dval < self.limits.dllm;

            if target_outside {
                // C: allow move if heading toward the valid range
                // Compares dval against ldvl (previous target), but we use drbv
                // as an approximation since we don't track ldvl separately
                let currently_above = self.pos.drbv > self.limits.dhlm;
                let currently_below = self.pos.drbv < self.limits.dllm;
                let moving_toward_valid = (currently_above && self.pos.dval < self.pos.drbv)
                    || (currently_below && self.pos.dval > self.pos.drbv);

                if !moving_toward_valid {
                    self.limits.lvio = true;
                    tracing::warn!(
                        "limit violation: dval={:.4}, limits=[{:.4}, {:.4}]",
                        self.pos.dval,
                        self.limits.dllm,
                        self.limits.dhlm
                    );
                    return;
                }
            }

            // C: for non-preferred direction (backlash), also check pretarget
            if self.retry.bdst != 0.0 {
                let backlash_needed = self.needs_backlash_for_move(self.pos.dval, self.pos.drbv);
                if backlash_needed {
                    let pretarget = Self::compute_backlash_pretarget(self.pos.dval, self.retry.bdst);
                    if pretarget > self.limits.dhlm || pretarget < self.limits.dllm {
                        self.limits.lvio = true;
                        tracing::warn!(
                            "limit violation: backlash pretarget={:.4}, limits=[{:.4}, {:.4}]",
                            pretarget,
                            self.limits.dllm,
                            self.limits.dhlm
                        );
                        return;
                    }
                }
            }
        }
        self.limits.lvio = false;

        // SPDB deadband: suppress move if already within setpoint deadband
        if self.retry.spdb > 0.0 && (self.pos.dval - self.pos.drbv).abs() <= self.retry.spdb {
            return;
        }

        // Determine if backlash correction is needed
        let backlash = self.needs_backlash_for_move(self.pos.dval, self.pos.drbv);

        // Compute move target: pretarget if backlash, otherwise dval
        let move_target = if backlash {
            Self::compute_backlash_pretarget(self.pos.dval, self.retry.bdst)
        } else {
            self.pos.dval
        };

        // Check hardware limits based on first move direction
        let dir = if move_target > self.pos.drbv {
            MotionDirection::Positive
        } else {
            MotionDirection::Negative
        };
        if self.is_blocked_by_hw_limit(dir) {
            tracing::warn!("hardware limit active, blocking {dir:?} move");
            return;
        }

        // DMOV pulse: set false before starting
        self.stat.dmov = false;
        self.suppress_flnk = true;
        self.retry.rcnt = 0;
        self.retry.miss = false;

        // tdir reflects the actual first-command direction
        self.stat.tdir = move_target > self.pos.drbv;
        // CDIR: commanded direction from the position error
        // C: cdir = (rdif < 0.0) ? 0 : 1
        self.stat.cdir = self.pos.diff >= 0.0;

        // Set MIP and phase
        self.stat.mip = MipFlags::MOVE;
        self.set_phase(MotionPhase::MainMove);
        self.internal.backlash_pending = backlash;

        effects.commands.push(MotorCommand::MoveAbsolute {
            position: move_target,
            velocity: self.vel.velo,
            acceleration: self.vel.accl,
        });
        effects.request_poll = true;
        effects.suppress_forward_link = true;
    }

    /// Handle STOP command.
    fn handle_stop(&mut self, effects: &mut ProcessEffects) {
        self.ctrl.stop = false; // pulse field
        if self.stat.phase != MotionPhase::Idle {
            self.stat.mip.insert(MipFlags::STOP);
            self.internal.backlash_pending = false;
            self.internal.pending_retarget = None;
            effects.commands.push(MotorCommand::Stop {
                acceleration: self.vel.accl,
            });
            // Sync VAL to RBV after stop
            self.pos.val = self.pos.rbv;
            self.pos.dval = self.pos.drbv;
            self.pos.rval = self.pos.rrbv;
        }
    }

    /// Start jogging.
    fn start_jog(&mut self, forward: bool, effects: &mut ProcessEffects) {
        let dir = if forward {
            MotionDirection::Positive
        } else {
            MotionDirection::Negative
        };
        if self.is_blocked_by_hw_limit(dir) {
            return;
        }

        self.stat.dmov = false;
        self.suppress_flnk = true;

        if forward {
            self.stat.mip = MipFlags::JOGF;
        } else {
            self.stat.mip = MipFlags::JOGR;
        }
        self.set_phase(MotionPhase::Jog);

        // CDIR for jog: account for DIR and MRES sign
        // C: cdir computed from jog direction considering dir polarity and MRES sign
        let user_forward = if self.conv.dir == MotorDir::Neg {
            !forward
        } else {
            forward
        };
        self.stat.cdir = if self.conv.mres >= 0.0 {
            user_forward
        } else {
            !user_forward
        };

        effects.commands.push(MotorCommand::MoveVelocity {
            direction: forward,
            velocity: self.vel.jvel,
            acceleration: self.vel.jar,
        });
        effects.request_poll = true;
        effects.suppress_forward_link = true;
    }

    /// Stop jogging.
    fn stop_jog(&mut self, effects: &mut ProcessEffects) {
        self.stat.mip.insert(MipFlags::JOG_STOP);
        self.set_phase(MotionPhase::JogStopping);
        effects.commands.push(MotorCommand::Stop {
            acceleration: if self.vel.jar > 0.0 {
                self.vel.jar
            } else {
                self.vel.accl
            },
        });
    }

    /// Start homing.
    fn start_home(&mut self, forward: bool, effects: &mut ProcessEffects) {
        // C: check limit switch in direction of home before starting
        // HOMF blocked by HLS (when DIR=Pos) or LLS (when DIR=Neg)
        let blocked = if forward {
            if self.conv.dir == MotorDir::Pos {
                self.limits.hls
            } else {
                self.limits.lls
            }
        } else {
            if self.conv.dir == MotorDir::Pos {
                self.limits.lls
            } else {
                self.limits.hls
            }
        };
        if blocked {
            if forward {
                self.ctrl.homf = false;
            } else {
                self.ctrl.homr = false;
            }
            return;
        }

        self.stat.dmov = false;
        self.suppress_flnk = true;

        if forward {
            self.stat.mip = MipFlags::HOMF;
            self.ctrl.homf = false; // pulse
        } else {
            self.stat.mip = MipFlags::HOMR;
            self.ctrl.homr = false; // pulse
        }
        self.set_phase(MotionPhase::Homing);

        // C: home direction is inverted when MRES is negative
        // if ((MIP_HOMF && mres>0) || (MIP_HOMR && mres<0)) => HOME_FOR else HOME_REV
        let hw_forward = if self.conv.mres >= 0.0 { forward } else { !forward };

        // CDIR for homing
        self.stat.cdir = forward;

        effects.commands.push(MotorCommand::Home {
            forward: hw_forward,
            velocity: self.vel.hvel,
            acceleration: self.vel.accl,
        });
        effects.request_poll = true;
        effects.suppress_forward_link = true;
    }

    /// Handle tweak (TWF/TWR).
    fn handle_tweak(&mut self, forward: bool, effects: &mut ProcessEffects) {
        if forward {
            self.ctrl.twf = false; // pulse
        } else {
            self.ctrl.twr = false; // pulse
        }

        let dir = if forward {
            MotionDirection::Positive
        } else {
            MotionDirection::Negative
        };
        if self.is_blocked_by_hw_limit(dir) {
            return;
        }

        let delta = if forward {
            self.ctrl.twv
        } else {
            -self.ctrl.twv
        };
        self.pos.val += delta;

        // Cascade from VAL
        if let Ok((dval, rval, off)) = coordinate::cascade_from_val(
            self.pos.val,
            self.conv.dir,
            self.pos.off,
            self.conv.foff,
            self.conv.mres,
            false,
            self.pos.dval,
        ) {
            self.pos.dval = dval;
            self.pos.rval = rval;
            self.pos.off = off;
        }

        self.plan_absolute_move(effects);
    }

    /// Handle SPMG mode change.
    fn handle_spmg_change(&mut self, effects: &mut ProcessEffects) {
        let old = self.internal.lspg;
        let new = self.ctrl.spmg;
        self.internal.lspg = new;

        match new {
            SpmgMode::Stop => {
                if self.stat.phase != MotionPhase::Idle {
                    self.internal.backlash_pending = false;
                    self.internal.pending_retarget = None;
                    effects.commands.push(MotorCommand::Stop {
                        acceleration: self.vel.accl,
                    });
                    // Sync VAL = RBV
                    self.pos.val = self.pos.rbv;
                    self.pos.dval = self.pos.drbv;
                    self.pos.rval = self.pos.rrbv;
                    self.finalize_motion(effects);
                }
            }
            SpmgMode::Pause => {
                if self.stat.phase != MotionPhase::Idle {
                    // C: Pause sends STOP and sets MIP_STOP, but does NOT
                    // clear phase/MIP or set DMOV here. The normal stop
                    // completion pipeline handles that. DVAL is preserved
                    // for potential resume via Go.
                    self.stat.mip.insert(MipFlags::STOP);
                    self.internal.pending_retarget = None;
                    effects.commands.push(MotorCommand::Stop {
                        acceleration: self.vel.accl,
                    });
                }
            }
            SpmgMode::Go => {
                // Resume: if coming from Pause and there's a saved target, replan
                if matches!(old, SpmgMode::Pause) && self.stat.phase == MotionPhase::Idle {
                    if (self.pos.dval - self.pos.drbv).abs() > self.retry.rdbd.max(1e-12) {
                        self.plan_absolute_move(effects);
                    }
                }
            }
            SpmgMode::Move => {
                // One-shot: like Go but will restore to Pause after completion
                if matches!(old, SpmgMode::Pause | SpmgMode::Stop)
                    && self.stat.phase == MotionPhase::Idle
                {
                    if (self.pos.dval - self.pos.drbv).abs() > self.retry.rdbd.max(1e-12) {
                        self.plan_absolute_move(effects);
                    }
                }
            }
        }
    }

    /// Handle retarget (NTM) -- new target while moving.
    pub fn handle_retarget(&mut self, new_dval: f64) -> RetargetAction {
        if !self.timing.ntm {
            return RetargetAction::Ignore;
        }

        // Only retarget during active move or retry phases
        let in_move = self.stat.mip.intersects(MipFlags::MOVE | MipFlags::RETRY);
        if !in_move || self.stat.mip.contains(MipFlags::STOP) {
            return RetargetAction::Ignore;
        }

        let diff = new_dval - self.pos.drbv;
        let deadband = self.timing.ntmf * (self.retry.bdst.abs() + self.retry.rdbd);

        // C: retarget only if direction changed AND error exceeds deadband
        let sign_diff = if diff >= 0.0 { true } else { false };
        let direction_changed = sign_diff != self.stat.cdir;

        if direction_changed && diff.abs() > deadband {
            RetargetAction::StopAndReplan
        } else if !direction_changed {
            // Same direction: extend the move without stopping
            RetargetAction::ExtendMove
        } else {
            // Direction changed but within deadband: ignore
            RetargetAction::Ignore
        }
    }

    /// Check if a new command can be accepted.
    pub fn can_accept_command(&self) -> bool {
        matches!(self.ctrl.spmg, SpmgMode::Go | SpmgMode::Move)
    }

    /// Check if a hardware limit blocks motion in the given direction.
    fn is_blocked_by_hw_limit(&self, dir: MotionDirection) -> bool {
        match dir {
            MotionDirection::Positive => self.limits.hls,
            MotionDirection::Negative => self.limits.lls,
        }
    }

    /// Process the motor record (called by EPICS record support).
    pub fn do_process(&mut self) -> ProcessEffects {
        // STUP: one-shot status refresh
        if self.stat.stup > 0 {
            self.stat.stup = 0;
            let mut effects = ProcessEffects::default();
            effects.status_refresh = true;
            return effects;
        }

        let event = self.pending_event.take();
        let src = self.last_write.take();

        // User write takes priority: if a field was put while a poll
        // update arrived, handle the write first. The poll status was
        // already applied in determine_event() for Idle phase.
        if let Some(src) = src {
            // If there was also a DeviceUpdate, apply it first so
            // plan_motion sees the latest readback.
            if let Some(MotorEvent::DeviceUpdate(status)) = &event {
                self.process_motor_info(status);
            }
            return self.plan_motion(src);
        }

        match event {
            Some(MotorEvent::Startup) => {
                // Handled by device support init
                ProcessEffects::default()
            }
            Some(MotorEvent::UserWrite(cmd_src)) => self.plan_motion(cmd_src),
            Some(MotorEvent::DeviceUpdate(status)) => {
                self.process_motor_info(&status);
                self.check_completion()
            }
            Some(MotorEvent::DelayExpired) => {
                let mut effects = ProcessEffects::default();
                self.finalize_motion(&mut effects);
                effects
            }
            None => ProcessEffects::default(),
        }
    }
}
