struct Conversion {
    red: vec4<f32>,
    green: vec4<f32>,
    blue: vec4<f32>,
    // Horizontal/vertical chroma shifts, alpha presence, unused.
    sampling: vec4<u32>,
};
@group(0) @binding(0) var y_plane: texture_2d<f32>;
@group(0) @binding(1) var u_plane: texture_2d<f32>;
@group(0) @binding(2) var v_plane: texture_2d<f32>;
@group(0) @binding(3) var alpha_plane: texture_2d<f32>;
@group(0) @binding(4) var<uniform> conversion: Conversion;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<u32>(position.xy);
    let chroma = vec2<i32>(pixel >> conversion.sampling.xy);
    let samples = vec4<f32>(
        textureLoad(y_plane, vec2<i32>(pixel), 0).r,
        textureLoad(u_plane, chroma, 0).r,
        textureLoad(v_plane, chroma, 0).r, 1.0);
    var alpha = 1.0;
    if conversion.sampling.z != 0u {
        // Alpha is full-range luma, regardless of the color signal's range.
        alpha = textureLoad(alpha_plane, vec2<i32>(pixel), 0).r;
    }
    return vec4<f32>(clamp(vec3<f32>(dot(samples, conversion.red),
        dot(samples, conversion.green), dot(samples, conversion.blue)),
        vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
}
