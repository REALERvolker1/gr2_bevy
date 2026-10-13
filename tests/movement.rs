use bevy::prelude::*;
use gr2_controller::movement::PendingMovement;

#[test]
fn pending_launch_overwrites_earlier_impulse_and_force_and_all_requests_clear() {
    let mut requests = PendingMovement::default();
    requests.add_impulse(Vec3::NEG_Z * 12.0);
    requests.add_force(Vec3::X * 30.0);
    requests.queue_launch(Vec3::Y * 11.0);
    let mut velocity = Vec3::NEG_Z * 10.0;
    assert!(requests.apply(&mut velocity, 0.1));
    assert_eq!(velocity, Vec3::Y * 11.0);
    assert!(!requests.apply(&mut velocity, 0.1));
    assert_eq!(velocity, Vec3::Y * 11.0);
}

#[test]
fn zero_launch_sentinel_keeps_accumulated_impulse_and_delta_scaled_force() {
    let mut requests = PendingMovement::default();
    requests.add_impulse(Vec3::NEG_Z * 12.0);
    requests.add_force(Vec3::X * 30.0);
    requests.queue_launch(Vec3::ZERO);
    let mut velocity = Vec3::NEG_Z * 10.0;
    assert!(!requests.apply(&mut velocity, 0.1));
    assert_eq!(velocity, Vec3::new(3.0, 0.0, -22.0));
}
