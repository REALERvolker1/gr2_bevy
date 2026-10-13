use avian3d::prelude::*;
use bevy::{
    app::AppExit,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::{
        Render, RenderApp, RenderPlugin, RenderSystems,
        render_resource::{
            CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment,
            RenderPassDescriptor, StoreOp,
        },
        renderer::{RenderDevice, RenderQueue},
        view::ExtractedWindows,
        view::screenshot::{Screenshot, save_to_disk},
    },
    time::TimeUpdateStrategy,
    window::{CursorGrabMode, CursorOptions},
};
use gr2_controller::{
    controller::*, grapple::GrappleAnchor, mantle::MantleSurface, motor::*, traversal::WallRunnable,
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    time::{Duration, Instant},
};

#[derive(Resource)]
struct RunMode {
    smoke: bool,
    frames: u32,
    screenshot_requested: bool,
    started: Instant,
}
#[derive(Resource)]
struct Trace(BufWriter<File>);
#[derive(Resource, Default)]
struct Proof {
    ground_dashes: u32,
    slides: u32,
    jumps: u32,
    air_dashes: u32,
    air_releases: u32,
}
#[derive(Resource, Default)]
struct View {
    yaw: f32,
    pitch: f32,
}
#[derive(Component)]
struct Hud;

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    std::fs::create_dir_all(".work/validation").expect("create private validation directory");
    let mut trace =
        BufWriter::new(File::create(".work/validation/controller.csv").expect("open trace"));
    writeln!(
        trace,
        "step,generation,audio,x,y,z,vx,vy,vz,grounded,crouched,stamina,action,result"
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(RenderPlugin {
                // Bevy's Linux upscaling pass can mark a swapchain used before its async pipeline is ready.
                // Compile synchronously to close later pipeline-readiness gaps; initialize the first image below.
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "GR2 controller research | Bevy + Avian".into(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins((
        PhysicsPlugins::new(FixedUpdate),
        MotorPlugin,
        ControllerPlugin,
    ))
    .insert_resource(Time::<Fixed>::from_hz(120.0))
    .insert_resource(RunMode {
        smoke,
        frames: 0,
        screenshot_requested: false,
        started: Instant::now(),
    })
    .insert_resource(Trace(trace))
    .init_resource::<View>()
    .init_resource::<Proof>()
    .insert_resource(ClearColor(Color::srgb(0.015, 0.025, 0.045)))
    .insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        ..default()
    })
    .add_systems(Startup, setup)
    .add_systems(PreUpdate, read_input)
    .add_systems(FixedUpdate, scripted_input.before(MotorSystems::Sense))
    .add_systems(
        FixedUpdate,
        record
            .after(ControllerSystems::PostMove)
            .before(PhysicsSystems::Writeback),
    )
    .add_systems(Update, (camera_and_hud, smoke_finish));
    // Bevy 0.19.1 can present before its first camera pass is ready. Initialize
    // that first image, matching its existing no-camera clear pass.
    // See docs/research/render-validation.md for the pinned-source failure path.
    #[cfg(target_os = "linux")]
    app.sub_app_mut(RenderApp).add_systems(
        Render,
        clear_initial_surface
            .after(RenderSystems::Prepare)
            .before(RenderSystems::Render),
    );
    if smoke {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    }
    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor = materials.add(Color::srgb(0.08, 0.13, 0.18));
    let wall = materials.add(Color::srgb(0.05, 0.35, 0.40));
    let obstacle = materials.add(Color::srgb(0.8, 0.22, 0.05));
    let blocks = [
        (
            Vec3::new(0.0, -0.25, -10.0),
            Vec3::new(40.0, 0.5, 50.0),
            floor.clone(),
        ),
        (
            Vec3::new(-6.0, 2.0, -10.0),
            Vec3::new(0.5, 4.0, 35.0),
            wall.clone(),
        ),
        (
            Vec3::new(6.0, 2.0, -10.0),
            Vec3::new(0.5, 4.0, 35.0),
            wall.clone(),
        ),
        (Vec3::new(0.0, 4.0, -25.0), Vec3::new(12.0, 8.0, 0.5), wall),
        (
            Vec3::new(3.0, 0.4, -5.0),
            Vec3::new(2.0, 0.8, 2.0),
            obstacle.clone(),
        ),
        (
            Vec3::new(-3.0, 1.35, -7.0),
            Vec3::new(2.0, 0.3, 4.0),
            obstacle,
        ),
    ];
    for (center, size, material) in blocks {
        let block = commands
            .spawn((
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(material),
                Transform::from_translation(center),
            ))
            .id();
        if size.y >= 4.0 {
            commands.entity(block).insert(WallRunnable);
        }
        if size.y < 2.0 && center.y > 0.0 {
            commands.entity(block).insert(MantleSurface);
        }
    }
    // Visible lane markers make displacement and clipping easy to inspect.
    let marker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.7, 0.85),
        emissive: LinearRgba::new(0.0, 1.0, 1.5, 1.0),
        ..default()
    });
    for z in (0..25).step_by(2) {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.08, 0.01, 0.6))),
            MeshMaterial3d(marker.clone()),
            Transform::from_xyz(0.0, 0.01, -(z as f32)),
        ));
    }
    let target = Vec3::new(0.0, 4.0, -10.0);
    commands.spawn((
        GrappleAnchor {
            target,
            ..default()
        },
        Mesh3d(meshes.add(Sphere::new(0.25))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.7, 0.1),
            emissive: LinearRgba::new(2.0, 1.2, 0.0, 1.0),
            ..default()
        })),
        Transform::from_translation(target),
    ));
    let config = ControllerConfig::default();
    commands.spawn((
        Motor::default(),
        config,
        Controller::new(config),
        PlayerCommand::default(),
        Position(Vec3::new(0.0, 0.91, 3.0)),
        Transform::from_xyz(0.0, 0.91, 3.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 90.0_f32.to_radians(),
            ..default()
        }),
        Transform::from_xyz(0.0, 1.6, 3.0),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 7000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.5, 0.0)),
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(19.0),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: px(18),
            top: px(18),
            ..default()
        },
    ));
}

