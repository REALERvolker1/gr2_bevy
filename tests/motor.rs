use std::time::Duration;

use avian3d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use gr2_controller::motor::{Motor, MotorPlugin, MotorTranslation};

fn world() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        PhysicsPlugins::new(FixedUpdate),
        MotorPlugin,
    ))
    .insert_resource(Time::<Fixed>::from_hz(60.0))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.finish();
    app.cleanup();
    app.update();
    app
}

fn block(app: &mut App, center: Vec3, size: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(center),
        ))
        .id()
}

fn player(app: &mut App, position: Vec3, velocity: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            Motor {
                velocity,
                ..default()
            },
            Position(position),
            Transform::from_translation(position),
        ))
        .id()
}

fn ticks(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

fn position(app: &App, player: Entity) -> Vec3 {
    app.world().get::<Position>(player).unwrap().0
}

#[test]
fn velocity_is_integrated_once_even_when_physics_velocity_is_present() {
    let mut app = world();
    let p = player(&mut app, Vec3::ZERO, Vec3::X * 6.0);
    app.world_mut()
        .entity_mut(p)
        .insert(LinearVelocity(Vec3::X * 6.0));
    ticks(&mut app, 60);
    assert!(
        (position(&app, p).x - 6.0).abs() < 0.001,
        "{:?}",
        position(&app, p)
    );
}

#[test]
fn action_displacement_is_swept_without_creating_physical_momentum() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(1.0, 20.0, 20.0),
    );
    let p = player(&mut app, Vec3::ZERO, Vec3::new(2.0, -1.0, 3.0));
    app.world_mut().get_mut::<Motor>(p).unwrap().translation =
        Some(MotorTranslation::Sweep(Vec3::X * 10.0));
    ticks(&mut app, 1);
    let motor = app.world().get::<Motor>(p).unwrap();
    assert!(motor.translation.is_none());
    assert!(motor.velocity.x.abs() < 0.001);
    assert!((motor.velocity.y + 1.0).abs() < 0.001);
    assert!((motor.velocity.z - 3.0).abs() < 0.001);
    assert!(position(&app, p).x < 1.151);

    let free = player(&mut app, Vec3::new(-20.0, 0.0, 0.0), Vec3::Z * 3.0);
    app.world_mut().get_mut::<Motor>(free).unwrap().translation =
        Some(MotorTranslation::Sweep(Vec3::X));
    ticks(&mut app, 1);
    assert!((position(&app, free).x + 19.0).abs() < 0.001);
    assert_eq!(
        app.world().get::<Motor>(free).unwrap().velocity,
        Vec3::Z * 3.0
    );
}

#[test]
fn floor_stops_falling_and_reports_ground() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(20.0, 1.0, 20.0),
    );
    let p = player(&mut app, Vec3::new(0.0, 3.0, 0.0), Vec3::NEG_Y * 10.0);
    ticks(&mut app, 30);
    assert!((position(&app, p).y - 0.902).abs() < 0.01);
    assert!(app.world().get::<Motor>(p).unwrap().grounded);
}

#[test]
fn wall_sweep_blocks_high_speed_motion_and_preserves_tangent() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(2.0, 2.0, 0.0),
        Vec3::new(1.0, 10.0, 20.0),
    );
    let p = player(&mut app, Vec3::ZERO, Vec3::new(300.0, 0.0, 6.0));
    ticks(&mut app, 10);
    let pos = position(&app, p);
    assert!(pos.x < 1.151 && pos.x > 1.14, "{pos:?}");
    assert!(pos.z > 0.95, "{pos:?}");
}

#[test]
fn corner_sweeps_resolve_both_planes() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(1.0, 20.0, 20.0),
    );
    block(
        &mut app,
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(20.0, 20.0, 1.0),
    );
    let p = player(&mut app, Vec3::ZERO, Vec3::new(300.0, 5.0, 300.0));
    ticks(&mut app, 3);
    let pos = position(&app, p);
    assert!(pos.x < 1.151 && pos.z < 1.151, "{pos:?}");
    let velocity = app.world().get::<Motor>(p).unwrap().velocity;
    // Two perpendicular wall planes constrain X/Z while their shared vertical
    // tangent remains free. Capsule contact normals have a small numerical tilt
    // (observed Y components around 0.00005), amplified by this 300 m/s sweep.
    assert!(
        velocity.x.abs() < 0.001 && velocity.z.abs() < 0.001,
        "{velocity:?}"
    );
    assert!((velocity.y - 5.0).abs() < 0.05, "{velocity:?}");
}

