struct Globals {
    screen: vec2<f32>,
    _pad: vec2<f32>,
    // Game area (letterboxed) in pixels: x, y, w, h.
    viewport: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(2) @binding(0) var mask_tex: texture_2d<f32>;
@group(2) @binding(1) var mask_samp: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,         // x, y, w, h in pixels (before rotation)
    @location(1) uv: vec4<f32>,           // u0, v0, u1, v1
    @location(2) color: vec4<f32>,        // sRGB, straight alpha
    @location(3) border_color: vec4<f32>, // sRGB, straight alpha
    @location(4) params: vec4<f32>,       // radius, border width, rotation (radians), mask invert
    @location(5) mask: vec4<f32>,         // kind, progress, ramp / block size, unused
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) half_size: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) params: vec4<f32>,
    @location(6) mask: vec4<f32>,
    @location(7) uv_rect: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32, inst: Instance) -> VsOut {
    let corner = vec2<f32>(f32(index & 1u), f32(index >> 1u));
    let local = (corner - vec2<f32>(0.5)) * inst.rect.zw;
    let angle = inst.params.z;
    let c = cos(angle);
    let s = sin(angle);
    let rotated = vec2<f32>(local.x * c - local.y * s, local.x * s + local.y * c);
    let p = inst.rect.xy + inst.rect.zw * 0.5 + rotated;
    var out: VsOut;
    out.position = vec4<f32>(p.x / globals.screen.x * 2.0 - 1.0, 1.0 - p.y / globals.screen.y * 2.0, 0.0, 1.0);
    out.uv = mix(inst.uv.xy, inst.uv.zw, corner);
    out.local = local;
    out.half_size = inst.rect.zw * 0.5;
    out.color = inst.color;
    out.border_color = inst.border_color;
    out.params = inst.params;
    out.mask = inst.mask;
    out.uv_rect = inst.uv;
    return out;
}

fn rounded_box_distance(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// How visible a pixel is for a reveal value in [0, 1] at `progress`.
fn reveal(value: f32, progress: f32, ramp: f32) -> f32 {
    let r = max(ramp, 0.0001);
    return clamp((progress * (1.0 + r) - value) / r, 0.0, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let kind = u32(in.mask.x + 0.5);
    let progress = in.mask.y;
    let screen_pos = (in.position.xy - globals.viewport.xy) / max(globals.viewport.zw, vec2<f32>(1.0));

    var uv = in.uv;
    if kind == 6u {
        // Pixellate: blocks shrink from `mask.z` pixels to 1 as progress goes to 1.
        let t = select(progress, 1.0 - progress, in.params.w > 0.5);
        let block = max(mix(in.mask.z, 1.0, t), 1.0);
        let dims = vec2<f32>(textureDimensions(tex));
        let texel = (uv - in.uv_rect.xy) * dims;
        uv = in.uv_rect.xy + (floor(texel / block) + 0.5) * block / dims;
    }

    let radius = min(in.params.x, min(in.half_size.x, in.half_size.y));
    let d = rounded_box_distance(in.local, in.half_size, radius);
    let coverage = clamp(0.5 - d, 0.0, 1.0);
    var color = textureSampleLevel(tex, samp, uv, 0.0) * in.color;
    let border = in.params.y;
    if border > 0.0 {
        let inside = clamp(0.5 - (d + border), 0.0, 1.0);
        color = mix(in.border_color, color, inside);
    }

    var visibility = 1.0;
    if kind == 1u {
        let m = textureSampleLevel(mask_tex, mask_samp, screen_pos, 0.0).r;
        visibility = reveal(m, progress, in.mask.z);
    } else if kind >= 2u && kind <= 5u {
        var v = screen_pos.x;
        if kind == 2u { v = 1.0 - screen_pos.x; }
        if kind == 4u { v = 1.0 - screen_pos.y; }
        if kind == 5u { v = screen_pos.y; }
        visibility = reveal(v, progress, in.mask.z);
    }
    if in.params.w > 0.5 && kind != 6u {
        visibility = 1.0 - visibility;
    }

    let alpha = color.a * coverage * visibility;
    return vec4<f32>(color.rgb * alpha, alpha);
}
