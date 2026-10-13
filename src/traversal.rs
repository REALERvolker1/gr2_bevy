//! Recovered wallrun guards. Surface classification and query scheduling stay in the host.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct WallRunConfig {
    pub maximum_forward_speed: f32,
    pub minimum_speed_fraction: f32,
    pub minimum_view_angle: f32,
    pub maximum_view_angle: f32,
    pub initial_speed: f32,
    pub final_speed: f32,
}

impl Default for WallRunConfig {
    fn default() -> Self {
        Self {
            maximum_forward_speed: 10.0,
            minimum_speed_fraction: 0.1,
            minimum_view_angle: 10.0_f32.to_radians(),
            maximum_view_angle: 140.0_f32.to_radians(),
            initial_speed: 13.5,
            final_speed: 10.0,
        }
    }
}

/// Original CanStartWallrunning speed/angle boundary after source state/class gates.
pub fn can_start_wallrun(
    velocity: Vec3,
    camera_forward: Vec3,
    surface_delta: Vec3,
    config: WallRunConfig,
) -> bool {
    let planar_speed = Vec2::new(velocity.x, velocity.z).length();
    if planar_speed < config.maximum_forward_speed * config.minimum_speed_fraction {
        return false;
    }
    let camera_planar = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize_or_zero();
    let angle = camera_planar
        .dot(surface_delta.normalize_or_zero())
        .clamp(-1.0, 1.0)
        .acos();
    config.minimum_view_angle < angle && angle < config.maximum_view_angle
}

/// Original ExecuteWallRun velocity/view gates, followed by CanKeep's strict speed check.
pub fn can_keep_wallrun(
    velocity: Vec3,
    camera_forward: Vec3,
    actor_forward: Vec3,
    wall_normal: Vec3,
    config: WallRunConfig,
) -> bool {
    let planar = Vec3::new(velocity.x, 0.0, velocity.z);
    planar.length() > config.maximum_forward_speed * 0.1
        && planar.dot(Vec3::new(camera_forward.x, 0.0, camera_forward.z)) > 0.0
        && velocity.normalize_or_zero().dot(camera_forward) >= 0.3
        && actor_forward.dot(wall_normal) >= -0.97
}

/// Native rotation uses UE Z-up. Coordinate mapping (x,y,z)->(y,z,-x) reverses its sign.
pub fn wall_forward(normal: Vec3, left_wall: bool) -> Vec3 {
    let angle = if left_wall {
        std::f32::consts::FRAC_PI_2
    } else {
        -std::f32::consts::FRAC_PI_2
    };
    Quat::from_rotation_y(angle) * normal
}

/// Host level marker standing in for the original usable-component/class filter.
#[derive(Component)]
pub struct WallRunnable;

#[derive(Clone, Copy, Debug)]
pub struct WallRun {
    pub entity: Entity,
    pub normal: Vec3,
    pub forward: Vec3,
    pub elapsed: f32,
    pub captured_speed: f32,
}

impl WallRun {
    pub fn speed(&self, config: WallRunConfig) -> f32 {
        // Qualified WallRunSlowdown cubic keys have zero endpoint tangents at 0 and 2s.
        let t = (self.elapsed / 2.0).clamp(0.0, 1.0);
        let alpha = 3.0 * t * t - 2.0 * t * t * t;
        self.captured_speed + (config.final_speed - self.captured_speed) * alpha
    }
}

/// Side rays are an explicit host adaptation to the native overlap/component classifier.
pub fn query_wall(
    spatial: &avian3d::prelude::SpatialQuery,
    tagged: &Query<(), With<WallRunnable>>,
    entity: Entity,
    position: Vec3,
    forward: Vec3,
    velocity: Vec3,
    radius: f32,
    config: WallRunConfig,
) -> Option<WallRun> {
    use avian3d::prelude::*;
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    for left in [true, false] {
        let direction = if left { -right } else { right };
        let Some(hit) = spatial.cast_ray(
            position,
            Dir3::new(direction).ok()?,
            radius * 1.5,
            true,
            &SpatialQueryFilter::from_excluded_entities([entity]),
        ) else {
            continue;
        };
        if tagged.get(hit.entity).is_err() {
            continue;
        }
        if !can_start_wallrun(velocity, forward, direction * hit.distance, config) {
            continue;
        }
        let tangent = wall_forward(hit.normal, left);
        if tangent.dot(forward) < 0.1 {
            continue;
        }
        return Some(WallRun {
            entity: hit.entity,
            normal: hit.normal,
            forward: tangent,
            elapsed: 0.0,
            captured_speed: velocity
                .length()
                .clamp(config.final_speed, config.initial_speed),
        });
    }
    None
}

#[derive(Clone, Debug)]
pub struct Mounting {
    pub motion: crate::mantle::Mantle,
    pub surface: Entity,
}
