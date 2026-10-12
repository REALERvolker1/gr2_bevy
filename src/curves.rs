//! Unweighted rich-curve keys recovered from the player Blueprint exports.
//! Times and tangents are authored data; dilation is applied by the timeline
//! owner, so evaluating a curve never advances its clock.

#[derive(Clone, Copy, Debug)]
pub enum Interpolation {
    Linear,
    Cubic,
}

#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub time: f32,
    pub value: f32,
    pub arrive_tangent: f32,
    pub leave_tangent: f32,
    /// Unreal selects the interpolation mode of the segment's left key.
    pub interpolation: Interpolation,
}

#[derive(Clone, Copy, Debug)]
pub struct Curve {
    pub keys: &'static [Key],
    pub length: f32,
    pub ignore_time_dilation: bool,
}

impl Curve {
    /// Evaluate the authored segment and clamp outside the keyed time range.
    pub fn evaluate(&self, time: f32) -> f32 {
        let Some(first) = self.keys.first() else {
            return 0.0;
        };
        if time <= first.time {
            return first.value;
        }
        for pair in self.keys.windows(2) {
            let [left, right] = pair else { unreachable!() };
            if time <= right.time {
                let duration = right.time - left.time;
                let alpha = (time - left.time) / duration;
                return match left.interpolation {
                    Interpolation::Linear => left.value + alpha * (right.value - left.value),
                    Interpolation::Cubic => {
                        // UE CubicInterp scales its endpoint derivatives by the
                        // segment duration. Keep ordinary f32 operations rather
                        // than replacing the expression with fused mul_add.
                        let squared = alpha * alpha;
                        let cubed = squared * alpha;
                        left.value * (2.0 * cubed - 3.0 * squared + 1.0)
                            + left.leave_tangent * duration * (cubed - 2.0 * squared + alpha)
                            + right.value * (-2.0 * cubed + 3.0 * squared)
                            + right.arrive_tangent * duration * (cubed - squared)
                    }
                };
            }
        }
        self.keys.last().unwrap().value
    }
}

// These decimal representations retain the extracted float values for audit.
#[allow(clippy::excessive_precision)]
pub const FORWARD_SPEED: Curve = Curve {
    length: 1.5,
    ignore_time_dilation: true,
    keys: &[
        Key {
            time: 0.0,
            value: 0.0,
            arrive_tangent: 4252.83203125,
            leave_tangent: 3.974170207977295,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.2937280237674713,
            value: 0.5997332334518433,
            arrive_tangent: 1.1194560527801514,
            leave_tangent: 1.1194560527801514,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.8043191432952881,
            value: 0.900399923324585,
            arrive_tangent: 0.21796390414237976,
            leave_tangent: 0.21796561777591705,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 1.5,
            value: 1.0,
            arrive_tangent: 0.13844440877437592,
            leave_tangent: 1.5846220254898071,
            interpolation: Interpolation::Cubic,
        },
    ],
};

#[allow(clippy::excessive_precision)]
pub const OTHER_SPEED: Curve = Curve {
    length: 1.0,
    ignore_time_dilation: false,
    keys: &[
        Key {
            time: 0.0,
            value: 0.0,
            arrive_tangent: 4.988814830780029,
            leave_tangent: 4.988807678222656,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.15142104029655457,
            value: 0.5627804398536682,
            arrive_tangent: 2.3176674842834473,
            leave_tangent: 2.3176674842834473,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.3806501030921936,
            value: 0.8822203874588013,
            arrive_tangent: 0.5152373313903809,
            leave_tangent: 0.5152373313903809,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 1.0,
            value: 1.0,
            arrive_tangent: 0.0,
            leave_tangent: 0.0,
            interpolation: Interpolation::Linear,
        },
    ],
};

#[allow(clippy::excessive_precision)]
pub const GROUND_DASH: Curve = Curve {
    length: 0.35,
    ignore_time_dilation: false,
    keys: &[
        Key {
            time: 0.0,
            value: 0.0,
            arrive_tangent: 0.0,
            leave_tangent: 4.16039609909,
            interpolation: Interpolation::Cubic,
        },
        Key {
            time: 0.35,
            value: 1.0,
            arrive_tangent: 1.08447968960,
            leave_tangent: 0.0,
            interpolation: Interpolation::Cubic,
        },
    ],
};
