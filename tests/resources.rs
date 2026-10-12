use gr2_controller::resources::*;
use gr2_controller::slide;

#[test]
fn dash_admits_insufficient_positive_stamina_and_blocked_spending_updates_clock() {
    let config = ResourceConfig::default();
    let mut stamina = Stamina::new(config);
    stamina.current = 1.0;
    assert_eq!(stamina.admit_dash(1.0, config), DashAdmission::Accepted);
    stamina.decrease(40.0, config.maximum);
    assert_eq!(stamina.current, -39.0);
    assert_eq!(stamina.admit_dash(1.1, config), DashAdmission::Cooldown);
    assert_eq!(stamina.admit_dash(2.0, config), DashAdmission::Depleted);
    stamina.current = 1.0;
    stamina.decrease_enabled = false;
    assert_eq!(stamina.admit_dash(3.0, config), DashAdmission::Accepted);
    assert_eq!(stamina.decrease(40.0, config.maximum), None);
    assert_eq!(stamina.last_dash_audio, 3.0);
    assert_eq!(stamina.current, 1.0);
}

#[test]
fn regeneration_keeps_starting_sign_and_native_clamp_branches() {
    let config = ResourceConfig::default();
    let mut stamina = Stamina::new(config);
    stamina.current = -1.0;
    let tick = ResourceTick {
        delta: 0.1,
        world_factor: 1.0,
        player_factor: 1.0,
        paused: false,
    };
    stamina.regenerate(config, tick);
    assert_eq!(stamina.current, 1.0);
    stamina.regenerate(config, tick);
    assert_eq!(stamina.current, 4.5);
    stamina.regeneration_enabled = false;
    assert_eq!(stamina.regenerate(config, tick), None);
    assert_eq!(stamina.set(f32::NAN, 100.0), -1.5);
    assert_eq!(stamina.set(200.0, 100.0), 1.0);
}

#[test]
fn source_tick_phase_remains_two_distinct_results() {
    let config = ResourceConfig::default();
    let tick = ResourceTick {
        delta: 0.0096731186,
        world_factor: 1.0,
        player_factor: 1.0,
        paused: false,
    };
    let mut before = Stamina::new(config);
    before.regenerate(config, tick);
    before.decrease(44.0, 100.0);
    let mut after = Stamina::new(config);
    after.decrease(44.0, 100.0);
    after.regenerate(config, tick);
    assert_eq!(before.current, 56.0);
    assert!((after.current - 56.3385506).abs() < 0.0001);
}

#[test]
fn ordered_cost_and_strict_slide_bands() {
    let modifiers = DashModifiers {
        invulnerable: 0.1,
        longer: 0.25,
        cheaper: 0.25,
        ..Default::default()
    };
    assert_eq!(modifiers.cost(0.0, 40.0), 44.0);
    assert_eq!(slide::select(&slide::FLAT, 700.0), None);
    assert_eq!(slide::select(&slide::FLAT, 850.0), None);
    assert_eq!(slide::select(&slide::FLAT, 850.1).unwrap().launch, 1600.0);
    assert!(slide::has_slide_input(0.1, -0.1));
    assert!(!slide::downhill_probe(-100.0));
    assert!(!slide::downhill_probe(-5.0));
    assert!(slide::downhill_probe(-5.1));
}
