use bevy::prelude::*;
use gr2_controller::mantle::*;

fn config() -> MantleConfig {
    MantleConfig::default().adjusted_to_capsule(0.4, 0.9)
}

fn mantle() -> Mantle {
    Mantle::from_impact(
        Vec3::Y,
        Vec3::new(0.0, 1.0, -0.5),
        Vec3::NEG_Z,
        Vec3::new(0.0, 0.5, -1.0),
        0.4,
        config(),
    )
    .unwrap()
}

#[test]
fn capsule_adjustment_and_impact_geometry_follow_original_arithmetic() {
    let config = config();
    assert!((config.max_wall_forward - 0.6).abs() < 0.000001);
    assert!((config.additional_height_offset - 1.55).abs() < 0.000001);
    let mantle = mantle();
    assert!((mantle.lift_target().y - 2.55).abs() < 0.000001);
    let expected = Vec3::new(0.0, 1.55, 0.5).length() - 0.4;
    assert!((mantle.forward_distance() - expected).abs() < 0.000001);
}

#[test]
fn large_step_does_not_carry_lift_time_into_forward_or_finish() {
    let mut mantle = mantle();
    let origin = Vec3::new(0.0, 0.5, -1.0);
    let target = mantle.lift_target();
    assert_eq!(
        mantle.tick(Vec3::Y, origin, Vec3::NEG_Z, 10.0, 1.0),
        MantleStep::Position(target)
    );
    assert_eq!(mantle.alphas(), (1.0, 0.0));
    assert!(matches!(
        mantle.tick(target, origin, Vec3::NEG_Z, 10.0, 1.0),
        MantleStep::Position(_)
    ));
    assert_eq!(mantle.alphas(), (1.0, 1.0));
    assert_eq!(
        mantle.tick(target, origin, Vec3::NEG_Z, 0.0, 1.0),
        MantleStep::Finished {
            velocity: Vec3::NEG_Z * 10.0
        }
    );
    assert_eq!(
        mantle.tick(target, origin, Vec3::NEG_Z, 1.0, 1.0),
        MantleStep::Inactive
    );
}

#[test]
fn forward_phase_uses_actual_height_and_incremental_surface_motion() {
    let mut mantle = mantle();
    let initial_origin = Vec3::new(0.0, 0.5, -1.0);
    mantle.tick(Vec3::Y, initial_origin, Vec3::NEG_Z, 10.0, 1.0);
    let distance = mantle.forward_distance();
    let delta = distance / (config().move_forward_speed * 4.0);
    let actual = Vec3::new(0.1, 3.0, -0.1);
    let moved_origin = initial_origin + Vec3::X;
    let MantleStep::Position(first) = mantle.tick(actual, moved_origin, Vec3::NEG_Z, delta, 1.0)
    else {
        panic!("expected a forward position");
    };
    assert!(first.distance(Vec3::new(0.25, 3.0, -distance * 0.25)) < 0.000001);
    let MantleStep::Position(second) = mantle.tick(first, moved_origin, Vec3::NEG_Z, delta, 1.0)
    else {
        panic!("expected a forward position");
    };
    // An unchanged origin contributes no displacement on the second step.
    assert!(second.distance(Vec3::new(0.0, 3.0, -distance * 0.5)) < 0.000001);
}

#[test]
fn lost_surface_advances_forward_alpha_before_skipping_pose_commit() {
    let mut mantle = mantle();
    mantle.tick(Vec3::Y, Vec3::ONE, Vec3::NEG_Z, 10.0, 1.0);
    assert_eq!(
        mantle.tick(Vec3::Y, Vec3::ZERO, Vec3::NEG_Z, 10.0, 1.0),
        MantleStep::Paused
    );
    assert_eq!(mantle.alphas(), (1.0, 1.0));
    assert!(matches!(
        mantle.tick(Vec3::Y, Vec3::ZERO, Vec3::NEG_Z, 0.0, 1.0),
        MantleStep::Finished { .. }
    ));
}

#[test]
fn host_validation_rejects_nonpositive_distances_without_source_epsilon() {
    let flat = Mantle::from_impact(
        Vec3::Y,
        Vec3::Y,
        Vec3::NEG_Z,
        Vec3::ONE,
        0.4,
        MantleConfig::default(),
    );
    assert!(matches!(flat, Err(MantleError::NonPositiveLiftDistance)));
    let short = Mantle::from_impact(
        Vec3::ZERO,
        Vec3::Y * 0.1,
        Vec3::NEG_Z,
        Vec3::ONE,
        0.4,
        MantleConfig::default(),
    );
    assert!(matches!(
        short,
        Err(MantleError::NonPositiveForwardDistance)
    ));
}

