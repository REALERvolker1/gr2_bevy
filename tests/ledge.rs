//! Source query specifications from original build16083277; no UE runtime parity claim.
use bevy::prelude::*;
use gr2_controller::{ledge::*, mantle::MantleConfig};

fn context() -> LedgeContext {
    LedgeContext {
        owner_position: Vec3::new(1.0, 2.0, 3.0),
        character_position: Vec3::new(4.0, 5.0, 6.0),
        movement_forward: Vec3::NEG_Z,
        camera_forward: Vec3::NEG_Z,
        radius: 0.4,
        half_height: 0.9,
    }
}
fn config() -> MantleConfig {
    MantleConfig::default().adjusted_to_capsule(0.4, 0.9)
}
fn contact(entity: Entity, point: Vec3) -> LedgeContact {
    LedgeContact {
        entity,
        impact_point: point,
        normal: Vec3::Z,
        origin: Vec3::ONE,
        surface: ClimbSurface::default(),
    }
}
fn near(actual: Vec3, expected: Vec3) {
    assert!(
        actual.distance(expected) < 1e-6,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn original_box_sphere_and_retry_capsules_use_impact_points_and_distinct_owners() {
    let mut world = World::new();
    let wall = contact(world.spawn_empty().id(), Vec3::new(1.0, 2.0, 2.4));
    let mut top = contact(world.spawn_empty().id(), Vec3::new(1.0, 3.0, 2.4));
    top.normal = Vec3::X; // No top walkable-normal test appears in the original body.
    let replies = [None, Some(wall), Some(top), Some(wall), None];
    let mut requests = Vec::new();
    let found = detect_ledge(context(), config(), false, |probe| {
        let result = replies[requests.len()];
        requests.push(probe);
        result
    })
    .unwrap();
    assert_eq!(found.entity, top.entity); // Wall and top need not belong to one actor.
    assert_eq!(requests.len(), 5);
    near(requests[0].start, Vec3::new(4.0, 5.0, 6.0));
    near(requests[0].end, Vec3::new(1.0, 6.8, 3.0));
    assert_eq!(
        requests[0].shape,
        ProbeShape::Capsule {
            radius: 0.4,
            half_height: 0.9
        }
    );
    near(requests[1].start, Vec3::new(1.0, 2.5, 3.0));
    near(requests[1].end, Vec3::new(1.0, 2.5, 2.4));
    assert_eq!(
        requests[1].shape,
        ProbeShape::Box {
            half_extents: Vec3::new(0.4, 1.3, 0.4)
        }
    );
    near(requests[2].start, Vec3::new(1.0, 5.0, 2.4));
    near(requests[2].end, wall.impact_point);
    assert_eq!(requests[2].shape, ProbeShape::Sphere { radius: 0.4 });
    near(requests[3].start, Vec3::new(1.0, 4.4, 2.4));
    near(requests[3].end, Vec3::new(1.0, 4.4, 2.0));
    near(requests[4].start, Vec3::new(1.0, 4.9, 2.8));
    near(requests[4].end, Vec3::new(1.0, 4.9, 2.0));
}

#[test]
fn overhead_and_both_clearance_blocks_reject_but_first_clearance_block_alone_does_not() {
    let hit = contact(Entity::PLACEHOLDER, Vec3::new(1.0, 3.0, 2.4));
    let mut calls = 0;
    assert!(
        detect_ledge(context(), config(), true, |_| {
            calls += 1;
            Some(hit)
        })
        .is_none()
    );
    assert_eq!(calls, 1);
    let replies = [None, Some(hit), Some(hit), Some(hit), Some(hit)];
    let mut calls = 0;
    assert!(
        detect_ledge(context(), config(), true, |_| {
            let response = replies[calls];
            calls += 1;
            response
        })
        .is_none()
    );
    assert_eq!(calls, 5);
}

#[test]
fn forced_call_bypasses_facing_only_and_block_tag_preserves_query_success() {
    let mut ctx = context();
    ctx.camera_forward = Vec3::Z;
    let wall = contact(Entity::PLACEHOLDER, Vec3::new(1.0, 2.0, 2.4));
    let mut top = contact(Entity::PLACEHOLDER, Vec3::new(1.0, 3.0, 2.4));
    top.surface.component_block_climb = true;
    for forced in [false, true] {
        let replies = [None, Some(wall), Some(top), None];
        let mut calls = 0;
        let found = detect_ledge(ctx, config(), forced, |_| {
            let reply = replies[calls];
            calls += 1;
            reply
        });
        if forced {
            assert!(!found.unwrap().permits_initialization());
            assert_eq!(calls, 4);
        } else {
            assert!(found.is_none());
            assert_eq!(calls, 2);
        }
    }
    for kind in [
        ClimbActorKind::EnemyBaseCharacter,
        ClimbActorKind::BaseProjectile,
        ClimbActorKind::ShurikenProjectile,
        ClimbActorKind::SurgeWaveElement,
    ] {
        let mut wall = wall;
        wall.surface.actor_kind = kind;
        let mut calls = 0;
        assert!(
            detect_ledge(ctx, config(), true, |_| {
                calls += 1;
                if calls == 1 { None } else { Some(wall) }
            })
            .is_none()
        );
        assert_eq!(calls, 2);
    }
}

#[test]
fn presentation_thresholds_are_strict_and_forced_mounts_select_high() {
    let cfg = config();
    assert_eq!(
        climb_presentation(2.0, 1.0, false, cfg),
        ClimbPresentation::Low
    );
    assert_eq!(
        climb_presentation(2.0001, 1.0, false, cfg),
        ClimbPresentation::High
    );
    assert_eq!(
        climb_presentation(1.0, 1.0, false, cfg),
        ClimbPresentation::None
    );
    assert_eq!(
        climb_presentation(0.0, 1.0, true, cfg),
        ClimbPresentation::High
    );
}
