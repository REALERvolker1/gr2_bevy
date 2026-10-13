//! Ordinary fixed-point grapple arithmetic recovered from build 16083277.
//! Avian sweeps, target selection and montage timing belong to the caller.
use crate::curves::{Curve, Interpolation, Key};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct GrappleConfig {
    pub speed: f32,
    pub release_extent: f32,
    pub launch_power: f32,
    pub min_play_rate: f32,
    pub max_play_rate: f32,
    pub maximum_range: f32,
}

impl Default for GrappleConfig {
    fn default() -> Self {
        Self {
            // Native constructor 0x1c88f80, plus Grapling_Point's 4000 cm override.
            speed: 13.0,
            release_extent: 2.0,
            launch_power: 20.0,
            min_play_rate: 0.6,
            max_play_rate: 2.5,
            maximum_range: 40.0,
        }
    }
}

/// Host level marker. Target is the endpoint component position, not the mesh origin.
#[derive(Component, Clone, Copy, Debug)]
pub struct GrappleAnchor {
    pub target: Vec3,
    pub blocked: bool,
    pub enabled: bool,
    /// Host fixed-clock mapping of the source point cooldown/reset timers.
    pub available_after: f32,
    pub occupied: bool,
}

impl Default for GrappleAnchor {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            blocked: false,
            enabled: true,
            available_after: 0.0,
            occupied: false,
        }
    }
}

/// Native IsHookAvailableR guards after source selection has chosen a point.
#[derive(Clone, Copy, Debug)]
pub struct GrappleAdmission {
    pub point_exists: bool,
    pub point_grappling: bool,
    pub player_grappling: bool,
    pub blocked: bool,
    pub point_state: u8,
    pub screen_position: Vec2,
    pub viewport_size: Vec2,
    pub is_previous_point: bool,
    pub previous_point_available: bool,
}

impl GrappleAdmission {
    /// RVA 0x1ca39d0. Visibility/range ranking and external branches are separate.
    pub fn allows(self) -> bool {
        self.point_exists
            && !self.point_grappling
            && !self.player_grappling
            && !self.blocked
            && self.point_state == 0
            && self.screen_position.cmpge(Vec2::ZERO).all()
            && self.screen_position.cmplt(self.viewport_size).all()
            && (!self.is_previous_point || self.previous_point_available)
    }
}

// Exact unweighted cubic keys from /Game/Blueprints/FloatCurves/GrapplingPointCurve.
#[allow(clippy::excessive_precision)]
pub const GRAPPLE_CURVE: Curve = Curve {
    length: 0.5,
    ignore_time_dilation: false,
    keys: &[
        Key {
            time: 0.0,
            value: 0.0,
            arrive_tangent: 7.6666669845581055,
            leave_tangent: 0.0,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.05999999865889549,
            value: 0.46000000834465027,
            arrive_tangent: 2.7978944778442383,
            leave_tangent: 2.7978944778442383,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.5,
            value: 1.0,
            arrive_tangent: -0.03536045551300049,
            leave_tangent: -0.03536045551300049,
            interpolation: Interpolation::Cubic,
        },
    ],
};

#[derive(Clone, Copy, Debug)]
pub struct Grapple {
    pub start: Vec3,
    pub target: Vec3,
    pub direction: Vec3,
    pub elapsed: f32,
    pub play_rate: f32,
    pub point: Entity,
    release_extent: f32,
    launch_power: f32,
}

impl Grapple {
    /// Point start RVA 0x1c9fcc0; capture the ordinary endpoint once.
    pub fn new(
        start: Vec3,
        target: Vec3,
        point: Entity,
        config: GrappleConfig,
        custom_time_dilation: f32,
    ) -> Option<Self> {
        let delta = target - start;
        let distance = delta.length();
        // Host validation keeps division/clamp finite for malformed authored targets
        // or configuration. The original assumes a valid selected point.
        if !start.is_finite()
            || !target.is_finite()
            || !distance.is_finite()
            || distance <= 0.0
            || !custom_time_dilation.is_finite()
            || custom_time_dilation <= 0.0
            || !config.speed.is_finite()
            || config.speed <= 0.0
            || !config.min_play_rate.is_finite()
            || config.min_play_rate <= 0.0
            || !config.max_play_rate.is_finite()
            || config.max_play_rate < config.min_play_rate
            || !config.release_extent.is_finite()
            || config.release_extent < 0.0
            || !config.launch_power.is_finite()
            || config.launch_power < 0.0
        {
            return None;
        }
        let play_rate = (config.speed / distance).clamp(config.min_play_rate, config.max_play_rate)
            * custom_time_dilation;
        if !play_rate.is_finite() || play_rate <= 0.0 {
            return None;
        }
        // Source normalizes only above 0.0001 cm², mapped to 1e-8 m².
        let direction = if delta.length_squared() > 1e-8 {
            delta / distance
        } else {
            delta
        };
        Some(Self {
            start,
            target,
            direction,
            elapsed: 0.0,
            play_rate,
            point,
            release_extent: config.release_extent,
            launch_power: config.launch_power,
        })
    }

    /// Advance in caller-supplied timeline seconds, then request a swept move.
    pub fn desired_position(&mut self, dt: f32) -> Vec3 {
        // Host clock validation ignores malformed deltas rather than poisoning poses.
        if dt.is_finite() && dt >= 0.0 {
            self.elapsed = (self.elapsed + dt * self.play_rate).min(GRAPPLE_CURVE.length);
        }
        self.start + (self.target - self.start) * GRAPPLE_CURVE.evaluate(self.elapsed)
    }

    /// RVA 0x1cb18f0 tests the post-sweep pose against a componentwise box.
    pub fn should_release(&self, actual_pose: Vec3) -> bool {
        (actual_pose - self.target)
            .abs()
            .cmple(Vec3::splat(self.release_extent))
            .all()
    }

    /// Caller applies both XY and Z overrides, as verified at RVA 0x1cb1b1f.
    pub fn release_velocity(&self) -> Vec3 {
        self.direction * self.launch_power
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= GRAPPLE_CURVE.length
    }
}
