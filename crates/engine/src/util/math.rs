use num_traits::ToPrimitive;

pub const fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (b - a).mul_add(t, a)
}

/// Rounds `value` to the nearest multiple of `step` above `min`, then clamps to the range.
pub fn snap(value: f32, min: f32, max: f32, step: Option<f32>) -> f32 {
    let value = match step {
        Some(step) if step > 0.0 => ((value - min) / step).round().mul_add(step, min),
        _ => value,
    };
    value.clamp(min.min(max), max.max(min))
}

/// Maps a [0, 1] fraction to a byte; out-of-range values clamp and NaN maps to zero.
pub fn unit_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round().to_u8().unwrap_or(0)
}

/// Truncates to `u32` and clamps to `max`; negatives and NaN map to zero.
pub fn clamp_to_u32(value: f32, max: u32) -> u32 {
    value.max(0.0).to_u32().unwrap_or(u32::MAX).min(max)
}

/// Truncates to `i32`, saturating at the integer range; NaN maps to zero.
pub fn saturating_i32(value: f32) -> i32 {
    value.to_i32().unwrap_or_else(|| {
        if value.is_nan() {
            0
        } else if value.is_sign_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{clamp_to_u32, saturating_i32, unit_to_u8};

    #[test]
    fn saturating_i32_handles_extremes_and_nan() {
        assert_eq!(saturating_i32(-12.0), -12);
        assert_eq!(saturating_i32(12.0), 12);
        assert_eq!(saturating_i32(f32::MAX), i32::MAX);
        assert_eq!(saturating_i32(-f32::MAX), i32::MIN);
        assert_eq!(saturating_i32(f32::INFINITY), i32::MAX);
        assert_eq!(saturating_i32(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(saturating_i32(f32::NAN), 0);
    }

    #[test]
    fn clamp_to_u32_handles_extremes_and_nan() {
        assert_eq!(clamp_to_u32(3.7, 10), 3);
        assert_eq!(clamp_to_u32(-3.0, 10), 0);
        assert_eq!(clamp_to_u32(f32::INFINITY, 10), 10);
        assert_eq!(clamp_to_u32(f32::NAN, 10), 0);
    }

    #[test]
    fn unit_to_u8_rounds_and_clamps() {
        assert_eq!(unit_to_u8(0.5), 128);
        assert_eq!(unit_to_u8(-1.0), 0);
        assert_eq!(unit_to_u8(2.0), 255);
        assert_eq!(unit_to_u8(f32::NAN), 0);
    }
}
