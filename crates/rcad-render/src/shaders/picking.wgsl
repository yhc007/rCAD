// Picking shader - renders entity IDs to a texture for selection

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

// Object ID passed via push constant or instance data
// For simplicity, we'll encode it in the vertex color's alpha channel
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) object_id: u32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    // Extract object ID from color alpha (scaled by 255 then cast to u32)
    out.object_id = u32(in.color.a * 255.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) u32 {
    return in.object_id;
}
