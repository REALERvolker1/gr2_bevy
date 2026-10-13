//! Cooked BP_PlayerCharacter speed target timelines, build 16083277.
//! Axis DoOnce gates, native modifier writes and landing/wall guards stay with the caller.
use crate::curves::{Curve, FORWARD_SPEED, Interpolation, Key, OTHER_SPEED};
use bevy::prelude::*;

const FORWARD_CAPTURE_FACTOR: f32 = 0.8;
const CAPTURE_CAP: f32 = 12.0;
const FORWARD_CLAMP_MAX: f32 = 30.0;
const HIGH_SPEED_THRESHOLD: f32 = 10.0;
const OTHER_DECELERATION_THRESHOLD: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct SpeedConfig {
    pub forward_min: f32,
    pub forward_max: f32,
    pub other_min: f32,
    pub other_max: f32,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            forward_min: 7.0,
            forward_max: 10.0,
            other_min: 5.0,
            other_max: 7.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedInput {
    Forward,
    Other,
    /// Source resets axis gates here; it does not itself stop the running timeline.
    NoNewTimeline,
}

/// Raw source axes, X right and Y forward. VM 1649 and 16541..16804.
pub fn classify_input(axes: Vec2) -> SpeedInput {
    if axes.y >= 0.5 {
        SpeedInput::Forward
    } else if axes.y < 0.0 || (axes.y >= 0.0 && axes.y < 0.2 && axes.x != 0.0) {
        SpeedInput::Other
    } else {
        SpeedInput::NoNewTimeline
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedProfile {
    Unchanged,
    Forward,
    ForwardRecovery,
    OtherAcceleration,
    OtherDeceleration,
}

// Player template export435, curve275: high incoming speed selects Timeline_5.
#[allow(clippy::excessive_precision)]
pub const FORWARD_RECOVERY: Curve = Curve {
    length: 1.5,
    ignore_time_dilation: false,
    keys: &[
        Key {
            time: 0.0,
            value: 0.0,
            arrive_tangent: 0.0,
            leave_tangent: 0.0,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.6307741403579712,
            value: 0.09813332557678223,
            arrive_tangent: 0.35970187187194824,
            leave_tangent: 0.35970187187194824,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 1.149470329284668,
            value: 0.4134666323661804,
            arrive_tangent: 1.0375516414642334,
            leave_tangent: 1.0375516414642334,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 1.5,
            value: 1.0,
            arrive_tangent: 2.146879196166992,
            leave_tangent: 2446.29150390625,
            interpolation: Interpolation::Cubic,
        },
    ],
};

// Template export418 lasts 2s; curve298 reaches its last key at 1s.
pub const OTHER_DECELERATION: Curve = Curve {
    length: 2.0,
    ignore_time_dilation: false,
    keys: &[
        Key {
            time: 0.0,
            value: 0.0,
            arrive_tangent: 0.0,
            leave_tangent: 0.0,
            interpolation: Interpolation::Linear,
        },
        Key {
            time: 1.0,
            value: 1.0,
            arrive_tangent: 0.0,
            leave_tangent: 0.0,
            interpolation: Interpolation::Linear,
        },
    ],
};

#[derive(Clone, Copy, Debug)]
pub struct SpeedState {
    pub profile: SpeedProfile,
    pub elapsed: f32,
    /// Native CurrentSpeed +0x704; captures are event-bound, not a per-tick velocity copy.
    pub captured_speed: f32,
}

impl Default for SpeedState {
    fn default() -> Self {
        Self {
            profile: SpeedProfile::Unchanged,
            elapsed: 0.0,
            // Native constructor RVA 0x1c4c470 initializes CurrentSpeed to 170 cm/s.
            captured_speed: 1.7,
        }
    }
}

impl SpeedState {
    /// VM16119..16536. Caller invokes once per accepted source axis gate.
    pub fn begin_forward(&mut self, full_speed: f32, just_used_slide: bool) {
        if !just_used_slide {
            self.captured_speed = (full_speed * FORWARD_CAPTURE_FACTOR).clamp(0.0, CAPTURE_CAP);
        }
        self.profile = if self.captured_speed >= HIGH_SPEED_THRESHOLD {
            SpeedProfile::ForwardRecovery
        } else {
            SpeedProfile::Forward
        };
        self.elapsed = 0.0;
    }

    /// VM17542 -> StopForwardAcceleration -> VM16971, with full incoming speed.
    pub fn begin_other(
        &mut self,
        full_speed: f32,
        hel_and_falling: bool,
        speed_modifier: f32,
        config: SpeedConfig,
    ) {
        self.stop_forward(speed_modifier, config);
        if full_speed >= OTHER_DECELERATION_THRESHOLD {
            if hel_and_falling {
                // Source pops execution flow: a prior Other timeline can remain
                // running because StopForwardAcceleration does not stop it.
                return;
            }
            self.captured_speed = full_speed;
            self.profile = SpeedProfile::OtherDeceleration;
        } else {
            self.profile = SpeedProfile::OtherAcceleration;
        }
        self.elapsed = 0.0;
    }

    /// VM17240..17518 resets CurrentSpeed only without a positive modifier.
    pub fn stop_forward(&mut self, speed_modifier: f32, config: SpeedConfig) {
        if speed_modifier <= 0.0 {
            self.captured_speed = config.forward_min;
        }
        if matches!(
            self.profile,
            SpeedProfile::Forward | SpeedProfile::ForwardRecovery
        ) {
            self.profile = SpeedProfile::Unchanged;
        }
    }

    pub fn stop_other(&mut self) {
        if matches!(
            self.profile,
            SpeedProfile::OtherAcceleration | SpeedProfile::OtherDeceleration
        ) {
            self.profile = SpeedProfile::Unchanged;
        }
    }

    fn curve(&self) -> Option<Curve> {
        match self.profile {
            SpeedProfile::Unchanged => None,
            SpeedProfile::Forward => Some(FORWARD_SPEED),
            SpeedProfile::ForwardRecovery => Some(FORWARD_RECOVERY),
            SpeedProfile::OtherAcceleration => Some(OTHER_SPEED),
            SpeedProfile::OtherDeceleration => Some(OTHER_DECELERATION),
        }
    }

    /// Forward's authored template ignores dilation; all other profiles respect it.
    pub fn advance(&mut self, unscaled_delta: f32, scaled_delta: f32) {
        let Some(curve) = self.curve() else {
            return;
        };
        let delta = if curve.ignore_time_dilation {
            unscaled_delta
        } else {
            scaled_delta
        };
        // Host validation excludes malformed clock values from an authoritative target.
        if delta.is_finite() && delta >= 0.0 {
            self.elapsed = (self.elapsed + delta).min(curve.length);
        }
    }

    /// MaxWalkSpeed writer equations. Caller suppresses Forward writes on walls
    /// or while LandingForwardAccelerationTimeline plays, as VM35719..35811 does.
    pub fn target(&self, config: SpeedConfig, speed_modifier: f32) -> Option<f32> {
        let curve = self.curve()?;
        let (from, to) = match self.profile {
            SpeedProfile::Forward => {
                if !(0.0..=FORWARD_CLAMP_MAX).contains(&config.forward_min) {
                    return None;
                }
                (
                    self.captured_speed
                        .clamp(config.forward_min, FORWARD_CLAMP_MAX),
                    config.forward_max,
                )
            }
            SpeedProfile::ForwardRecovery => (
                self.captured_speed.clamp(0.0, CAPTURE_CAP),
                config.forward_max,
            ),
            SpeedProfile::OtherAcceleration => (config.other_min, config.other_max),
            SpeedProfile::OtherDeceleration => {
                let upper = CAPTURE_CAP * (1.0 + speed_modifier);
                // Host configuration validation prevents an invalid clamp bound.
                if !upper.is_finite() || upper < 0.0 {
                    return None;
                }
                (self.captured_speed.clamp(0.0, upper), config.other_max)
            }
            SpeedProfile::Unchanged => return None,
        };
        let alpha = curve.evaluate(self.elapsed);
        let target = from + (to - from) * alpha;
        target.is_finite().then_some(target)
    }
}
