//! Native UClimbComponent query order and geometry, GR2 build 16083277.
//! Source RVAs 0x1a49ac0 -> 0x1a25440 -> 0x1a259f0. Coordinates are Y-up metres.
use crate::mantle::MantleConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Cooked SCS FollowCamera template export261: UE(-10,0,70)cm.
/// Expressed in Bevy actor-local coordinates; runtime camera effects are separate.
pub const SOURCE_CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 0.70, 0.10);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClimbActorKind {
    #[default]
    Ordinary,
    EnemyBaseCharacter,
    BaseProjectile,
    ShurikenProjectile,
    SurgeWaveElement,
    /// Native class guard reads an unnamed flag at actor +0x230.
    StaticMeshActorOml {
        flag_230: bool,
    },
}

impl ClimbActorKind {
    fn rejects_wall(self) -> bool {
        matches!(
            self,
            Self::EnemyBaseCharacter
                | Self::BaseProjectile
                | Self::ShurikenProjectile
                | Self::SurgeWaveElement
        )
    }
}

/// Host metadata for source actor classes and actor/component BlockClimb tags.
/// Absent metadata means an ordinary blocking collider, not an unmountable surface.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ClimbSurface {
    pub actor_kind: ClimbActorKind,
    pub actor_block_climb: bool,
    pub component_block_climb: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct LedgeContext {
    pub owner_position: Vec3,
    pub character_position: Vec3,
    /// Source obtains this from global player 0's actor root, not the camera.
    pub movement_forward: Vec3,
    pub camera_forward: Vec3,
    pub radius: f32,
    pub half_height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProbeShape {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgeProbe {
    pub start: Vec3,
    pub end: Vec3,
    pub shape: ProbeShape,
}

#[derive(Clone, Copy, Debug)]
pub struct LedgeContact {
    pub entity: Entity,
    /// UE ImpactPoint. Avian's point on the hit shape is the host mapping.
    pub impact_point: Vec3,
    /// UE Normal, not ImpactNormal. Avian's outward hit normal is the host mapping.
    pub normal: Vec3,
    pub origin: Vec3,
    pub surface: ClimbSurface,
}

impl LedgeContact {
    pub fn permits_initialization(self) -> bool {
        !self.surface.actor_block_climb && !self.surface.component_block_climb
    }
}

fn planar_normalize(vector: Vec3) -> Vec3 {
    let vector = Vec3::new(vector.x, 0.0, vector.z);
    let squared = vector.length_squared();
    // The source direction tolerance is dimensionless, not a cm-to-m conversion.
    if squared < 1e-8 {
        Vec3::ZERO
    } else {
        vector / squared.sqrt()
    }
}

/// Native query result is distinct from mounting initialization. BlockClimb and
/// a zero origin can leave TryClimb returning true without bIsClimbing becoming true.
pub fn detect_ledge(
    context: LedgeContext,
    config: MantleConfig,
    forced: bool,
    mut cast: impl FnMut(LedgeProbe) -> Option<LedgeContact>,
) -> Option<LedgeContact> {
    let capsule = ProbeShape::Capsule {
        radius: context.radius,
        half_height: context.half_height,
    };
    let overhead = LedgeProbe {
        start: context.character_position,
        end: context.owner_position
            + Vec3::Y * (2.0 * context.half_height + config.max_climb_height),
        shape: capsule,
    };
    if cast(overhead).is_some() {
        return None;
    }
    let forward = planar_normalize(context.movement_forward);
    let start = context.owner_position + Vec3::Y * (config.max_climb_height * 0.5 - 1.0);
    let wall = cast(LedgeProbe {
        start,
        end: start + forward * config.max_wall_forward,
        shape: ProbeShape::Box {
            half_extents: Vec3::new(
                context.radius,
                config.bottom_ledge_max_distance + 0.5,
                context.radius,
            ),
        },
    })?;
    if wall.surface.actor_kind.rejects_wall() {
        return None;
    }
    if !forced {
        let cosine = planar_normalize(context.camera_forward)
            .dot(wall.normal)
            .clamp(-1.0, 1.0);
        if 180.0 - cosine.acos() * 57.295776 > 20.0 {
            return None;
        }
    }
    let top = cast(LedgeProbe {
        start: wall.impact_point + Vec3::Y * config.max_climb_height,
        end: wall.impact_point,
        shape: ProbeShape::Sphere {
            radius: context.radius,
        },
    })?;
    let start = top.impact_point + Vec3::Y * (context.half_height + 0.5);
    if cast(LedgeProbe {
        start,
        end: start + forward * (context.radius + config.ledge_depth_check_offset),
        shape: capsule,
    })
    .is_some()
    {
        // Native retries only after the first clearance sweep blocks. Neither
        // a slope test nor equality of wall/top actor identities is present.
        let start = start + Vec3::Y * 0.5 - forward * context.radius;
        if cast(LedgeProbe {
            start,
            end: start + forward * (2.0 * context.radius + config.ledge_depth_check_offset),
            shape: capsule,
        })
        .is_some()
        {
            return None;
        }
    }
    if matches!(
        top.surface.actor_kind,
        ClimbActorKind::StaticMeshActorOml { flag_230: false }
    ) {
        return None;
    }
    Some(top)
}

/// Avian adapter for the configured ECC_WorldStatic probes. Fixture colliders
/// block this channel by default. Actor/component hierarchy and PhysX contact
/// generation remain host mappings; this does not equate Avian hits to UE hits.
pub fn query_ledge(
    spatial: &SpatialQuery,
    surfaces: &Query<(&Position, Option<&ClimbSurface>)>,
    player: Entity,
    context: LedgeContext,
    config: MantleConfig,
    forced: bool,
) -> Option<LedgeContact> {
    let filter = SpatialQueryFilter::from_excluded_entities([player]);
    detect_ledge(context, config, forced, |probe| {
        let collider = match probe.shape {
            ProbeShape::Box { half_extents } => Collider::cuboid(
                half_extents.x * 2.0,
                half_extents.y * 2.0,
                half_extents.z * 2.0,
            ),
            ProbeShape::Sphere { radius } => Collider::sphere(radius),
            ProbeShape::Capsule {
                radius,
                half_height,
            } => Collider::capsule(radius, (half_height - radius) * 2.0),
        };
        let delta = probe.end - probe.start;
        // All original sweeps use the identity quaternion. Avian needs a unit
        // direction even for a zero-length request; the zero distance retains it.
        let direction = Dir3::new(delta).unwrap_or(Dir3::Y);
        let hit = spatial.cast_shape(
            &collider,
            probe.start,
            Quat::IDENTITY,
            direction,
            &ShapeCastConfig::from_max_distance(delta.length()),
            &filter,
        )?;
        let (position, metadata) = surfaces.get(hit.entity).ok()?;
        Some(LedgeContact {
            entity: hit.entity,
            impact_point: hit.point1,
            normal: hit.normal1,
            origin: position.0,
            surface: metadata.copied().unwrap_or_default(),
        })
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClimbPresentation {
    #[default]
    None,
    High,
    Low,
}

/// Native TryClimbing 0x1c828b0 compares stored ledge height to camera world height.
pub fn climb_presentation(
    ledge_y: f32,
    camera_y: f32,
    forced: bool,
    config: MantleConfig,
) -> ClimbPresentation {
    let height = ledge_y - camera_y;
    if height > config.normal_ledge_height || forced {
        ClimbPresentation::High
    } else if height > config.low_ledge_height {
        ClimbPresentation::Low
    } else {
        ClimbPresentation::None
    }
}
