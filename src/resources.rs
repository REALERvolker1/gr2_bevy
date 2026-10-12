//! Recovered float32 arithmetic. Source locators are in docs/source-boundaries.md.
//! Callers resolve upgrade membership, maximum, conversion and clock factors.

#[derive(Clone, Copy, Debug)]
pub struct ResourceConfig {
    pub maximum: f32,
    pub regeneration: f32,
    pub negative_regeneration: f32,
    pub dash_cost: f32,
    pub dash_cooldown: f32,
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            maximum: 100.0,
            regeneration: 35.0,
            negative_regeneration: 20.0,
            dash_cost: 40.0,
            dash_cooldown: 0.2,
        }
    }
}

/// These clocks are separate even when the host maps both factors to one.
#[derive(Clone, Copy, Debug)]
pub struct ResourceTick {
    pub delta: f32,
    pub world_factor: f32,
    pub player_factor: f32,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashAdmission {
    Accepted,
    Cooldown,
    Depleted,
}

#[derive(Clone, Copy, Debug)]
pub struct Stamina {
    pub current: f32,
    pub last_dash_audio: f32,
    pub decrease_enabled: bool,
    pub regeneration_enabled: bool,
}

impl Stamina {
    pub fn new(config: ResourceConfig) -> Self {
        Self {
            current: config.maximum,
            last_dash_audio: 0.0,
            decrease_enabled: true,
            regeneration_enabled: true,
        }
    }

    /// The unordered branch follows the native setter rather than Rust's clamp.
    pub fn set(&mut self, requested: f32, maximum: f32) -> f32 {
        self.current = if requested >= -150.0 {
            if requested <= maximum {
                requested
            } else {
                maximum
            }
        } else {
            -150.0
        };
        (1.0 / maximum) * self.current
    }

    pub fn regenerate(&mut self, config: ResourceConfig, tick: ResourceTick) -> Option<f32> {
        if tick.paused || !self.regeneration_enabled {
            return None;
        }
        let rate = if self.current < 0.0 {
            config.negative_regeneration
        } else {
            config.regeneration
        };
        // Preserve source operation order, including the starting-sign rate across zero.
        Some(self.set(
            self.current + tick.delta * tick.world_factor * rate * tick.player_factor,
            config.maximum,
        ))
    }

    /// Admission precedes conversion/spending. Blocked decrease still advances cooldown.
    pub fn admit_dash(&mut self, audio_time: f32, config: ResourceConfig) -> DashAdmission {
        if !(config.dash_cooldown <= audio_time - self.last_dash_audio) {
            return DashAdmission::Cooldown;
        }
        if !(self.current > 0.0) {
            return DashAdmission::Depleted;
        }
        self.last_dash_audio = audio_time;
        DashAdmission::Accepted
    }

    pub fn decrease(&mut self, resolved_debit: f32, maximum: f32) -> Option<f32> {
        self.decrease_enabled
            .then(|| self.set(self.current - resolved_debit, maximum))
    }
}

/// Resolved coefficients in native order, including signed HalfDash values.
#[derive(Clone, Copy, Debug, Default)]
pub struct DashModifiers {
    pub half_dash: f32,
    pub invulnerable: f32,
    pub longer: f32,
    pub extra: f32,
    pub cheaper: f32,
    pub roguelike_reductions: [f32; 4],
}

impl DashModifiers {
    pub fn cost(self, requested: f32, configured_base: f32) -> f32 {
        let base = if requested <= 0.0 {
            configured_base
        } else {
            requested
        };
        let mut adjustment = base * self.half_dash
            + base * self.invulnerable
            + base * self.longer
            + base * self.extra;
        adjustment -= base * self.cheaper;
        for reduction in self.roguelike_reductions {
            adjustment -= base * reduction;
        }
        adjustment + base
    }
}
