use gr2_controller::curves::{Curve, FORWARD_SPEED, GROUND_DASH, Interpolation, Key, OTHER_SPEED};

#[test]
fn recovered_curves_reproduce_each_authored_key() {
    for curve in [FORWARD_SPEED, OTHER_SPEED, GROUND_DASH] {
        for key in curve.keys {
            assert_eq!(curve.evaluate(key.time), key.value);
        }
    }
}

#[test]
fn ground_dash_midpoint_uses_recovered_tangents() {
    assert!((GROUND_DASH.evaluate(0.175) - 0.6345713).abs() < 0.0000002);
}

#[test]
fn forward_curve_clamps_at_boundaries_and_retains_timeline_metadata() {
    assert_eq!(FORWARD_SPEED.evaluate(-1.0), 0.0);
    assert_eq!(FORWARD_SPEED.evaluate(0.0), 0.0);
    assert_eq!(FORWARD_SPEED.evaluate(1.5), 1.0);
    assert_eq!(FORWARD_SPEED.evaluate(100.0), 1.0);
    assert_eq!(FORWARD_SPEED.length, 1.5);
    assert!(FORWARD_SPEED.ignore_time_dilation);
    assert!(!OTHER_SPEED.ignore_time_dilation);
}

#[test]
fn segment_uses_left_key_interpolation_mode() {
    let curve = Curve {
        length: 1.0,
        ignore_time_dilation: false,
        keys: &[
            Key {
                time: 0.0,
                value: 0.0,
                arrive_tangent: 100.0,
                leave_tangent: 100.0,
                interpolation: Interpolation::Linear,
            },
            Key {
                time: 1.0,
                value: 1.0,
                arrive_tangent: 0.0,
                leave_tangent: 0.0,
                interpolation: Interpolation::Cubic,
            },
        ],
    };
    assert_eq!(curve.evaluate(0.25), 0.25);
}
