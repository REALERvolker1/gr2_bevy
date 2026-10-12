//! Original LaunchCharacter and air-release arithmetic, in Bevy coordinates.
use bevy::prelude::*;

/// Concrete vtable +0x7d8, RVA 0x37f9e70. Actual application uses pending launch.
pub fn resolve_launch(
    current: Vec3,
    requested: Vec3,
    horizontal_override: bool,
    vertical_override: bool,
) -> Vec3 {
    let horizontal = if horizontal_override {
        Vec3::ZERO
    } else {
        Vec3::new(current.x, 0.0, current.z)
    };
    let vertical = if vertical_override { 0.0 } else { current.y };
    requested + horizontal + Vec3::Y * vertical
}

/// RVA 0x1c5b750. Incoming speed and forces have been converted from centimetres.
/// Source clears movement velocity before queuing this velocity-change impulse.
pub fn air_dash_release(
    aim: Vec3,
    incoming_speed: f32,
    forward_force: f32,
    upward_force: f32,
    gravity_multiplier: f32,
    special_ability_multiplier: f32,
) -> Vec3 {
    let speed_factor = (incoming_speed * (100.0 * 0.00083333335)).clamp(1.0, 1.5);
    let factor = 1.0 + special_ability_multiplier;
    Vec3::new(
        aim.x * forward_force * factor * speed_factor,
        aim.y * upward_force * factor * gravity_multiplier,
        aim.z * forward_force * factor * speed_factor,
    )
}

/// Ordinary native first jump. Input direction uses full 3D speed, not planar speed.
pub fn first_jump(
    current: Vec3,
    first_velocity: f32,
    velocity_multiplier: f32,
    forward_multiplier: f32,
) -> Vec3 {
    let direction = current.normalize_or_zero();
    let planar_bonus = first_velocity * (forward_multiplier - 1.0);
    let requested = Vec3::new(
        direction.x * planar_bonus,
        first_velocity * velocity_multiplier,
        direction.z * planar_bonus,
    );
    resolve_launch(current, requested, false, true)
}

/// Native DoWallJump/helper. Both stored wall vectors use full 3D normalization.
pub fn wall_jump(
    current: Vec3,
    wall_forward: Vec3,
    wall_normal: Vec3,
    forward_strength: f32,
    normal_strength: f32,
    height: f32,
) -> Vec3 {
    let forward = wall_forward.normalize_or_zero();
    let normal = wall_normal.normalize_or_zero();
    resolve_launch(
        current,
        Vec3::new(
            forward.x * forward_strength + normal.x * normal_strength,
            height,
            forward.z * forward_strength + normal.z * normal_strength,
        ),
        false,
        true,
    )
}