fn read_input(
    mode: Res<RunMode>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mouse: Res<AccumulatedMouseMotion>,
    mut cursor: Single<&mut CursorOptions>,
    mut view: ResMut<View>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut anchors: Query<(Entity, &mut GrappleAnchor)>,
    mut players: Query<(
        &mut PlayerCommand,
        &mut Controller,
        &ControllerConfig,
        &mut Position,
        &mut Motor,
    )>,
) {
    if mode.smoke {
        return;
    }
    if buttons.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    if cursor.grab_mode == CursorGrabMode::Locked {
        view.yaw -= mouse.delta.x * 0.002;
        view.pitch = (view.pitch - mouse.delta.y * 0.002).clamp(-1.5, 1.5);
    }
    let axis =
        |positive, negative| (keys.pressed(positive) as i8 - keys.pressed(negative) as i8) as f32;
    let local = Vec3::new(
        axis(KeyCode::KeyD, KeyCode::KeyA),
        0.0,
        -axis(KeyCode::KeyW, KeyCode::KeyS),
    );
    for (mut command, mut controller, config, mut position, mut motor) in &mut players {
        command.aim = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0) * Vec3::NEG_Z;
        command.dash_held = keys.pressed(KeyCode::ShiftLeft);
        command.forward = Quat::from_rotation_y(view.yaw) * Vec3::NEG_Z;
        command.axes = Some(Vec2::new(local.x, -local.z));
        command.direction = Quat::from_rotation_y(view.yaw) * local.clamp_length_max(1.0);
        command.jump |= keys.just_pressed(KeyCode::Space);
        command.dash |= keys.just_pressed(KeyCode::ShiftLeft);
        command.crouch = keys.pressed(KeyCode::ControlLeft);
        command.block = buttons.pressed(MouseButton::Right);
        if keys.just_pressed(KeyCode::KeyE) {
            let viewport = camera.0.logical_viewport_rect();
            command.grapple_target = anchors
                .iter()
                .filter_map(|(entity, anchor)| {
                    if !anchor.enabled || anchor.blocked || anchor.occupied {
                        return None;
                    }
                    let screen = camera.0.world_to_viewport(camera.1, anchor.target).ok()?;
                    let viewport = viewport?;
                    if !screen.cmpge(viewport.min).all() || !screen.cmplt(viewport.max).all() {
                        return None;
                    }
                    let distance = position.0.distance(anchor.target);
                    if distance >= config.grapple.maximum_range {
                        return None;
                    }
                    // HOST ADAPTATION: screen-centre ranking and camera data come from Bevy.
                    let centre = (viewport.min + viewport.max) * 0.5;
                    let score = screen.distance(centre) / viewport.size().length()
                        + 2.0 * distance / config.grapple.maximum_range;
                    Some((entity, score))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(entity, _)| entity);
        }
        if keys.just_pressed(KeyCode::KeyR) || position.y < -10.0 {
            if let Action::Grapple(grapple) = controller.action {
                if let Ok((_, mut anchor)) = anchors.get_mut(grapple.point) {
                    anchor.occupied = false;
                    anchor.available_after = 0.0;
                }
            }
            controller.reset(*config);
            position.0 = Vec3::new(0.0, 0.91, 3.0);
            motor.velocity = Vec3::ZERO;
        }
    }
}

fn scripted_input(
    mode: Res<RunMode>,
    mut view: ResMut<View>,
    mut players: Query<(&Controller, &mut PlayerCommand)>,
) {
    if !mode.smoke {
        return;
    }
    for (controller, mut input) in &mut players {
        let step = controller.step;
        if step >= 650 {
            view.yaw = std::f32::consts::PI;
            view.pitch = -0.15;
        }
        input.axes = Some(if step < 600 { Vec2::Y } else { Vec2::ZERO });
        input.direction = if step < 600 { Vec3::NEG_Z } else { Vec3::ZERO };
        input.dash = step == 60 || step == 62 || step == 350 || step == 500;
        input.dash_held = (350..380).contains(&step);
        input.jump = step == 300;
        input.crouch = (170..250).contains(&step);
        input.block = (400..450).contains(&step);
    }
}

