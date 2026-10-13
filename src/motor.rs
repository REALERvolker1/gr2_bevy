//! Host adaptation: capsule collision and contact sensing supplied by Avian.
//!
//! This is not a reconstruction of Unreal's CharacterMovement physics. Gameplay
//! actions own velocity; this module owns its single integration into Position.
use avian3d::prelude::*;
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotorSystems {
    Sense,
    Move,
}

pub struct MotorPlugin;

impl Plugin for MotorPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (MotorSystems::Sense, MotorSystems::Move)
                .chain()
                // Avian 0.7 refreshes ColliderTrees during StepSimulation rather
                // than exposing the former SpatialQuery::update_pipeline API.
                .after(PhysicsSystems::StepSimulation)
                .before(PhysicsSystems::Writeback),
        )
        .add_systems(FixedUpdate, sense.in_set(MotorSystems::Sense))
        .add_systems(FixedUpdate, move_motor.in_set(MotorSystems::Move));
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MotorConfig {
    pub radius: f32,
    /// Full capsule height, including both hemispheres.
    pub standing_height: f32,
    pub crouching_height: f32,
    pub max_slope_angle: f32,
    pub ground_probe: f32,
    pub skin: f32,
}

impl Default for MotorConfig {
    fn default() -> Self {
        Self {
            radius: 0.35,
            standing_height: 1.8,
            crouching_height: 1.0,
            max_slope_angle: 45.0_f32.to_radians(),
            ground_probe: 0.08,
            skin: 0.002,
        }
    }
}

#[derive(Component, Debug, Default, Clone)]
#[require(
    RigidBody::Kinematic,
    CustomPositionIntegration,
    SpeculativeMargin(0.0),
    Collider = default_capsule()
)]
pub struct Motor {
    pub velocity: Vec3,
    /// One tick of action-authored translation, swept by the host collision
    /// motor without converting the action curve into physical momentum.
    pub displacement: Option<Vec3>,
    pub grounded: bool,
    pub ground_normal: Option<Vec3>,
    pub crouched: bool,
    pub request_crouch: bool,
    pub config: MotorConfig,
}

fn default_capsule() -> Collider {
    capsule(MotorConfig::default(), false)
}

fn capsule(config: MotorConfig, crouched: bool) -> Collider {
    let height = if crouched {
        config.crouching_height
    } else {
        config.standing_height
    };
    Collider::capsule(config.radius, (height - 2.0 * config.radius).max(0.0))
}

impl Motor {
    pub fn height(&self) -> f32 {
        if self.crouched {
            self.config.crouching_height
        } else {
            self.config.standing_height
        }
    }

    pub fn collider(&self) -> Collider {
        capsule(self.config, self.crouched)
    }
}

fn sense(
    mut params: ParamSet<(
        SpatialQuery,
        Query<(Entity, &mut Motor, &mut Position, &Rotation, &mut Collider)>,
    )>,
) {
    // Query helpers read collider poses. Keep their borrows separate from the
    // authoritative pose writes instead of bypassing Bevy's alias checks.
    let entities: Vec<Entity> = params.p1().iter().map(|row| row.0).collect();
    for entity in entities {
        let (mut motor, mut position, rotation, mut collider) = {
            let query = params.p1();
            let (_, motor, position, rotation, collider) = query.get(entity).unwrap();
            (motor.clone(), *position, *rotation, collider.clone())
        };
        let spatial = params.p0();
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        if motor.request_crouch != motor.crouched {
            let next_height = if motor.request_crouch {
                motor.config.crouching_height
            } else {
                motor.config.standing_height
            };
            let next_position = position.0 + Vec3::Y * (next_height - motor.height()) * 0.5;
            let next_collider = capsule(motor.config, motor.request_crouch);
            // Preserve the feet while resizing. Standing requires the entire
            // enlarged capsule to fit, including sideways overhead obstacles.
            let clear = motor.request_crouch
                || spatial
                    .shape_intersections(&next_collider, next_position, rotation.0, &filter)
                    .is_empty();
            if clear {
                position.0 = next_position;
                collider = next_collider;
                motor.crouched = motor.request_crouch;
            }
        }
        let ground = spatial.cast_shape(
            &collider,
            position.0,
            rotation.0,
            Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(motor.config.ground_probe),
            &filter,
        );
        motor.ground_normal = ground
            .filter(|hit| hit.normal1.y >= motor.config.max_slope_angle.cos())
            .map(|hit| hit.normal1);
        // A jump can remain within the probe distance for several frames.
        motor.grounded = motor.ground_normal.is_some() && motor.velocity.y <= 0.0;
        let mut query = params.p1();
        let (_, mut stored_motor, mut stored_position, _, mut stored_collider) =
            query.get_mut(entity).unwrap();
        *stored_motor = motor;
        *stored_position = position;
        *stored_collider = collider;
    }
}

fn move_motor(
    time: Res<Time<Fixed>>,
    mut params: ParamSet<(
        MoveAndSlide,
        Query<(Entity, &mut Motor, &mut Position, &Rotation, &Collider)>,
    )>,
) {
    let entities: Vec<Entity> = params.p1().iter().map(|row| row.0).collect();
    for entity in entities {
        let (motor, position, rotation, collider) = {
            let query = params.p1();
            let (_, motor, position, rotation, collider) = query.get(entity).unwrap();
            (motor.clone(), *position, *rotation, collider.clone())
        };
        let mut contact_normals = Vec::new();
        let sweep_velocity = motor
            .displacement
            .map_or(motor.velocity, |delta| delta / time.delta_secs());
        let output = params.p0().move_and_slide(
            &collider,
            position.0,
            rotation.0,
            sweep_velocity,
            time.delta(),
            &MoveAndSlideConfig {
                skin_width: motor.config.skin,
                move_and_slide_iterations: 8,
                ..default()
            },
            &SpatialQueryFilter::from_excluded_entities([entity]),
            |hit| {
                contact_normals.push(*hit.normal);
                MoveAndSlideHitResponse::Accept
            },
        );
        let mut query = params.p1();
        let (_, mut motor, mut position, _, _) = query.get_mut(entity).unwrap();
        position.0 = output.position;
        motor.velocity = if motor.displacement.take().is_some() {
            // The original action teleports along its timeline. Sweeping that
            // translation is a host adaptation; only collision constraints
            // affect the retained physical velocity.
            MoveAndSlide::project_velocity(motor.velocity, &contact_normals)
        } else {
            output.projected_velocity
        };
    }
}