#[test]
fn ceiling_removes_upward_velocity() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(0.0, 3.0, 0.0),
        Vec3::new(20.0, 1.0, 20.0),
    );
    let p = player(&mut app, Vec3::ZERO, Vec3::Y * 100.0);
    ticks(&mut app, 3);
    assert!(position(&app, p).y < 1.602);
    assert!(app.world().get::<Motor>(p).unwrap().velocity.y.abs() < 0.01);
    assert!(!app.world().get::<Motor>(p).unwrap().grounded);
}

#[test]
fn ground_sensing_distinguishes_walkable_and_steep_slopes() {
    for (angle, walkable) in [(30.0_f32, true), (60.0, false)] {
        let mut app = world();
        let rotation = Quat::from_rotation_z(angle.to_radians());
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(20.0, 1.0, 20.0),
            Transform::from_rotation(rotation),
        ));
        // Capsule's support extent on this plane is radius plus the vertical
        // half-segment projected onto its normal.
        let center_y = 0.55 + 0.85 / angle.to_radians().cos();
        let p = player(&mut app, Vec3::Y * center_y, Vec3::ZERO);
        ticks(&mut app, 2);
        assert_eq!(
            app.world().get::<Motor>(p).unwrap().grounded,
            walkable,
            "angle {angle}"
        );
    }
}

#[test]
fn walking_off_a_ledge_clears_grounded() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(2.0, 1.0, 20.0),
    );
    let p = player(&mut app, Vec3::new(0.0, 0.902, 0.0), Vec3::X * 6.0);
    ticks(&mut app, 1);
    assert!(app.world().get::<Motor>(p).unwrap().grounded);
    ticks(&mut app, 20);
    assert!(!app.world().get::<Motor>(p).unwrap().grounded);
}

#[test]
fn crouch_preserves_feet_and_standing_requires_clearance() {
    let mut app = world();
    block(
        &mut app,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(20.0, 1.0, 20.0),
    );
    let p = player(&mut app, Vec3::new(0.0, 0.902, 0.0), Vec3::ZERO);
    app.world_mut().get_mut::<Motor>(p).unwrap().request_crouch = true;
    ticks(&mut app, 1);
    assert!((position(&app, p).y - 0.502).abs() < 0.001);
    let ceiling = block(&mut app, Vec3::new(0.0, 1.7, 0.0), Vec3::new(4.0, 1.0, 4.0));
    app.world_mut().get_mut::<Motor>(p).unwrap().request_crouch = false;
    ticks(&mut app, 2);
    assert!(app.world().get::<Motor>(p).unwrap().crouched);
    app.world_mut().entity_mut(ceiling).despawn();
    ticks(&mut app, 2);
    assert!(!app.world().get::<Motor>(p).unwrap().crouched);
    assert!((position(&app, p).y - 0.902).abs() < 0.001);
}

#[test]
fn native_unswept_position_crosses_a_wall_without_velocity_or_second_integration() {
    let mut app = world();
    block(&mut app, Vec3::X * 2.0, Vec3::new(1.0, 20.0, 20.0));
    let velocity = Vec3::new(1.0, 2.0, 3.0);
    let p = player(&mut app, Vec3::ZERO, velocity);
    app.world_mut().get_mut::<Motor>(p).unwrap().translation =
        Some(MotorTranslation::SetPosition(Vec3::X * 10.0));
    ticks(&mut app, 1);
    assert_eq!(position(&app, p), Vec3::X * 10.0);
    assert_eq!(app.world().get::<Motor>(p).unwrap().velocity, velocity);
    assert!(app.world().get::<Motor>(p).unwrap().translation.is_none());
    ticks(&mut app, 1);
    assert!((position(&app, p) - (Vec3::X * 10.0 + velocity / 60.0)).length() < 1e-5);
}
