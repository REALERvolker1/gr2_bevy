//! Actual Avian query/motor integration of the qualified native mounting specification.
use avian3d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use gr2_controller::{controller::*, ledge::*, motor::*};
use std::time::Duration;

fn fixture(block_tag: bool, overhead: bool, gravity: f32) -> (App, Entity) {
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
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 0.5, 20.0),
        Transform::from_xyz(0.0, -0.25, 0.0),
    ));
    // Ordinary geometry needs no special mount marker in the source query.
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(2.0, 1.0, 2.0),
        Transform::from_xyz(0.0, 0.5, -1.0),
        ClimbSurface {
            component_block_climb: block_tag,
            ..default()
        },
    ));
    if overhead {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(10.0, 0.1, 10.0),
            Transform::from_xyz(0.0, 3.2, 0.4),
        ));
    }
    let cfg = ControllerConfig {
        gravity,
        ..default()
    };
    let motor_config = MotorConfig::gr2();
    let player = app
        .world_mut()
        .spawn((
            Motor {
                config: motor_config,
                ..default()
            },
            motor_config.standing_collider(),
            cfg,
            Controller::new(cfg),
            PlayerCommand::default(),
            Position(Vec3::new(0.0, 0.962, 0.6)),
            Transform::from_xyz(0.0, 0.962, 0.6),
        ))
        .id();
    app.finish();
    app.cleanup();
    for _ in 0..5 {
        app.update();
    }
    (app, player)
}
fn jump_forward(app: &mut App, player: Entity) {
    let mut command = app.world_mut().get_mut::<PlayerCommand>(player).unwrap();
    command.direction = Vec3::NEG_Z;
    command.axes = Some(Vec2::Y);
    command.jump = true;
    drop(command);
    app.update();
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::JumpAccepted
    );
}

#[test]
fn automatic_ascent_mount_resets_jump_and_zeroes_velocity_without_held_jump() {
    let (mut app, player) = fixture(false, false, 29.4);
    jump_forward(&mut app, player);
    assert!(app.world().get::<Motor>(player).unwrap().velocity.y > 0.0);
    app.update();
    let controller = app.world().get::<Controller>(player).unwrap();
    assert_eq!(controller.result, ActionResult::MantleAccepted);
    assert_eq!(controller.jump_count, 0);
    assert!(!controller.can_use_climbing);
    assert_eq!(
        app.world().get::<Motor>(player).unwrap().velocity,
        Vec3::ZERO
    );
    // Releasing forward does not stop the native component's committed trajectory.
    {
        let mut command = app.world_mut().get_mut::<PlayerCommand>(player).unwrap();
        command.direction = Vec3::ZERO;
        command.axes = Some(Vec2::ZERO);
    }
    for _ in 0..60 {
        app.update();
        if app.world().get::<Controller>(player).unwrap().result == ActionResult::MantleFinished {
            let velocity = app.world().get::<Motor>(player).unwrap().velocity;
            assert!((velocity.z + 10.0).abs() < 1e-5);
            assert!(app.world().get::<Position>(player).unwrap().y > 2.4);
            return;
        }
    }
    panic!("native mantle failed to finish");
}

#[test]
fn block_climb_tag_returns_a_hit_but_does_not_start_or_zero_movement() {
    let (mut app, player) = fixture(true, false, 29.4);
    jump_forward(&mut app, player);
    app.world_mut()
        .get_mut::<Controller>(player)
        .unwrap()
        .air_ready = false;
    app.update();
    let controller = app.world().get::<Controller>(player).unwrap();
    assert_eq!(controller.result, ActionResult::MantleDetected);
    assert!(matches!(controller.action, Action::Locomotion));
    assert_eq!(controller.jump_count, 0);
    assert!(!controller.can_use_climbing);
    assert!(!controller.air_ready, "ResetJump is not ResetDash");
    assert!(app.world().get::<Motor>(player).unwrap().velocity.y > 5.0);
}

#[test]
fn original_long_overhead_sweep_rejects_even_when_final_ledge_pose_would_fit() {
    let (mut app, player) = fixture(false, true, 29.4);
    jump_forward(&mut app, player);
    for _ in 0..15 {
        app.update();
        assert!(!matches!(
            app.world().get::<Controller>(player).unwrap().action,
            Action::Mounting(_)
        ));
    }
    assert!(
        app.world()
            .get::<Controller>(player)
            .unwrap()
            .can_use_climbing
    );
}

#[test]
fn shared_presentation_permission_delay_uses_world_clock_and_survives_motion_finish() {
    let (mut app, player) = fixture(false, false, 0.0);
    {
        let mut clocks = app.world_mut().resource_mut::<ControllerClocks>();
        clocks.custom_time_dilation = 0.5;
        clocks.global_time_dilation = 0.5;
    }
    app.world_mut()
        .get_mut::<PlayerCommand>(player)
        .unwrap()
        .force_mount = true;
    app.update();
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::MantleAccepted
    );
    assert_eq!(
        app.world()
            .get::<Controller>(player)
            .unwrap()
            .climb_presentation,
        ClimbPresentation::High
    );
    let mut finished = false;
    for _ in 0..380 {
        app.update();
        finished |=
            app.world().get::<Controller>(player).unwrap().result == ActionResult::MantleFinished;
    }
    assert!(finished);
    assert!(
        !app.world()
            .get::<Controller>(player)
            .unwrap()
            .can_use_climbing,
        "finish cannot invent a permission reset"
    );
    for _ in 0..25 {
        app.update();
    }
    assert!(
        app.world()
            .get::<Controller>(player)
            .unwrap()
            .can_use_climbing
    );
}

#[test]
fn second_mount_after_landing_preserves_pending_blueprint_delay() {
    let (mut app, player) = fixture(false, false, 0.0);
    app.world_mut()
        .resource_mut::<ControllerClocks>()
        .custom_time_dilation = 0.5;
    app.world_mut()
        .get_mut::<PlayerCommand>(player)
        .unwrap()
        .force_mount = true;
    app.update();
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::MantleAccepted
    );
    for _ in 0..100 {
        app.update();
    }
    // A landing permits another mount before the old latent callback completes.
    // Use the motor's pose request so sensing observes a real floor transition.
    {
        let mut motor = app.world_mut().get_mut::<Motor>(player).unwrap();
        motor.velocity = Vec3::ZERO;
        motor.translation = Some(MotorTranslation::SetPosition(Vec3::new(0.0, 0.962, 0.6)));
    }
    app.update();
    app.update();
    assert!(
        app.world()
            .get::<Controller>(player)
            .unwrap()
            .can_use_climbing
    );
    app.world_mut()
        .get_mut::<PlayerCommand>(player)
        .unwrap()
        .force_mount = true;
    app.update();
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::MantleAccepted
    );
    for _ in 0..110 {
        app.update();
    }
    assert!(
        app.world()
            .get::<Controller>(player)
            .unwrap()
            .can_use_climbing,
        "ordinary Delay keeps its first deadline; RetriggerableDelay would postpone it"
    );
}
