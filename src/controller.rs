//! Source action arithmetic with explicit host integration policy.
//! See docs/source-boundaries.md before treating any host test as GR2 parity.
use crate::{
    grapple::{Grapple, GrappleAnchor, GrappleConfig},
    launch::{air_dash_release, first_jump, wall_jump},
    locomotion::{WalkingConfig, walking_velocity},
    mantle::{MantleConfig, MantleStep, MantleSurface},
    motor::{Motor, MotorSystems},
    movement::PendingMovement,
    resources::*,
    slide,
    speed::{SpeedConfig, SpeedInput, SpeedState, classify_input},
    traversal::{
        Mounting, WallRun, WallRunConfig, WallRunnable, can_keep_wallrun, query_mantle, query_wall,
    },
};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Source centimetres to host metres. UE Z-up maps to Bevy Y-up.
pub const CM_TO_METRES: f32 = 0.01;

#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerCommand {
    /// Source axes X right, Y forward. None derives axes from the world command.
    pub axes: Option<Vec2>,
    pub direction: Vec3,
    pub forward: Vec3,
    pub aim: Vec3,
    pub dash_held: bool,
    pub jump: bool,
    pub dash: bool,
    pub crouch: bool,
    pub block: bool,
    pub grapple_target: Option<Entity>,
}

