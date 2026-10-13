use bevy::prelude::*;
use gr2_controller::speed::*;

#[test]
fn raw_axes_use_source_thresholds_instead_of_world_direction_angle() {
    assert_eq!(classify_input(Vec2::new(1.0, 0.5)), SpeedInput::Forward);
    assert_eq!(classify_input(Vec2::new(1.0, 0.199)), SpeedInput::Other);
    assert_eq!(classify_input(Vec2::new(0.0, -0.01)), SpeedInput::Other);
    assert_eq!(
        classify_input(Vec2::new(1.0, 0.2)),
        SpeedInput::NoNewTimeline
    );
    assert_eq!(classify_input(Vec2::ZERO), SpeedInput::NoNewTimeline);
}

#[test]
fn forward_captures_once_and_preserves_slide_written_current_speed() {
    let config = SpeedConfig::default();
    let mut state = SpeedState::default();
    state.begin_forward(10.0, false);
    assert_eq!(state.captured_speed, 8.0);
    assert_eq!(state.target(config, 0.0), Some(8.0));
    state.advance(1.5, 0.15);
    assert_eq!(state.captured_speed, 8.0);
    assert_eq!(state.target(config, 0.0), Some(10.0));
    state.captured_speed = 9.99;
    state.begin_forward(20.0, true);
    assert_eq!(state.profile, SpeedProfile::Forward);
    assert_eq!(state.captured_speed, 9.99);
}

#[test]
fn high_incoming_forward_speed_uses_distinct_dilated_recovery_curve() {
    let config = SpeedConfig::default();
    let mut state = SpeedState::default();
    state.begin_forward(20.0, false);
    assert_eq!(state.captured_speed, 12.0);
    assert_eq!(state.profile, SpeedProfile::ForwardRecovery);
    state.advance(1.5, 0.15);
    assert_eq!(state.elapsed, 0.15);
    assert!(state.target(config, 0.0).unwrap() > 11.0);
    for key in FORWARD_RECOVERY.keys {
        assert_eq!(FORWARD_RECOVERY.evaluate(key.time), key.value);
    }
}

#[test]
fn other_selects_ascent_or_captured_linear_deceleration_at_eight_metres() {
    let config = SpeedConfig::default();
    let mut state = SpeedState::default();
    state.begin_other(7.99, false, 0.0, config);
    assert_eq!(state.profile, SpeedProfile::OtherAcceleration);
    assert_eq!(state.target(config, 0.0), Some(5.0));
    state.advance(1.0, 0.1);
    assert_eq!(state.elapsed, 0.1);
    state.begin_other(10.0, false, 0.0, config);
    assert_eq!(state.profile, SpeedProfile::OtherDeceleration);
    state.advance(0.5, 0.5);
    assert_eq!(state.target(config, 0.0), Some(8.5));
    state.advance(1.0, 1.0);
    assert_eq!(state.elapsed, 1.5);
    assert_eq!(state.target(config, 0.0), Some(7.0));
    assert_eq!(OTHER_DECELERATION.length, 2.0);
    state.begin_forward(5.0, false);
    state.begin_other(8.0, true, 0.0, config);
    assert_eq!(state.target(config, 0.0), None);
    state.begin_other(7.0, false, 0.0, config);
    state.advance(0.25, 0.25);
    state.begin_other(8.0, true, 0.0, config);
    assert_eq!(state.profile, SpeedProfile::OtherAcceleration);
    assert_eq!(state.elapsed, 0.25);
}

#[test]
fn source_stop_resets_capture_conditionally_and_preserves_unchanged_target() {
    let config = SpeedConfig::default();
    let mut state = SpeedState::default();
    state.begin_forward(10.0, false);
    state.stop_forward(0.25, config);
    assert_eq!(state.captured_speed, 8.0);
    assert_eq!(state.target(config, 0.25), None);
    state.begin_forward(10.0, false);
    state.stop_forward(0.0, config);
    assert_eq!(state.captured_speed, 7.0);
    assert_eq!(state.target(config, 0.0), None);
}

#[test]
fn other_deceleration_caps_capture_using_current_speed_modifier() {
    let config = SpeedConfig::default();
    let mut state = SpeedState::default();
    state.begin_other(20.0, false, 0.5, config);
    assert_eq!(state.captured_speed, 20.0);
    assert_eq!(state.target(config, 0.0), Some(12.0));
    assert_eq!(state.target(config, 0.5), Some(18.0));
}
