//! Ordinary native pending-operation boundary before root motion and physics dispatch.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct PendingMovement {
    launch: Vec3,
    impulse: Vec3,
    force: Vec3,
}

impl PendingMovement {
    /// LaunchCharacter has already resolved the horizontal/vertical override flags.
    pub fn queue_launch(&mut self, velocity: Vec3) {
        self.launch = velocity;
    }
    pub fn add_impulse(&mut self, impulse: Vec3) {
        self.impulse += impulse;
    }
    pub fn add_force(&mut self, force: Vec3) {
        self.force += force;
    }

    /// PerformMovement 0x3838620 calls GR2's force consumer before pending launch.
    /// A nonzero launch replaces the resulting velocity. Zero is the source no-op sentinel.
    pub fn apply(&mut self, velocity: &mut Vec3, delta: f32) -> bool {
        let requests = std::mem::take(self);
        *velocity = (requests.force * delta + requests.impulse) + *velocity;
        if requests.launch != Vec3::ZERO {
            *velocity = requests.launch;
            true
        } else {
            false
        }
    }
}
