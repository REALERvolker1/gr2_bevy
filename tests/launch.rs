use bevy::prelude::*;
use gr2_controller::launch::*;

#[test]
fn launch_overrides_only_requested_channels() {
    let current = Vec3::new(8.0, -3.0, 6.0);
    let jump = resolve_launch(current, Vec3::Y * 11.0, false, true);
    assert_eq!(jump, Vec3::new(8.0, 11.0, 6.0));
    let slide = resolve_launch(current, Vec3::NEG_Z * 18.0, true, false);
    assert_eq!(slide, Vec3::new(0.0, -3.0, -18.0));
}

#[test]
fn air_release_uses_incoming_full_speed_and_camera_vertical_separately() {
    assert_eq!(
        air_dash_release(Vec3::NEG_Z, 0.0, 22.0, 13.0, 1.0, 0.0),
        Vec3::NEG_Z * 22.0
    );
    assert_eq!(
        air_dash_release(Vec3::NEG_Z, 24.0, 22.0, 13.0, 1.0, 0.0),
        Vec3::NEG_Z * 33.0
    );
    assert_eq!(
        air_dash_release(Vec3::Y, 24.0, 22.0, 13.0, 1.0, 0.0),
        Vec3::Y * 13.0
    );
}

#[test]
fn first_and_wall_jump_preserve_native_addition_and_full_direction_normalization() {
    let current = Vec3::new(3.0, 4.0, 0.0);
    assert_eq!(
        first_jump(current, 11.0, 1.0, 1.0),
        Vec3::new(3.0, 11.0, 0.0)
    );
    let boosted = first_jump(current, 11.0, 2.0, 2.0);
    assert!((boosted.x - 9.6).abs() < 0.00001);
    assert_eq!(boosted.y, 22.0);
    assert_eq!(
        wall_jump(Vec3::NEG_Z * 10.0, Vec3::NEG_Z, Vec3::X, 2.5, 8.5, 11.0),
        Vec3::new(8.5, 11.0, -12.5)
    );
}
