//! Integration checks for the committed-pose release boundary, not GR2 parity.
use avian3d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use gr2_controller::{controller::*, grapple::*, motor::*};
use std::time::Duration;

fn fixture(blocked: bool) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        PhysicsPlugins::new(FixedUpdate),
        MotorPlugin,
        ControllerPlugin,
    ))
    .insert_resource(Time::<Fixed>::from_hz(100.0))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        10,
    )));
    let target = Vec3::X * 10.0;
    let point = app
        .world_mut()
        .spawn(GrappleAnchor {
            target,
            occupied: true,
            ..default()
        })
        .id();
    if blocked {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(1.0, 20.0, 20.0),
            Transform::from_xyz(5.0, 0.0, 0.0),
        ));
    }
    let config = ControllerConfig {
        gravity: 0.0,
        air_acceleration: 0.0,
        ..default()
    };
    let mut controller = Controller::new(config);
    controller.action =
        Action::Grapple(Grapple::new(Vec3::ZERO, target, point, config.grapple, 1.0).unwrap());
    let player = app
        .world_mut()
        .spawn((
            Motor {
                velocity: Vec3::new(1.0, 5.0, -2.0),
                ..default()
            },
            config,
            controller,
            PlayerCommand::default(),
            Position(Vec3::ZERO),
            Transform::default(),
        ))
        .id();
    app.finish();
    app.cleanup();
    (app, player, point)
}

fn finish(app: &mut App, player: Entity) {
    for _ in 0..100 {
        app.update();
        if !matches!(
            app.world().get::<Controller>(player).unwrap().action,
            Action::Grapple(_)
        ) {
            return;
        }
    }
    panic!("grapple did not terminate within its authored curve duration");
}

#[test]
fn free_grapple_queues_full_override_after_committed_pose_then_releases_point() {
    let (mut app, player, point) = fixture(false);
    finish(&mut app, player);
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::GrappleReleased
    );
    assert!(app.world().get::<Position>(player).unwrap().x >= 8.0);
    let anchor = app.world().get::<GrappleAnchor>(point).unwrap();
    assert!(!anchor.occupied);
    assert!(anchor.available_after > app.world().resource::<ControllerClocks>().audio_elapsed);
    app.update();
    assert!((app.world().get::<Motor>(player).unwrap().velocity - Vec3::X * 20.0).length() < 0.001);
}

#[test]
fn blocked_grapple_cannot_release_from_its_unachieved_curve_target() {
    let (mut app, player, point) = fixture(true);
    finish(&mut app, player);
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::GrappleCancelled
    );
    assert!(app.world().get::<Position>(player).unwrap().x < 4.151);
    assert!(!app.world().get::<GrappleAnchor>(point).unwrap().occupied);
    app.update();
    assert!(app.world().get::<Motor>(player).unwrap().velocity.x < 0.001);
}
