use bevy::prelude::*;
use gr2_controller::controller::*;

#[test]
fn dash_hermite_curve_and_distance_playrate_match_blueprint() {
    let mut dash = GroundDash::new(Vec3::ZERO, Vec3::NEG_Z * 5.0, 5.0, true);
    assert_eq!(dash.play_rate, 1.8);
    assert_eq!(dash.alpha(), 0.0);
    dash.elapsed = 0.35 / 1.8;
    assert!((dash.alpha() - 1.0).abs() < 0.000001);
    assert!(dash.finished());
    dash.elapsed = 0.175 / 1.8;
    assert!((dash.alpha() - 0.6345713).abs() < 0.00001);
    let short = GroundDash::new(Vec3::ZERO, Vec3::NEG_Z, 5.0, false);
    assert_eq!(short.play_rate, 7.0);
}

#[test]
fn respawn_clears_action_and_cooldown_generation() {
    let config = ControllerConfig::default();
    let mut controller = Controller::new(config);
    controller.stamina.last_dash_audio = 100.0;
    controller.action = Action::GroundDash(GroundDash::new(Vec3::ZERO, Vec3::X, 5.0, false));
    controller.reset(config);
    assert!(matches!(controller.action, Action::Locomotion));
    assert_eq!(controller.generation, 1);
    assert_eq!(controller.stamina.last_dash_audio, 0.0);
}

#[test]
fn ground_request_spends_once_and_position_curve_keeps_physical_momentum() {
    use avian3d::prelude::*;
    use bevy::time::TimeUpdateStrategy;
    use gr2_controller::motor::*;
    use std::time::Duration;
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        PhysicsPlugins::new(FixedUpdate),
        MotorPlugin,
        ControllerPlugin,
    ))
    .insert_resource(Time::<Fixed>::from_hz(100.0))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        0.01,
    )));
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.5, 40.0),
        Transform::from_xyz(0.0, -0.25, 0.0),
    ));
    let config = ControllerConfig::default();
    let player = app
        .world_mut()
        .spawn((
            Motor::default(),
            config,
            Controller::new(config),
            PlayerCommand::default(),
            Position(Vec3::new(0.0, 0.902, 0.0)),
            Transform::from_xyz(0.0, 0.902, 0.0),
        ))
        .id();
    app.finish();
    app.cleanup();
    for _ in 0..25 {
        app.update();
    }
    {
        let mut command = app.world_mut().get_mut::<PlayerCommand>(player).unwrap();
        command.direction = Vec3::NEG_Z;
        command.dash = true;
        command.block = true;
    }
    app.update();
    assert_eq!(
        app.world().get::<Controller>(player).unwrap().result,
        ActionResult::DashAccepted
    );
    let after_request = app
        .world()
        .get::<Controller>(player)
        .unwrap()
        .stamina
        .current;
    assert_eq!(after_request, 60.0);
    assert_eq!(
        app.world().get::<Motor>(player).unwrap().velocity.z,
        0.0,
        "positional timeline must not generate physical dash momentum"
    );
    for _ in 0..30 {
        if !matches!(
            app.world().get::<Controller>(player).unwrap().action,
            Action::GroundDash(_)
        ) {
            break;
        }
        app.update();
    }
    assert!(!matches!(
        app.world().get::<Controller>(player).unwrap().action,
        Action::GroundDash(_)
    ));
    assert_eq!(
        app.world()
            .get::<Controller>(player)
            .unwrap()
            .stamina
            .current,
        after_request
    );
    assert!((app.world().get::<Position>(player).unwrap().z + 8.0).abs() < 0.001);
    assert!(
        (app.world().get::<Motor>(player).unwrap().velocity.z + 12.0).abs() < 0.001,
        "end queues exactly the recovered physical velocity-change impulse"
    );
    app.update();
    assert!(app.world().get::<Position>(player).unwrap().z < -8.0);
}
