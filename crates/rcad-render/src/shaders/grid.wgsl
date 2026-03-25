// Infinite grid shader
// Based on the technique from "The Best Darn Grid Shader (Yet)"

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) near_point: vec3<f32>,
    @location(1) far_point: vec3<f32>,
};

// Grid plane vertices (fullscreen quad)
var<private> grid_plane: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(-1.0, -1.0, 0.0),
    vec3<f32>( 1.0, -1.0, 0.0),
    vec3<f32>( 1.0,  1.0, 0.0),
    vec3<f32>( 1.0,  1.0, 0.0),
    vec3<f32>(-1.0,  1.0, 0.0),
    vec3<f32>(-1.0, -1.0, 0.0),
);

fn unproject_point(x: f32, y: f32, z: f32) -> vec3<f32> {
    let inv_view_proj = inverse(camera.view_proj);
    let unprojected = inv_view_proj * vec4<f32>(x, y, z, 1.0);
    return unprojected.xyz / unprojected.w;
}

// 4x4 matrix inverse (needed for unproject)
fn inverse(m: mat4x4<f32>) -> mat4x4<f32> {
    let a00 = m[0][0]; let a01 = m[0][1]; let a02 = m[0][2]; let a03 = m[0][3];
    let a10 = m[1][0]; let a11 = m[1][1]; let a12 = m[1][2]; let a13 = m[1][3];
    let a20 = m[2][0]; let a21 = m[2][1]; let a22 = m[2][2]; let a23 = m[2][3];
    let a30 = m[3][0]; let a31 = m[3][1]; let a32 = m[3][2]; let a33 = m[3][3];

    let b00 = a00 * a11 - a01 * a10;
    let b01 = a00 * a12 - a02 * a10;
    let b02 = a00 * a13 - a03 * a10;
    let b03 = a01 * a12 - a02 * a11;
    let b04 = a01 * a13 - a03 * a11;
    let b05 = a02 * a13 - a03 * a12;
    let b06 = a20 * a31 - a21 * a30;
    let b07 = a20 * a32 - a22 * a30;
    let b08 = a20 * a33 - a23 * a30;
    let b09 = a21 * a32 - a22 * a31;
    let b10 = a21 * a33 - a23 * a31;
    let b11 = a22 * a33 - a23 * a32;

    let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
    let inv_det = 1.0 / det;

    return mat4x4<f32>(
        vec4<f32>(
            (a11 * b11 - a12 * b10 + a13 * b09) * inv_det,
            (a02 * b10 - a01 * b11 - a03 * b09) * inv_det,
            (a31 * b05 - a32 * b04 + a33 * b03) * inv_det,
            (a22 * b04 - a21 * b05 - a23 * b03) * inv_det,
        ),
        vec4<f32>(
            (a12 * b08 - a10 * b11 - a13 * b07) * inv_det,
            (a00 * b11 - a02 * b08 + a03 * b07) * inv_det,
            (a32 * b02 - a30 * b05 - a33 * b01) * inv_det,
            (a20 * b05 - a22 * b02 + a23 * b01) * inv_det,
        ),
        vec4<f32>(
            (a10 * b10 - a11 * b08 + a13 * b06) * inv_det,
            (a01 * b08 - a00 * b10 - a03 * b06) * inv_det,
            (a30 * b04 - a31 * b02 + a33 * b00) * inv_det,
            (a21 * b02 - a20 * b04 - a23 * b00) * inv_det,
        ),
        vec4<f32>(
            (a11 * b07 - a10 * b09 - a12 * b06) * inv_det,
            (a00 * b09 - a01 * b07 + a02 * b06) * inv_det,
            (a31 * b01 - a30 * b03 - a32 * b00) * inv_det,
            (a20 * b03 - a21 * b01 + a22 * b00) * inv_det,
        )
    );
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    let p = grid_plane[vertex_index];
    out.clip_position = vec4<f32>(p, 1.0);

    // Unproject to get world space points on near and far planes
    out.near_point = unproject_point(p.x, p.y, 0.0);
    out.far_point = unproject_point(p.x, p.y, 1.0);

    return out;
}

fn grid(frag_pos: vec3<f32>, scale: f32) -> vec4<f32> {
    let coord = frag_pos.xz * scale;
    let derivative = fwidth(coord);
    let grid_lines = abs(fract(coord - 0.5) - 0.5) / derivative;
    let line = min(grid_lines.x, grid_lines.y);
    let minimumz = min(derivative.y, 1.0);
    let minimumx = min(derivative.x, 1.0);
    var color = vec4<f32>(0.3, 0.3, 0.3, 1.0 - min(line, 1.0));

    // X axis (red)
    if frag_pos.z > -0.1 * minimumz && frag_pos.z < 0.1 * minimumz {
        color = vec4<f32>(1.0, 0.2, 0.2, 1.0);
    }

    // Z axis (blue)
    if frag_pos.x > -0.1 * minimumx && frag_pos.x < 0.1 * minimumx {
        color = vec4<f32>(0.2, 0.2, 1.0, 1.0);
    }

    return color;
}

fn compute_depth(pos: vec3<f32>) -> f32 {
    let clip_space_pos = camera.view_proj * vec4<f32>(pos, 1.0);
    return clip_space_pos.z / clip_space_pos.w;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;

    // Calculate the intersection with Y=0 plane
    let t = -in.near_point.y / (in.far_point.y - in.near_point.y);

    // Discard if plane is behind camera or at invalid intersection
    if t < 0.0 {
        discard;
    }

    let frag_pos = in.near_point + t * (in.far_point - in.near_point);

    // Compute proper depth
    out.depth = compute_depth(frag_pos);

    // Fade out with distance
    let dist = length(frag_pos.xz - camera.camera_pos.xz);
    let fade = 1.0 - smoothstep(50.0, 500.0, dist);

    // Multiple grid scales
    let grid1 = grid(frag_pos, 1.0);   // 1 unit grid
    let grid2 = grid(frag_pos, 0.1);   // 10 unit grid

    out.color = grid1;
    out.color.a *= fade;

    // Blend larger grid
    out.color = mix(out.color, grid2, 0.5) * vec4<f32>(1.0, 1.0, 1.0, fade);

    if out.color.a < 0.01 {
        discard;
    }

    return out;
}
