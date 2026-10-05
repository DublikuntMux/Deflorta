// Instanced quads: solid or textured rectangles with rounded corners and borders.

struct Globals {
    screen: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,         // x, y, w, h in pixels
    @location(1) uv: vec4<f32>,           // u0, v0, u1, v1
    @location(2) color: vec4<f32>,        // sRGB, straight alpha
    @location(3) border_color: vec4<f32>, // sRGB, straight alpha
    @location(4) params: vec4<f32>,       // radius, border width
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) half_size: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) params: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32, inst: Instance) -> VsOut {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let p = inst.rect.xy + corner * inst.rect.zw;
    var out: VsOut;
    out.position = vec4<f32>(p.x / globals.screen.x * 2.0 - 1.0, 1.0 - p.y / globals.screen.y * 2.0, 0.0, 1.0);
    out.uv = mix(inst.uv.xy, inst.uv.zw, corner);
    out.local = (corner - vec2<f32>(0.5)) * inst.rect.zw;
    out.half_size = inst.rect.zw * 0.5;
    out.color = inst.color;
    out.border_color = inst.border_color;
    out.params = inst.params;
    return out;
}

fn rounded_box_distance(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let radius = min(in.params.x, min(in.half_size.x, in.half_size.y));
    let d = rounded_box_distance(in.local, in.half_size, radius);
    let coverage = clamp(0.5 - d, 0.0, 1.0);
    var color = textureSample(tex, samp, in.uv) * in.color;
    let border = in.params.y;
    if border > 0.0 {
        let inside = clamp(0.5 - (d + border), 0.0, 1.0);
        color = mix(in.border_color, color, inside);
    }
    let alpha = color.a * coverage;
    return vec4<f32>(color.rgb * alpha, alpha);
}
