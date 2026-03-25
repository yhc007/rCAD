//! # rcad-render
//!
//! WebGPU rendering engine for rCAD.
//! Provides high-performance 3D rendering with PBR materials, camera controls,
//! mesh rendering, and selection support.

pub mod camera;
pub mod materials;
pub mod mesh;
pub mod pipeline;
pub mod renderer;
pub mod selection;

pub use camera::*;
pub use materials::*;
pub use mesh::*;
pub use pipeline::*;
pub use renderer::*;
pub use selection::*;

use thiserror::Error;

/// Result type for render operations
pub type Result<T> = std::result::Result<T, RenderError>;

/// Render errors
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("WebGPU initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Pipeline creation failed: {0}")]
    PipelineCreationFailed(String),

    #[error("Buffer creation failed: {0}")]
    BufferCreationFailed(String),

    #[error("Texture creation failed: {0}")]
    TextureCreationFailed(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilationFailed(String),

    #[error("Surface error: {0}")]
    SurfaceError(String),

    #[error("No GPU adapter found")]
    NoAdapterFound,

    #[error("Device request failed")]
    DeviceRequestFailed,
}

/// Render configuration
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Clear color (RGBA)
    pub clear_color: [f32; 4],

    /// Enable antialiasing
    pub msaa_samples: u32,

    /// Enable wireframe overlay
    pub show_wireframe: bool,

    /// Enable grid
    pub show_grid: bool,

    /// Enable coordinate axes
    pub show_axes: bool,

    /// Grid size
    pub grid_size: f32,

    /// Grid divisions
    pub grid_divisions: u32,

    /// Background mode
    pub background: BackgroundMode,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            clear_color: [0.1, 0.1, 0.12, 1.0],
            msaa_samples: 4,
            show_wireframe: false,
            show_grid: true,
            show_axes: true,
            grid_size: 100.0,
            grid_divisions: 10,
            background: BackgroundMode::Gradient,
        }
    }
}

/// Background rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundMode {
    /// Solid color
    Solid,
    /// Gradient from top to bottom
    Gradient,
    /// Environment map (HDR)
    Environment,
}

/// Vertex format for mesh rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position
        1 => Float32x3,  // normal
        2 => Float32x2,  // uv
        3 => Float32x4,  // color
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Camera uniforms sent to GPU
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniforms {
    /// View-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// View matrix
    pub view: [[f32; 4]; 4],
    /// Projection matrix
    pub proj: [[f32; 4]; 4],
    /// Camera position in world space
    pub camera_pos: [f32; 4],
}

/// Model uniforms sent to GPU
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniforms {
    /// Model matrix
    pub model: [[f32; 4]; 4],
    /// Normal matrix (transpose of inverse of model)
    pub normal_matrix: [[f32; 4]; 4],
}

/// Light uniforms
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    /// Directional light direction
    pub direction: [f32; 4],
    /// Light color and intensity
    pub color: [f32; 4],
    /// Ambient color
    pub ambient: [f32; 4],
}

impl Default for LightUniforms {
    fn default() -> Self {
        Self {
            direction: [-0.5, -1.0, -0.5, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            ambient: [0.1, 0.1, 0.15, 1.0],
        }
    }
}
