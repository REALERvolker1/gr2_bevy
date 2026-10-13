//! Native UClimbComponent mounting arithmetic, in metres and Bevy coordinates.
//! Source build 16083277: TryClimb 0x1a49ac0, update 0x1a442c0.
use bevy::prelude::*;

/// Authored host marker replacing source collision channels, actor classes and tags.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MantleSurface;

/// Cooked BP_PlayerCharacter export 267, with omitted native constructor defaults.
#[derive(Clone, Copy, Debug)]
pub struct MantleConfig {
    pub max_climb_height: f32,
    pub max_wall_forward: f32,
    pub max_empty_top_forward: f32,
    pub additional_height_offset: f32,
    pub climb_speed: f32,
    pub move_forward_offset: f32,
    pub move_forward_speed: f32,
    pub bottom_ledge_max_distance: f32,
    pub ledge_depth_check_offset: f32,
    pub finish_speed: f32,
}

impl Default for MantleConfig {
    fn default() -> Self {
        Self {
            max_climb_height: 3.0,
            max_wall_forward: 0.5,
            max_empty_top_forward: 3.0,
            additional_height_offset: 0.0,
            climb_speed: 15.0,
            move_forward_offset: 0.0,
            move_forward_speed: 25.0,
            bottom_ledge_max_distance: 0.8,
            ledge_depth_check_offset: 0.0,
            finish_speed: 10.0,
        }
    }
}

impl MantleConfig {
    /// AdjustToCapsule 0x1a1c660 replaces the cooked values during player setup.
    pub fn adjusted_to_capsule(mut self, radius: f32, half_height: f32) -> Self {
        self.max_wall_forward = radius * 1.5;
        self.additional_height_offset = half_height * 1.5 + 0.2;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MantleError {
    NonFiniteGeometry,
    InvalidCapsuleRadius,
    InvalidSpeed,
    InvalidSurfaceOrigin,
    MissingForwardDirection,
    NonPositiveLiftDistance,
    NonPositiveForwardDistance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MantleStep {
    Position(Vec3),
    Finished { velocity: Vec3 },
    Paused,
    Inactive,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Lift { alpha: f32 },
    Forward { alpha: f32 },
    Finished,
}

#[derive(Clone, Debug)]
pub struct Mantle {
    start: Vec3,
    lift_target: Vec3,
    forward: Vec3,
    lift_distance: f32,
    forward_distance: f32,
    previous_surface_origin: Vec3,
    config: MantleConfig,
    phase: Phase,
}

impl Mantle {
    /// Source target and distance arithmetic; collision admission belongs to the caller.
    /// Finite, positive geometry validation is a host boundary, not a source epsilon.
    pub fn from_impact(
        start: Vec3,
        impact: Vec3,
        actor_forward: Vec3,
        surface_origin: Vec3,
        capsule_radius: f32,
        config: MantleConfig,
    ) -> Result<Self, MantleError> {
        if !start.is_finite()
            || !impact.is_finite()
            || !actor_forward.is_finite()
            || !surface_origin.is_finite()
            || !config.additional_height_offset.is_finite()
            || !config.move_forward_offset.is_finite()
        {
            return Err(MantleError::NonFiniteGeometry);
        }
        if !capsule_radius.is_finite() || capsule_radius <= 0.0 {
            return Err(MantleError::InvalidCapsuleRadius);
        }
        if [
            config.climb_speed,
            config.move_forward_speed,
            config.finish_speed,
        ]
        .iter()
        .any(|speed| !speed.is_finite() || *speed <= 0.0)
        {
            return Err(MantleError::InvalidSpeed);
        }
        // 0x1a287e0 returns zero for invalid references; TryClimb rejects that result.
        if surface_origin == Vec3::ZERO {
            return Err(MantleError::InvalidSurfaceOrigin);
        }
        let forward = Vec3::new(actor_forward.x, 0.0, actor_forward.z).normalize_or_zero();
        if forward == Vec3::ZERO {
            return Err(MantleError::MissingForwardDirection);
        }
        let lift_target = Vec3::new(start.x, impact.y + config.additional_height_offset, start.z);
        let lift_distance = lift_target.distance(start);
        let forward_distance =
            lift_target.distance(impact) + config.move_forward_offset - capsule_radius;
        if !lift_distance.is_finite() || !forward_distance.is_finite() {
            return Err(MantleError::NonFiniteGeometry);
        }
        if lift_distance <= 0.0 {
            return Err(MantleError::NonPositiveLiftDistance);
        }
        if forward_distance <= 0.0 {
            return Err(MantleError::NonPositiveForwardDistance);
        }
        Ok(Self {
            start,
            lift_target,
            forward,
            lift_distance,
            forward_distance,
            previous_surface_origin: surface_origin,
            config,
            phase: Phase::Lift { alpha: 0.0 },
        })
    }

    pub fn lift_target(&self) -> Vec3 {
        self.lift_target
    }

    pub fn forward_distance(&self) -> f32 {
        self.forward_distance
    }

    pub fn alphas(&self) -> (f32, f32) {
        match self.phase {
            Phase::Lift { alpha } => (alpha, 0.0),
            Phase::Forward { alpha } => (1.0, alpha),
            Phase::Finished => (1.0, 1.0),
        }
    }

    /// Update 0x1a442c0 handles one phase per call, preserving its boundary delays.
    pub fn tick(
        &mut self,
        actual_position: Vec3,
        surface_origin: Vec3,
        actor_forward: Vec3,
        delta: f32,
        player_factor: f32,
    ) -> MantleStep {
        if matches!(self.phase, Phase::Finished) {
            return MantleStep::Inactive;
        }
        // Host input validation prevents invalid engine/query values entering the core.
        if !actual_position.is_finite()
            || !surface_origin.is_finite()
            || !actor_forward.is_finite()
            || !delta.is_finite()
            || !player_factor.is_finite()
            || delta < 0.0
            || player_factor < 0.0
        {
            return MantleStep::Paused;
        }
        match self.phase {
            Phase::Lift { alpha } => {
                let alpha = (alpha
                    + delta * self.config.climb_speed * player_factor / self.lift_distance)
                    .clamp(0.0, 1.0);
                self.phase = if alpha >= 1.0 {
                    Phase::Forward { alpha: 0.0 }
                } else {
                    Phase::Lift { alpha }
                };
                MantleStep::Position(self.start.lerp(self.lift_target, alpha))
            }
            Phase::Forward { alpha } if alpha >= 1.0 => {
                self.phase = Phase::Finished;
                MantleStep::Finished {
                    velocity: actor_forward * self.config.finish_speed,
                }
            }
            Phase::Forward { alpha } => {
                let alpha = (alpha
                    + delta * self.config.move_forward_speed * player_factor
                        / self.forward_distance)
                    .clamp(0.0, 1.0);
                self.phase = Phase::Forward { alpha };
                // The native update advances alpha before rejecting a lost surface origin.
                if surface_origin == Vec3::ZERO {
                    return MantleStep::Paused;
                }
                let base = Vec3::new(self.start.x, actual_position.y, self.start.z);
                let target = base + self.forward * self.forward_distance + surface_origin
                    - self.previous_surface_origin;
                self.previous_surface_origin = surface_origin;
                MantleStep::Position(base.lerp(target, alpha))
            }
            Phase::Finished => MantleStep::Inactive,
        }
    }
}
