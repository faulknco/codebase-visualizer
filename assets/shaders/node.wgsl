struct Camera {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) depth: f32,
    @location(4) color: vec4<f32>,
    @location(5) glow: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) glow: f32,
};

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = vec4<f32>(
        inst.center + vert.position * inst.radius,
        inst.depth * 0.5,
        1.0
    );
    out.clip_position = camera.view_proj * world_pos;
    out.uv = vert.position;
    out.color = inst.color;
    out.glow = inst.glow;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(in.uv);
    if dist > 1.0 {
        discard;
    }
    let core = smoothstep(0.8, 0.0, dist);
    let edge = smoothstep(1.0, 0.7, dist);
    let lit = in.color.rgb * (0.4 + 0.6 * core);
    let glow_strength = in.glow * smoothstep(1.0, 0.3, dist) * 0.5;
    let glow_color = in.color.rgb * glow_strength;
    let final_color = lit + glow_color;
    let alpha = edge;
    return vec4<f32>(final_color, alpha);
}