fn record(
    clocks: Res<ControllerClocks>,
    mode: Res<RunMode>,
    mut trace: ResMut<Trace>,
    mut proof: ResMut<Proof>,
    players: Query<(&Position, &Motor, &Controller)>,
) {
    for (position, motor, controller) in &players {
        match controller.result {
            ActionResult::DashAccepted => proof.ground_dashes += 1,
            ActionResult::SlideAccepted => proof.slides += 1,
            ActionResult::JumpAccepted => proof.jumps += 1,
            ActionResult::AirDashAccepted => proof.air_dashes += 1,
            ActionResult::AirDashReleased => proof.air_releases += 1,
            _ => {}
        }
        if !mode.smoke && controller.result == ActionResult::None && controller.step % 12 != 0 {
            continue;
        }
        let action = match controller.action {
            Action::Locomotion => "locomotion",
            Action::Slide => "slide",
            Action::GroundDash(_) => "ground_dash",
            Action::AirDash(_) => "air_dash",
            Action::WallRun(_) => "wall_run",
            Action::Grapple(_) => "grapple",
            Action::Mounting(_) => "mounting",
        };
        // Compact idle trace; every active diagnostic step is retained for replay.
        writeln!(
            trace.0,
            "{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{:.6},{},{:?}",
            controller.step,
            controller.generation,
            clocks.audio_elapsed,
            position.x,
            position.y,
            position.z,
            motor.velocity.x,
            motor.velocity.y,
            motor.velocity.z,
            motor.grounded,
            motor.crouched,
            controller.stamina.current,
            action,
            controller.result
        )
        .expect("write controller trace");
    }
}

fn camera_and_hud(
    view: Res<View>,
    player: Single<(&Position, &Motor, &Controller)>,
    mut camera: Single<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let (position, motor, controller) = *player;
    camera.0.translation = position.0 + Vec3::Y * (motor.height() * 0.5 - 0.15);
    camera.0.rotation = Quat::from_euler(EulerRot::YXZ, view.yaw, view.pitch, 0.0);
    if let Projection::Perspective(projection) = &mut *camera.1 {
        projection.fov = (90.0 + controller.fov_add).to_radians();
    }
    hud.0 = format!(
        "GR2 controller research\nWASD  move   Space  jump   Shift  ground dash   Ctrl  slide/crouch\nE  grapple   RMB  block   R  reset   click  capture mouse   Esc  release\n\nSpeed  {:.2} m/s   Stamina  {:.2}   Ground  {}\n{:?}\n\nStatic reconstruction + documented host policies. Source parity pending.",
        motor.velocity.length(),
        controller.stamina.current,
        motor.grounded,
        controller.action
    );
}

fn smoke_finish(
    mut mode: ResMut<RunMode>,
    mut commands: Commands,
    mut trace: ResMut<Trace>,
    player: Single<(&Position, &Controller)>,
    proof: Res<Proof>,
    mut exit: MessageWriter<AppExit>,
) {
    if !mode.smoke {
        return;
    }
    mode.frames += 1;
    if mode.frames >= 300
        && !mode.screenshot_requested
        && mode.started.elapsed() >= Duration::from_secs(15)
    {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(".work/validation/xvfb.png"));
        mode.screenshot_requested = true;
    }
    if mode.frames >= 360
        && mode.screenshot_requested
        && mode.started.elapsed() >= Duration::from_secs(20)
    {
        trace.0.flush().expect("flush trace");
        let (position, controller) = *player;
        assert!(mode.screenshot_requested && position.0.is_finite() && controller.step >= 600);
        assert!(controller.stamina.current >= -150.0 && controller.stamina.current <= 100.0);
        assert!(
            proof.ground_dashes > 0
                && proof.slides > 0
                && proof.jumps > 0
                && proof.air_dashes > 0
                && proof.air_releases > 0,
            "script must execute ground dash, slide, jump, air press and release"
        );
        assert!(position.z >= -24.5, "wall sweep must stop the capsule");
        info!(step = controller.step, position = ?position.0, "Xvfb controller smoke completed");
        exit.write(AppExit::Success);
    }
}

#[cfg(target_os = "linux")]
fn clear_initial_surface(
    windows: Res<ExtractedWindows>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    for window in windows.values() {
        if !window.needs_initial_present {
            continue;
        }
        let Some(view) = window.swap_chain_texture_view.as_ref() else {
            continue;
        };
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("initial_surface_clear"),
        });
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("initial_surface_clear"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(LinearRgba::BLACK.into()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        queue.submit([encoder.finish()]);
    }
}
