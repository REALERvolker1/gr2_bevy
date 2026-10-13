//! Ordinary horizontal walking arithmetic from GR2 build16083277.
//!
//! Scope: native CalcVelocity 0x382baa0 through GR2 delegate 0x1c54cf0,
//! without root motion, requested/path-following movement, fluid drag, or forced
//! maximum acceleration. The caller supplies an already clamped analog input.
//! Keeping the host Y velocity untouched separates this horizontal boundary
//! from gravity, launch, floor integration and the Avian collision motor.
use bevy::prelude::*;

const MIN_TICK: f32 = 1e-6;
const EXCEED_SPEED_SQUARED_FACTOR: f32 = 1.01;
// Native squared-length tolerances converted from cm² to m².
const NORMALIZE_TOLERANCE_SQUARED: f32 = 1e-12;
const BRAKING_ZERO_SPEED_SQUARED: f32 = 1e-8;
const BRAKING_STOP_SPEED_SQUARED: f32 = 0.01;
const MIN_CLAMP_SPEED: f32 = 1e-6;

#[derive(Clone, Copy, Debug)]
pub struct WalkingConfig {
    pub ground_friction: f32,
    pub braking_friction_factor: f32,
    /// Constant braking deceleration in m/s².
    pub braking_deceleration: f32,
    /// Maximum input acceleration in m/s².
    pub max_acceleration: f32,
    pub braking_substep_time: f32,
    pub minimum_analog_speed: f32,
    /// `None` uses ground friction, matching disabled separate braking friction.
    pub separate_braking_friction: Option<f32>,
}

impl Default for WalkingConfig {
    fn default() -> Self {
        Self {
            // Native CharacterMovement constructor 0x3828540 offsets+16c,
            // +1a8,+1b4,+1b0,+1dc; cooked CharMoveComp export309 MaxAcceleration.
            ground_friction: 8.0,
            braking_friction_factor: 2.0,
            braking_deceleration: 20.48,
            max_acceleration: 500.0,
            braking_substep_time: 0.030303031,
            minimum_analog_speed: 0.0,
            separate_braking_friction: None,
        }
    }
}

pub fn walking_velocity(
    current: Vec3,
    input: Vec3,
    maximum_speed: f32,
    dt: f32,
    config: WalkingConfig,
) -> Vec3 {
    if dt < MIN_TICK {
        return current;
    }
    let mut velocity = Vec2::new(current.x, current.z);
    let input = Vec2::new(input.x, input.z);
    debug_assert!(
        input.length_squared() <= 1.0001,
        "analog input must be clamped"
    );
    let acceleration = input * config.max_acceleration.max(0.0);
    let analog = if config.max_acceleration > 0.0 {
        input.length()
    } else {
        0.0
    };
    let maximum_speed = (maximum_speed * analog).max(config.minimum_analog_speed);
    let friction = config.ground_friction.max(0.0);
    let no_acceleration = acceleration == Vec2::ZERO;
    let over_maximum = exceeds_speed(velocity, maximum_speed);

    if no_acceleration || over_maximum {
        let previous = velocity;
        velocity = brake(velocity, dt, config);
        // CalcVelocity restores the target speed only when braking crosses
        // below it from overspeed and input supports the original direction.
        if over_maximum
            && velocity.length_squared() < maximum_speed * maximum_speed
            && previous.dot(acceleration) > 0.0
        {
            velocity = safe_normalize(previous) * maximum_speed;
        }
    } else {
        let direction = safe_normalize(acceleration);
        velocity -= (velocity - direction * velocity.length()) * (dt * friction).min(1.0);
    }

    if !no_acceleration {
        // Preserve supported overspeed while adding input: the cap is selected
        // after braking/turning, before acceleration, using the source1.01 test.
        let cap = if exceeds_speed(velocity, maximum_speed) {
            velocity.length()
        } else {
            maximum_speed
        };
        velocity += acceleration * dt;
        velocity = clamp_speed(velocity, cap);
    }
    Vec3::new(velocity.x, current.y, velocity.y)
}

fn exceeds_speed(velocity: Vec2, speed: f32) -> bool {
    // Native vtable+438 ->0x3869fe0, multiplier at readonly RVA0x489d1c0.
    let speed = speed.max(0.0);
    velocity.length_squared() > speed * speed * EXCEED_SPEED_SQUARED_FACTOR
}

fn safe_normalize(vector: Vec2) -> Vec2 {
    let squared = vector.length_squared();
    if squared < NORMALIZE_TOLERANCE_SQUARED {
        Vec2::ZERO
    } else {
        // Portable square root is a host numerical adaptation of the native
        // rsqrtss plus two Newton refinements; bitwise source parity is unproved.
        vector / squared.sqrt()
    }
}

fn clamp_speed(velocity: Vec2, maximum: f32) -> Vec2 {
    if maximum < MIN_CLAMP_SPEED {
        Vec2::ZERO
    } else if velocity.length_squared() > maximum * maximum {
        safe_normalize(velocity) * maximum
    } else {
        velocity
    }
}

fn brake(mut velocity: Vec2, mut remaining: f32, config: WalkingConfig) -> Vec2 {
    // ApplyVelocityBraking 0x382af20: its fixed reverse acceleration uses the
    // original direction, while friction uses velocity from each substep.
    let friction = (config
        .separate_braking_friction
        .unwrap_or(config.ground_friction)
        * config.braking_friction_factor.max(0.0))
    .max(0.0);
    let deceleration = config.braking_deceleration.max(0.0);
    if velocity == Vec2::ZERO || (friction == 0.0 && deceleration == 0.0) {
        return velocity;
    }
    let original = velocity;
    let reverse_acceleration = safe_normalize(original) * -deceleration;
    let maximum_substep = config.braking_substep_time.clamp(0.013333334, 0.05);
    loop {
        let step = if remaining > maximum_substep && friction != 0.0 {
            maximum_substep.min(remaining * 0.5)
        } else {
            remaining
        };
        remaining -= step;
        velocity += (velocity * -friction + reverse_acceleration) * step;
        if velocity.dot(original) <= 0.0 {
            return Vec2::ZERO;
        }
        if remaining < MIN_TICK {
            break;
        }
    }
    let squared = velocity.length_squared();
    if squared <= BRAKING_ZERO_SPEED_SQUARED
        || (deceleration != 0.0 && squared <= BRAKING_STOP_SPEED_SQUARED)
    {
        Vec2::ZERO
    } else {
        velocity
    }
}
