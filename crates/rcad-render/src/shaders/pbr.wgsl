// PBR (Physically Based Rendering) Shader
// Implements a simplified PBR lighting model

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

struct LightUniforms {
    direction: vec4<f32>,
    color: vec4<f32>,
    ambient: vec4<f32>,
};

struct MaterialUniforms {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    ao: f32,
    emissive: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> light: LightUniforms;
@group(1) @binding(0) var<uniform> material: MaterialUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.world_position = in.position;
    out.world_normal = normalize(in.normal);
    out.uv = in.uv;
    out.color = in.color;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);

    return out;
}

const PI: f32 = 3.14159265359;

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// GGX/Trowbridge-Reitz normal distribution
fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;

    let num = a2;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return num / denom;
}

// Schlick-GGX geometry function
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;

    let num = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;

    return num / denom;
}

// Smith's geometry function
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);

    return ggx1 * ggx2;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = material.base_color.rgb * in.color.rgb;
    let metallic = material.metallic;
    let roughness = max(material.roughness, 0.04);
    let ao = material.ao;

    let n = normalize(in.world_normal);
    let v = normalize(camera.camera_pos.xyz - in.world_position);
    let l = normalize(-light.direction.xyz);
    let h = normalize(v + l);

    // Calculate reflectance at normal incidence
    var f0 = vec3<f32>(0.04);
    f0 = mix(f0, albedo, metallic);

    // Cook-Torrance BRDF
    let ndf = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let numerator = ndf * g * f;
    let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
    let specular = numerator / denominator;

    let k_s = f;
    var k_d = vec3<f32>(1.0) - k_s;
    k_d *= 1.0 - metallic;

    let n_dot_l = max(dot(n, l), 0.0);

    // Combine diffuse and specular
    var lo = (k_d * albedo / PI + specular) * light.color.rgb * n_dot_l;

    // Ambient lighting
    let ambient = light.ambient.rgb * albedo * ao;

    // Emissive
    let emissive = albedo * material.emissive;

    var color = ambient + lo + emissive;

    // HDR tone mapping (Reinhard)
    color = color / (color + vec3<f32>(1.0));

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, material.base_color.a);
}