impl Default for PlayerCommand {
    fn default() -> Self {
        Self {
            axes: None,
            direction: Vec3::ZERO,
            forward: Vec3::NEG_Z,
            aim: Vec3::NEG_Z,
            dash_held: false,
            jump: false,
            dash: false,
            crouch: false,
            block: false,
            grapple_target: None,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct ControllerConfig {
    pub resources: ResourceConfig,
    pub speed: SpeedConfig,
    pub walking: WalkingConfig,
    pub crouch_speed: f32,
    pub air_acceleration: f32,
    pub gravity: f32,
    pub jump_speed: f32,
    pub dash_forward_range: f32,
    pub dash_side_range: f32,
    pub dash_forward_angle: f32,
    pub dash_end_impulse: f32,
    pub air_forward_force: f32,
    pub air_upward_force: f32,
    pub air_gravity_multiplier: f32,
    pub sensory_cost_per_second: f32,
    pub air_duration: f32,
    pub coyote_time: f32,
    pub wallrun: WallRunConfig,
    pub grapple: GrappleConfig,
    pub mantle: MantleConfig,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            resources: ResourceConfig::default(),
            speed: SpeedConfig::default(),
            walking: WalkingConfig::default(),
            crouch_speed: 5.0,
            air_acceleration: 8.0,
            // Native gravity fallback 980 cm/s² and jump scale 3. Volume overrides remain caller configuration.
            gravity: 29.4,
            jump_speed: 11.0,
            dash_forward_range: 8.0,
            dash_side_range: 5.5,
            dash_forward_angle: 15.0_f32.to_radians(),
            dash_end_impulse: 12.0,
            air_forward_force: 22.0,
            air_upward_force: 13.0,
            air_gravity_multiplier: 1.0,
            sensory_cost_per_second: 50.0,
            air_duration: 5.0,
            coyote_time: 0.3,
            wallrun: WallRunConfig::default(),
            grapple: GrappleConfig::default(),
            mantle: MantleConfig::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GroundDash {
    pub start: Vec3,
    pub target: Vec3,
    pub elapsed: f32,
    pub play_rate: f32,
}

impl GroundDash {
    pub const CURVE_LENGTH: f32 = 0.35;

    /// BP CalculateDashPlayRate, export211. Distance is source centimetres.
    pub fn new(start: Vec3, target: Vec3, range: f32, forward: bool) -> Self {
        let multiplier = if forward { 1.8 } else { 1.4 };
        let play_rate = range / start.distance(target).max(CM_TO_METRES) * multiplier;
        Self {
            start,
            target,
            elapsed: 0.0,
            play_rate,
        }
    }

    /// Timeline_4's two unweighted cubic keys, export287; no guessed impulse.
    pub fn alpha(&self) -> f32 {
        crate::curves::GROUND_DASH.evaluate(self.elapsed * self.play_rate)
    }

    pub fn finished(&self) -> bool {
        self.elapsed * self.play_rate >= Self::CURVE_LENGTH
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AirDash {
    pub incoming_speed: f32,
    pub elapsed: f32,
    pub sensory_used: f32,
    pub timer_elapsed: f32,
}

#[derive(Clone, Debug, Default)]
pub enum Action {
    #[default]
    Locomotion,
    Slide,
    GroundDash(GroundDash),
    AirDash(AirDash),
    WallRun(WallRun),
    Grapple(Grapple),
    Mounting(Mounting),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ActionResult {
    #[default]
    None,
    DashAccepted,
    DashCooldown,
    DashDepleted,
    AirDashAccepted,
    AirDashReleased,
    AirDashNotReady,
    JumpAccepted,
    JumpDenied,
    SlideAccepted,
    WallJumpAccepted,
    GrappleAccepted,
    GrappleDenied,
    GrappleReleased,
    GrappleCancelled,
    MantleAccepted,
    MantleFinished,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SpeedGate {
    #[default]
    Idle,
    Forward,
    Backward,
    Side,
}

#[derive(Component, Debug)]
pub struct Controller {
    pub speed: SpeedState,
    pub maximum_speed: f32,
    speed_gate: SpeedGate,
    pub stamina: Stamina,
    pub modifiers: DashModifiers,
    pub action: Action,
    pub result: ActionResult,
    pub fov_add: f32,
    pub air_ready: bool,
    pub not_on_ground_time: f32,
    pub jump_count: u32,
    pub wall_cooldown: f32,
    pub pending: PendingMovement,
    pub previous_grapple: Option<Entity>,
    pub generation: u64,
    pub step: u64,
}

impl Controller {
    pub fn new(config: ControllerConfig) -> Self {
        Self {
            speed: SpeedState::default(),
            maximum_speed: config.speed.forward_max,
            speed_gate: SpeedGate::Idle,
            stamina: Stamina::new(config.resources),
            modifiers: DashModifiers::default(),
            action: Action::Locomotion,
            result: ActionResult::None,
            fov_add: 0.0,
            air_ready: true,
            not_on_ground_time: 0.0,
            jump_count: 0,
            wall_cooldown: 0.0,
            pending: PendingMovement::default(),
            previous_grapple: None,
            generation: 0,
            step: 0,
        }
    }
    /// Host lifecycle resets deadlines when respawning into a new clock generation.
    pub fn reset(&mut self, config: ControllerConfig) {
        let generation = self.generation + 1;
        *self = Self::new(config);
        self.generation = generation;
    }
}

/// Audio elapsed is supplied separately from simulation delta for replay.
#[derive(Resource, Debug, Default)]
pub struct ControllerClocks {
    pub audio_elapsed: f32,
    pub world_factor: f32,
    /// Regeneration-specific factor, distinct from actor CustomTimeDilation.
    pub player_factor: f32,
    pub custom_time_dilation: f32,
    pub global_time_dilation: f32,
    pub paused: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerSystems {
    Actions,
    PostMove,
}

pub struct ControllerPlugin;
impl Plugin for ControllerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ControllerClocks {
            world_factor: 1.0,
            player_factor: 1.0,
            custom_time_dilation: 1.0,
            global_time_dilation: 1.0,
            ..default()
        })
        .add_systems(FixedUpdate, advance_audio.before(MotorSystems::Sense))
        .add_systems(
            FixedUpdate,
            update_controller
                .in_set(ControllerSystems::Actions)
                .after(MotorSystems::Sense)
                .before(MotorSystems::Move),
        )
        .add_systems(
            FixedUpdate,
            finish_grapple
                .in_set(ControllerSystems::PostMove)
                .after(MotorSystems::Move)
                .before(PhysicsSystems::Writeback),
        );
    }
}

fn advance_audio(time: Res<Time<Fixed>>, mut clocks: ResMut<ControllerClocks>) {
    // HOST ADAPTATION: fixed unpaused seconds stand in for UE AudioTimeSeconds.
    if !clocks.paused {
        clocks.audio_elapsed += time.delta_secs();
    }
}

fn update_controller(
    time: Res<Time<Fixed>>,
    clocks: Res<ControllerClocks>,
    spatial: SpatialQuery,
    wall_surfaces: Query<(), With<WallRunnable>>,
    mantle_surfaces: Query<&Position, With<MantleSurface>>,
    mut grapple_anchors: Query<&mut GrappleAnchor>,
    mut players: Query<(
        Entity,
        &Position,
        &Collider,
        &ControllerConfig,
        &mut PlayerCommand,
        &mut Controller,
        &mut Motor,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, position, collider, config, mut command, mut controller, mut motor) in &mut players
    {
        controller.step += 1;
        controller.result = ActionResult::None;
        if clocks.paused {
            motor.displacement = Some(Vec3::ZERO);
            continue;
        }
        // HOST ADAPTATION: fixed post-query reset/update of the source NotOnGroundTimer.
        if motor.grounded {
            controller.not_on_ground_time = 0.0;
            controller.jump_count = 0;
        } else {
            controller.not_on_ground_time += dt;
        }
        motor.request_crouch = command.crouch;
        controller.stamina.regeneration_enabled = !command.block;
        // HOST ADAPTATION: choose post-action regeneration, retain both schedules in pure fixtures.
        let direction =
            Vec3::new(command.direction.x, 0.0, command.direction.z).clamp_length_max(1.0);
        if let Some(point) = command.grapple_target.take() {
            let mut admitted = false;
            if matches!(controller.action, Action::Locomotion | Action::WallRun(_)) {
                if let Ok(mut anchor) = grapple_anchors.get_mut(point) {
                    if anchor.enabled
                        && !anchor.blocked
                        && !anchor.occupied
                        && anchor.available_after <= clocks.audio_elapsed
                        && position.0.distance(anchor.target) < config.grapple.maximum_range
                    {
                        let delta = anchor.target - position.0;
                        let clear = Dir3::new(delta).ok().is_some_and(|direction| {
                            spatial
                                .cast_ray(
                                    position.0,
                                    direction,
                                    delta.length(),
                                    true,
                                    &SpatialQueryFilter::from_excluded_entities([entity, point]),
                                )
                                .is_none()
                        });
                        if clear {
                            if let Some(grapple) = Grapple::new(
                                position.0,
                                anchor.target,
                                point,
                                config.grapple,
                                clocks.custom_time_dilation,
                            ) {
                                anchor.occupied = true;
                                anchor.available_after = clocks.audio_elapsed + 0.5;
                                controller.previous_grapple = Some(point);
                                controller.action = Action::Grapple(grapple);
                                admitted = true;
                            }
                        }
                    }
                }
            }
            controller.result = if admitted {
                ActionResult::GrappleAccepted
            } else {
                ActionResult::GrappleDenied
            };
        }
        // HOST ADAPTATION: tagged ledge sensing maps ordinary jump/falling activation to source TryClimb.
        if matches!(controller.action, Action::Locomotion)
            && direction.dot(command.forward) > 0.0
            && !command.crouch
            && (command.jump || (!motor.grounded && motor.velocity.y <= 0.0))
        {
            if let Some(mounting) = query_mantle(
                &spatial,
                &mantle_surfaces,
                entity,
                position.0,
                command.forward,
                collider,
                motor.config.radius,
                motor.height(),
                config.mantle,
            ) {
                controller.action = Action::Mounting(mounting);
                controller.result = ActionResult::MantleAccepted;
                command.jump = false;
            }
        }
        controller.wall_cooldown = (controller.wall_cooldown - dt).max(0.0);
        if matches!(controller.action, Action::Locomotion | Action::WallRun(_)) {
            let contact = if !motor.grounded && !command.crouch && controller.wall_cooldown == 0.0 {
                query_wall(
                    &spatial,
                    &wall_surfaces,
                    entity,
                    position.0,
                    command.forward,
                    motor.velocity,
                    motor.config.radius,
                    config.wallrun,
                )
            } else {
                None
            };
            match (controller.action.clone(), contact) {
                (Action::WallRun(previous), Some(contact))
                    if previous.entity == contact.entity
                        && can_keep_wallrun(
                            motor.velocity,
                            command.aim,
                            command.forward,
                            contact.normal,
                            config.wallrun,
                        ) => {}
                (Action::WallRun(_), _) => {
                    controller.action = Action::Locomotion;
                    controller.wall_cooldown = 0.3;
                }
                (Action::Locomotion, Some(contact)) => {
                    controller.action = Action::WallRun(contact);
                }
                _ => {}
            }
        }
        // HOST ADAPTATION: landing reset caller is not closed, but ResetDash restores readiness.
        if motor.grounded && !matches!(controller.action, Action::AirDash(_)) {
            controller.air_ready = true;
        }
        if command.dash
            && matches!(
                controller.action,
                Action::Locomotion | Action::Slide | Action::WallRun(_)
            )
        {
            if !motor.grounded && !matches!(controller.action, Action::WallRun(_)) {
                if controller.air_ready && controller.stamina.current > 0.0 {
                    controller.action = Action::AirDash(AirDash {
                        incoming_speed: motor.velocity.length(),
                        elapsed: 0.0,
                        sensory_used: 0.0,
                        timer_elapsed: 0.0,
                    });
                    controller.result = ActionResult::AirDashAccepted;
                } else {
                    controller.result = ActionResult::AirDashNotReady;
                }
            } else {
                let admission = controller
                    .stamina
                    .admit_dash(clocks.audio_elapsed, config.resources);
                controller.result = match admission {
                    DashAdmission::Accepted => ActionResult::DashAccepted,
                    DashAdmission::Cooldown => ActionResult::DashCooldown,
                    DashAdmission::Depleted => ActionResult::DashDepleted,
                };
                if admission == DashAdmission::Accepted {
                    let debit = controller.modifiers.cost(0.0, config.resources.dash_cost);
                    // No energy conversion is active in this empty-loadout fixture.
                    controller.stamina.decrease(debit, config.resources.maximum);
                    let heading = direction.try_normalize().unwrap_or(command.forward);
                    let forward = direction.length_squared() > 0.0
                        && direction.normalize().dot(command.forward)
                            > config.dash_forward_angle.cos();
                    let range = if forward {
                        config.dash_forward_range
                    } else {
                        config.dash_side_range
                    };
                    // HOST ADAPTATION: native DashEndPoint/step-up closure is pending.
                    let distance = spatial
                        .cast_shape(
                            collider,
                            position.0,
                            Quat::IDENTITY,
                            Dir3::new(heading).unwrap(),
                            &ShapeCastConfig::from_max_distance(range),
                            &SpatialQueryFilter::from_excluded_entities([entity]),
                        )
                        .map_or(range, |hit| (hit.distance - motor.config.skin).max(0.0));
                    let target = position.0 + heading * distance;
                    controller.action =
                        Action::GroundDash(GroundDash::new(position.0, target, range, forward));
                }
            }
        }
        if command.jump {
            if let Action::WallRun(wall) = controller.action {
                controller.pending.queue_launch(wall_jump(
                    motor.velocity,
                    wall.forward,
                    wall.normal,
                    2.5,
                    8.5,
                    11.0,
                ));
                controller.action = Action::Locomotion;
                controller.wall_cooldown = 0.3;
                controller.result = ActionResult::WallJumpAccepted;
                command.jump = false;
            }
        }
        if command.jump {
            // BP coyote discriminator + native first jump; collision/state guards remain bounded host policy.
            if controller.jump_count == 0
                && (motor.grounded || controller.not_on_ground_time < config.coyote_time)
                && !matches!(
                    controller.action,
                    Action::AirDash(_) | Action::Grapple(_) | Action::Mounting(_)
                )
            {
                controller.pending.queue_launch(first_jump(
                    motor.velocity,
                    config.jump_speed,
                    1.0,
                    1.0,
                ));
                motor.grounded = false;
                controller.air_ready = true;
                controller.jump_count += 1;
                controller.action = Action::Locomotion;
                controller.result = ActionResult::JumpAccepted;
            } else {
                controller.result = ActionResult::JumpDenied;
            }
        }
        if command.crouch
            && motor.grounded
            && matches!(controller.action, Action::Locomotion)
            && slide::has_slide_input(direction.z, direction.x)
        {
            if let Some(band) = slide::select(&slide::FLAT, motor.velocity.length() / CM_TO_METRES)
            {
                let planar = Vec3::new(motor.velocity.x, 0.0, motor.velocity.z).normalize_or_zero();
                // LaunchCharacter slot +0x7d8: horizontal override true, vertical override false.
                controller
                    .pending
                    .queue_launch(crate::launch::resolve_launch(
                        motor.velocity,
                        planar * band.launch * CM_TO_METRES,
                        true,
                        false,
                    ));
                controller.fov_add = band.fov;
                controller.action = Action::Slide;
                controller.result = ActionResult::SlideAccepted;
            }
        }
        if !command.crouch && matches!(controller.action, Action::Slide) {
            controller.action = Action::Locomotion;
            controller.fov_add = 0.0;
        }
        let mut sensory_debit = 0.0;
        let mut dash_end_impulse = Vec3::ZERO;
        match &mut controller.action {
            Action::GroundDash(dash) => {
                dash.elapsed += dt;
                let intended = dash.start.lerp(dash.target, dash.alpha());
                // Source teleports unswept; host sweeps each displacement to prevent wall clipping.
                motor.displacement = Some(intended - position.0);
                if dash.finished() {
                    // Native +0x818 queues a velocity-change impulse, not an overwrite.
                    // Ordinary native PerformMovement consumes impulses before a pending launch.
                    dash_end_impulse =
                        (dash.target - dash.start).normalize_or_zero() * config.dash_end_impulse;
                    controller.action = Action::Locomotion;
                }
            }
            Action::AirDash(air) => {
                air.elapsed += dt;
                air.timer_elapsed += dt;
                while air.timer_elapsed >= 0.01 {
                    air.timer_elapsed -= 0.01;
                    // Source uses its repeating timer and global dilation. Host fixes dilation at one.
                    let debit = config.sensory_cost_per_second * 0.01 / clocks.global_time_dilation;
                    air.sensory_used += debit;
                    sensory_debit += debit;
                }
                // HOST ADAPTATION: no sensory world slowdown until factor/clock closure.
            }
            Action::WallRun(wall) => {
                wall.elapsed += dt;
                // Native tangent velocity rewrite + slowdown curve. Query/constraint phase is host policy.
                motor.velocity = wall.forward.normalize_or_zero() * wall.speed(config.wallrun);
                // Source plane constraints project displacement rather than stored velocity.
                // HOST ADAPTATION: opaque constraint-enable gates are represented by this active wall state.
                if Vec2::new(motor.velocity.x, motor.velocity.z).length() > 1.1 {
                    motor.displacement =
                        Some(Vec3::new(motor.velocity.x, 0.0, motor.velocity.z) * dt);
                }
            }
            Action::Locomotion => {
                update_speed_target(
                    &mut controller,
                    config,
                    command.axes.unwrap_or_else(|| {
                        Vec2::new(
                            direction.dot(command.forward.cross(Vec3::Y)),
                            direction.dot(command.forward),
                        )
                    }),
                    motor.velocity.length(),
                    dt,
                    &clocks,
                );
                let speed = if motor.crouched {
                    config.crouch_speed
                } else {
                    controller.maximum_speed
                };
                if motor.grounded {
                    motor.velocity =
                        walking_velocity(motor.velocity, direction, speed, dt, config.walking);
                } else {
                    // HOST ADAPTATION: falling CalcVelocity/AirControl boosts remain a source dependency.
                    let horizontal = Vec3::new(motor.velocity.x, 0.0, motor.velocity.z);
                    motor.velocity += (direction * speed - horizontal)
                        .clamp_length_max(config.air_acceleration * dt);
                }
            }
            Action::Slide => {}
            Action::Grapple(grapple) => {
                motor.displacement = Some(grapple.desired_position(dt) - position.0);
            }
            Action::Mounting(mounting) => {
                let origin = mantle_surfaces
                    .get(mounting.surface)
                    .map_or(Vec3::ZERO, |pose| pose.0);
                match mounting.motion.tick(
                    position.0,
                    origin,
                    command.forward,
                    dt,
                    clocks.custom_time_dilation,
                ) {
                    MantleStep::Position(next) => {
                        motor.displacement = Some(next - position.0);
                    }
                    MantleStep::Finished { velocity } => {
                        motor.velocity = velocity;
                        controller.action = Action::Locomotion;
                        controller.result = ActionResult::MantleFinished;
                    }
                    MantleStep::Paused => {
                        motor.displacement = Some(Vec3::ZERO);
                    }
                    MantleStep::Inactive => {
                        controller.action = Action::Locomotion;
                    }
                }
            }
        }
        if let Action::AirDash(air) = controller.action {
            // Source spends each timer increment, while accumulated use determines end minimum.
            controller
                .stamina
                .decrease(sensory_debit, config.resources.maximum);
            if !command.dash_held
                || controller.stamina.current <= 0.0
                || air.elapsed >= config.air_duration + 0.1
            {
                let remainder = if !motor.grounded {
                    (config.resources.dash_cost - air.sensory_used).max(0.0)
                } else {
                    0.0
                };
                controller
                    .stamina
                    .decrease(remainder, config.resources.maximum);
                motor.velocity = Vec3::ZERO;
                controller.pending.add_impulse(air_dash_release(
                    command.aim,
                    air.incoming_speed,
                    config.air_forward_force,
                    config.air_upward_force,
                    config.air_gravity_multiplier,
                    0.0,
                ));
                controller.action = Action::Locomotion;
                controller.air_ready = false;
                controller.result = ActionResult::AirDashReleased;
            }
        }
        controller.pending.add_impulse(dash_end_impulse);
        if controller.pending.apply(&mut motor.velocity, dt) {
            motor.grounded = false;
        }
        if matches!(
            controller.action,
            Action::Locomotion | Action::Slide | Action::AirDash(_)
        ) {
            motor.velocity.y -= config.gravity * dt;
        }
        controller.stamina.regenerate(
            config.resources,
            ResourceTick {
                delta: dt,
                world_factor: clocks.world_factor,
                player_factor: clocks.player_factor,
                paused: clocks.paused,
            },
        );
        command.jump = false;
        command.dash = false;
    }
}

fn finish_grapple(
    clocks: Res<ControllerClocks>,
    mut anchors: Query<&mut GrappleAnchor>,
    mut players: Query<(&Position, &mut Controller)>,
) {
    for (position, mut controller) in &mut players {
        if let Action::Grapple(grapple) = controller.action {
            let released = grapple.should_release(position.0);
            if released || grapple.finished() {
                if let Ok(mut anchor) = anchors.get_mut(grapple.point) {
                    anchor.occupied = false;
                    anchor.available_after = anchor.available_after.max(clocks.audio_elapsed + 0.3);
                }
                if released {
                    controller.pending.queue_launch(grapple.release_velocity());
                }
                controller.action = Action::Locomotion;
                controller.result = if released {
                    ActionResult::GrappleReleased
                } else {
                    ActionResult::GrappleCancelled
                };
                // HOST ADAPTATION: montage is omitted; same-point gate opens at completion,
                // while the source point's own cooldown/reset delay is still preserved.
            }
        }
    }
}

/// Empty-loadout timeline admission from raw source axes. Landing and slide-end
/// gates remain open; world-only callers use an explicitly adapted axis projection.
fn update_speed_target(
    controller: &mut Controller,
    config: &ControllerConfig,
    axes: Vec2,
    incoming_speed: f32,
    dt: f32,
    clocks: &ControllerClocks,
) {
    let gate = match classify_input(axes) {
        SpeedInput::Forward => SpeedGate::Forward,
        SpeedInput::Other if axes.y < 0.0 => SpeedGate::Backward,
        SpeedInput::Other => SpeedGate::Side,
        SpeedInput::NoNewTimeline => SpeedGate::Idle,
    };
    if gate != controller.speed_gate && gate != SpeedGate::Idle {
        if gate == SpeedGate::Forward {
            controller.speed.stop_other();
            controller.speed.begin_forward(incoming_speed, false);
        } else {
            controller
                .speed
                .begin_other(incoming_speed, false, 0.0, config.speed);
        }
    }
    controller.speed_gate = gate;
    controller.speed.advance(
        dt,
        dt * clocks.global_time_dilation * clocks.custom_time_dilation,
    );
    if let Some(target) = controller.speed.target(config.speed, 0.0) {
        controller.maximum_speed = target;
    }
}
