//! Cooked SlidingLaunch records, independently read from the owned package.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlideBand {
    pub minimum: f32,
    pub maximum: f32,
    pub launch: f32,
    pub fov: f32,
}

pub const FLAT: [SlideBand; 5] = [
    SlideBand {
        minimum: 0.0,
        maximum: 1.0,
        launch: 1000.0,
        fov: 0.0,
    },
    SlideBand {
        minimum: 0.0,
        maximum: 700.0,
        launch: 1000.0,
        fov: 0.0,
    },
    SlideBand {
        minimum: 700.0,
        maximum: 850.0,
        launch: 1400.0,
        fov: 0.0,
    },
    SlideBand {
        minimum: 850.0,
        maximum: 950.0,
        launch: 1600.0,
        fov: 3.0,
    },
    SlideBand {
        minimum: 950.0,
        maximum: 2000.0,
        launch: 1800.0,
        fov: 8.0,
    },
];
pub const SLOPE: [SlideBand; 4] = [
    SlideBand {
        minimum: 0.0,
        maximum: 1.0,
        launch: 1500.0,
        fov: 0.0,
    },
    SlideBand {
        minimum: 0.0,
        maximum: 800.0,
        launch: 2000.0,
        fov: 5.0,
    },
    SlideBand {
        minimum: 800.0,
        maximum: 1000.0,
        launch: 2500.0,
        fov: 7.0,
    },
    SlideBand {
        minimum: 1000.0,
        maximum: 1400.0,
        launch: 3000.0,
        fov: 10.0,
    },
];

/// Strict bounds preserve holes at exact endpoints. The caller supplies full 3D speed.
pub fn select(bands: &[SlideBand], speed: f32) -> Option<SlideBand> {
    bands
        .iter()
        .copied()
        .find(|band| band.minimum < speed && speed < band.maximum)
}

pub fn has_slide_input(forward: f32, right: f32) -> bool {
    forward.abs() + right.abs() >= 0.2
}

/// The probe result is a height difference in source centimetres, not an angle.
pub fn downhill_probe(height_difference: f32) -> bool {
    -100.0 < height_difference && height_difference < -5.0
}