#[test]
fn saved_planar_direction_uses_source_squared_length_tolerance() {
    let make = |forward| {
        Mantle::from_impact(
            Vec3::Y,
            Vec3::new(0.0, 1.0, -0.5),
            forward,
            Vec3::ONE,
            0.4,
            config(),
        )
    };
    assert!(matches!(
        make(Vec3::Y + Vec3::NEG_Z * 0.00005),
        Err(MantleError::MissingForwardDirection)
    ));
    let mut mantle = make(Vec3::Y + Vec3::NEG_Z * 0.0002).unwrap();
    let target = mantle.lift_target();
    let distance = mantle.forward_distance();
    mantle.tick(Vec3::Y, Vec3::ONE, Vec3::ZERO, 10.0, 1.0);
    let MantleStep::Position(position) = mantle.tick(target, Vec3::ONE, Vec3::ZERO, 10.0, 1.0)
    else {
        panic!("expected forward position");
    };
    assert!(position.distance(target + Vec3::NEG_Z * distance) < 0.000001);
}

#[test]
fn lift_ignores_pose_surface_and_finish_direction_inputs() {
    let mut mantle = mantle();
    let target = mantle.lift_target();
    let invalid = Vec3::splat(f32::NAN);
    assert_eq!(
        mantle.tick(invalid, invalid, invalid, 10.0, 1.0),
        MantleStep::Position(target)
    );
    // Forward consumes current height, not its X/Z or the finish direction.
    assert!(matches!(
        mantle.tick(
            Vec3::new(f32::NAN, target.y, f32::NAN),
            Vec3::new(0.0, 0.5, -1.0),
            invalid,
            10.0,
            1.0,
        ),
        MantleStep::Position(_)
    ));
}

#[test]
fn finish_consumes_only_direction_and_clears_only_source_reset_fields() {
    let mut mantle = mantle();
    let distance = mantle.forward_distance();
    mantle.tick(Vec3::Y, Vec3::ONE, Vec3::ZERO, 10.0, 1.0);
    mantle.tick(Vec3::Y, Vec3::ONE, Vec3::ZERO, 10.0, 1.0);
    let invalid = Vec3::splat(f32::NAN);
    assert_eq!(
        mantle.tick(invalid, invalid, invalid, f32::NAN, -1.0),
        MantleStep::Paused
    );
    assert_eq!(mantle.alphas(), (1.0, 1.0));
    assert_eq!(
        mantle.tick(invalid, invalid, Vec3::NEG_Z, f32::NAN, -1.0),
        MantleStep::Finished {
            velocity: Vec3::NEG_Z * 10.0,
        }
    );
    assert_eq!(mantle.alphas(), (0.0, 0.0));
    assert_eq!(mantle.lift_target(), Vec3::ZERO);
    // Native finish leaves F in +1B8; completed typestate prevents its reuse.
    assert_eq!(mantle.forward_distance(), distance);
    assert_eq!(
        mantle.tick(invalid, invalid, invalid, f32::NAN, f32::NAN),
        MantleStep::Inactive
    );
}

#[test]
fn zero_finish_speed_is_admitted_and_finishes_with_zero_velocity() {
    let mut config = config();
    config.finish_speed = 0.0;
    let mut mantle = Mantle::from_impact(
        Vec3::Y,
        Vec3::new(0.0, 1.0, -0.5),
        Vec3::NEG_Z,
        Vec3::ONE,
        0.4,
        config,
    )
    .unwrap();
    mantle.tick(Vec3::Y, Vec3::ONE, Vec3::ZERO, 10.0, 1.0);
    mantle.tick(Vec3::Y, Vec3::ONE, Vec3::ZERO, 10.0, 1.0);
    assert_eq!(
        mantle.tick(Vec3::Y, Vec3::ZERO, Vec3::NEG_Z, 0.0, 1.0),
        MantleStep::Finished {
            velocity: Vec3::ZERO,
        }
    );
}

#[test]
fn static_distant_surface_origin_cannot_change_forward_phase_height() {
    let origin = Vec3::splat(1_000_000.0);
    let mut motion = Mantle::from_impact(
        Vec3::Y,
        Vec3::new(0.0, 1.0, -0.5),
        Vec3::NEG_Z,
        origin,
        0.4,
        config(),
    )
    .unwrap();
    let MantleStep::Position(lift) = motion.tick(Vec3::Y, origin, Vec3::NEG_Z, 100.0, 1.0) else {
        panic!("lift")
    };
    let MantleStep::Position(forward) = motion.tick(lift, origin, Vec3::NEG_Z, 100.0, 1.0) else {
        panic!("forward")
    };
    assert_eq!(
        forward.y, lift.y,
        "source computes delta_origin before combining it with the actor pose"
    );
    assert!(forward.z < -1.0);
}
