use bevy::prelude::*;
use gr2_controller::locomotion::{WalkingConfig, walking_velocity};

fn close(actual: Vec3, expected: Vec3) {
    assert!(
        actual.distance(expected) < 0.00001,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn directional_friction_turns_before_input_acceleration_and_preserves_vertical() {
    let config = WalkingConfig {
        ground_friction: 2.0,
        max_acceleration: 2.0,
        ..default()
    };
    close(
        walking_velocity(Vec3::new(4.0, 7.0, 0.0), Vec3::Z, 10.0, 0.1, config),
        Vec3::new(3.2, 7.0, 1.0),
    );
}

#[test]
fn analog_input_scales_acceleration_and_speed_limit() {
    close(
        walking_velocity(Vec3::ZERO, Vec3::X * 0.25, 10.0, 0.1, default()),
        Vec3::X * 2.5,
    );
}

#[test]
fn braking_below_limit_restores_speed_only_with_supporting_input() {
    let config = WalkingConfig {
        ground_friction: 0.0,
        braking_deceleration: 300.0,
        max_acceleration: 100.0,
        ..default()
    };
    close(
        walking_velocity(Vec3::X * 12.0, Vec3::X, 10.0, 0.01, config),
        Vec3::X * 10.0,
    );
    close(
        walking_velocity(Vec3::X * 12.0, Vec3::NEG_X, 10.0, 0.01, config),
        Vec3::X * 8.0,
    );
}

#[test]
fn supported_overspeed_is_braked_then_capped_before_acceleration() {
    let config = WalkingConfig {
        ground_friction: 0.0,
        braking_deceleration: 2.0,
        max_acceleration: 100.0,
        ..default()
    };
    close(
        walking_velocity(Vec3::X * 20.0, Vec3::X, 10.0, 0.01, config),
        Vec3::X * 19.98,
    );
}

#[test]
fn native_squared_speed_tolerance_selects_the_braking_branch() {
    let config = WalkingConfig {
        ground_friction: 0.0,
        braking_deceleration: 100.0,
        max_acceleration: 100.0,
        ..default()
    };
    // 10.01² < 10²*1.01: ordinary acceleration, without braking.
    close(
        walking_velocity(Vec3::X * 10.01, Vec3::NEG_X, 10.0, 0.01, config),
        Vec3::X * 9.01,
    );
    // 10.06² > 10²*1.01: first brake, then add opposing acceleration.
    close(
        walking_velocity(Vec3::X * 10.06, Vec3::NEG_X, 10.0, 0.01, config),
        Vec3::X * 8.06,
    );
}

#[test]
fn braking_substeps_split_the_last_pair_instead_of_leaving_a_tiny_tail() {
    let config = WalkingConfig {
        ground_friction: 2.0,
        braking_friction_factor: 1.0,
        braking_deceleration: 0.0,
        braking_substep_time: 0.03,
        ..default()
    };
    // Native split: .03, .03, .02, .02. A .03,.03,.03,.01 split differs.
    let expected = 10.0 * 0.94 * 0.94 * 0.96 * 0.96;
    close(
        walking_velocity(Vec3::X * 10.0, Vec3::ZERO, 10.0, 0.1, config),
        Vec3::X * expected,
    );
}

#[test]
fn constant_braking_has_a_stop_threshold_but_friction_only_does_not() {
    let config = WalkingConfig {
        ground_friction: 0.0,
        braking_deceleration: 1.0,
        ..default()
    };
    assert_eq!(
        walking_velocity(Vec3::X * 0.11, Vec3::ZERO, 10.0, 0.02, config),
        Vec3::ZERO,
    );
    let friction_only = WalkingConfig {
        ground_friction: 1.0,
        braking_friction_factor: 1.0,
        braking_deceleration: 0.0,
        ..default()
    };
    close(
        walking_velocity(Vec3::X * 0.05, Vec3::ZERO, 10.0, 0.01, friction_only),
        Vec3::X * 0.0495,
    );
}

#[test]
fn separate_braking_friction_and_factor_are_applied_together() {
    let config = WalkingConfig {
        ground_friction: 8.0,
        separate_braking_friction: Some(2.0),
        braking_friction_factor: 3.0,
        braking_deceleration: 0.0,
        ..default()
    };
    close(
        walking_velocity(Vec3::X * 10.0, Vec3::ZERO, 10.0, 0.01, config),
        Vec3::X * 9.4,
    );
}

#[test]
fn braking_cannot_reverse_the_original_velocity() {
    let config = WalkingConfig {
        ground_friction: 0.0,
        braking_deceleration: 100.0,
        ..default()
    };
    assert_eq!(
        walking_velocity(Vec3::X, Vec3::ZERO, 10.0, 1.0, config),
        Vec3::ZERO,
    );
}
