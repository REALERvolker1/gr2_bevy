use bevy::prelude::*;
use gr2_controller::grapple::{
    GRAPPLE_CURVE, Grapple, GrappleAdmission, GrappleAnchor, GrappleConfig,
};

fn grapple(target: Vec3) -> Grapple {
    Grapple::new(
        Vec3::ZERO,
        target,
        Entity::PLACEHOLDER,
        GrappleConfig::default(),
        1.0,
    )
    .unwrap()
}

#[test]
fn source_curve_retains_keys_and_unweighted_hermite_shape() {
    for key in GRAPPLE_CURVE.keys {
        assert_eq!(GRAPPLE_CURVE.evaluate(key.time), key.value);
    }
    // First-interval midpoint includes the next key's arrive tangent.
    assert!((GRAPPLE_CURVE.evaluate(0.03) - 0.20901579).abs() < 0.000001);
    assert!((GRAPPLE_CURVE.evaluate(0.28) - 0.88582903).abs() < 0.000001);
}

#[test]
fn distance_sets_rate_with_native_clamps_before_custom_dilation() {
    assert_eq!(grapple(Vec3::X).play_rate, 2.5);
    assert_eq!(grapple(Vec3::X * 13.0).play_rate, 1.0);
    assert_eq!(grapple(Vec3::X * 40.0).play_rate, 0.6);
    let dilated = Grapple::new(
        Vec3::ZERO,
        Vec3::X * 40.0,
        Entity::PLACEHOLDER,
        GrappleConfig::default(),
        0.5,
    )
    .unwrap();
    assert_eq!(dilated.play_rate, 0.3);
}

#[test]
fn requested_pose_uses_captured_endpoint_and_curve_time() {
    let mut anchor = GrappleAnchor {
        target: Vec3::X * 13.0,
        ..GrappleAnchor::default()
    };
    let mut state = grapple(anchor.target);
    // Ordinary source travel retains the endpoint even if the level anchor moves.
    anchor.target = Vec3::Y * 30.0;
    assert_eq!(state.desired_position(0.0), Vec3::ZERO);
    let requested = state.desired_position(0.06);
    assert!((requested.x - 5.98).abs() < 0.00001);
    assert_eq!(state.target, Vec3::X * 13.0);
    assert_ne!(state.target, anchor.target);
    assert!(!state.finished());
    assert_eq!(state.desired_position(1.0), state.target);
    assert!(state.finished());
    assert_eq!(state.release_velocity(), Vec3::X * 20.0);
}

#[test]
fn release_uses_actual_pose_and_box_extent_instead_of_sphere() {
    let state = grapple(Vec3::new(10.0, 10.0, 10.0));
    let corner = state.target + Vec3::splat(2.0);
    assert!(corner.distance(state.target) > 2.0);
    assert!(state.should_release(corner));
    assert!(!state.should_release(state.target + Vec3::new(2.001, 0.0, 0.0)));
    // A requested endpoint alone cannot release a body obstructed far from it.
    assert!(!state.should_release(Vec3::ZERO));
}

#[test]
fn source_admission_keeps_viewport_edges_state_and_previous_point_gates() {
    let allowed = GrappleAdmission {
        point_exists: true,
        point_grappling: false,
        player_grappling: false,
        blocked: false,
        point_state: 0,
        screen_position: Vec2::ZERO,
        viewport_size: Vec2::new(1280.0, 720.0),
        is_previous_point: false,
        previous_point_available: false,
    };
    assert!(allowed.allows());
    assert!(
        !GrappleAdmission {
            screen_position: Vec2::new(1280.0, 0.0),
            ..allowed
        }
        .allows()
    );
    assert!(
        !GrappleAdmission {
            point_state: 1,
            ..allowed
        }
        .allows()
    );
    assert!(
        !GrappleAdmission {
            is_previous_point: true,
            ..allowed
        }
        .allows()
    );
    assert!(
        GrappleAdmission {
            is_previous_point: true,
            previous_point_available: true,
            ..allowed
        }
        .allows()
    );
}

#[test]
fn host_validation_rejects_degenerate_targets_and_rate_configuration() {
    for target in [
        Vec3::ZERO,
        Vec3::splat(f32::NAN),
        Vec3::splat(f32::INFINITY),
    ] {
        assert!(
            Grapple::new(
                Vec3::ZERO,
                target,
                Entity::PLACEHOLDER,
                GrappleConfig::default(),
                1.0,
            )
            .is_none()
        );
    }
    for config in [
        GrappleConfig {
            min_play_rate: 3.0,
            ..GrappleConfig::default()
        },
        GrappleConfig {
            speed: -13.0,
            ..GrappleConfig::default()
        },
    ] {
        assert!(Grapple::new(Vec3::ZERO, Vec3::X, Entity::PLACEHOLDER, config, 1.0,).is_none());
    }
    let mut state = grapple(Vec3::X);
    assert_eq!(state.desired_position(-1.0), Vec3::ZERO);
    assert_eq!(state.desired_position(f32::NAN), Vec3::ZERO);
}
