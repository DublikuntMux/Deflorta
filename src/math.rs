pub const fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (b - a).mul_add(t, a)
}
