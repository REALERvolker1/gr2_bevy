use bevy::prelude::*;
use gr2_controller::traversal::*;

#[test]
fn source_wallrun_start_and_keep_have_different_strict_speed_gates() {
    let config = WallRunConfig::default();
    assert!(can_start_wallrun(Vec3::NEG_Z, Vec3::NEG_Z, Vec3::X, config));
    assert!(!can_keep_wallrun(
        Vec3::NEG_Z,
        Vec3::NEG_Z,
        Vec3::NEG_Z,
        Vec3::NEG_X,
        config
    ));
    assert!(can_keep_wallrun(
        Vec3::NEG_Z * 10.0,
        Vec3::NEG_Z,
        Vec3::NEG_Z,
        Vec3::NEG_X,
        config
    ));
    assert!(!can_start_wallrun(
        Vec3::NEG_Z * 10.0,
        Vec3::NEG_Z,
        Vec3::NEG_Z,
        config
    ));
    assert!(!can_keep_wallrun(
        Vec3::Z * 10.0,
        Vec3::NEG_Z,
        Vec3::NEG_Z,
        Vec3::NEG_X,
        config
    ));
}

#[test]
fn source_normal_rotation_maps_both_wall_sides_to_forward_tangent() {
    assert!(wall_forward(Vec3::X, true).distance(Vec3::NEG_Z) < 0.00001);
    assert!(wall_forward(Vec3::NEG_X, false).distance(Vec3::NEG_Z) < 0.00001);
}

#[test]
fn wall_slowdown_uses_source_capture_clamp_and_two_second_cubic() {
    let config = WallRunConfig::default();
    let mut wall = WallRun {
        entity: Entity::PLACEHOLDER,
        normal: Vec3::X,
        forward: Vec3::NEG_Z,
        elapsed: 0.0,
        captured_speed: 13.5,
    };
    assert_eq!(wall.speed(config), 13.5);
    wall.elapsed = 1.0;
    assert_eq!(wall.speed(config), 11.75);
    wall.elapsed = 2.0;
    assert_eq!(wall.speed(config), 10.0);
}
