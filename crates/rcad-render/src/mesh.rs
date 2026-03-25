//! GPU mesh management
//!
//! Handles uploading geometry data to GPU buffers.

use crate::{Result, RenderError, Vertex};
use rcad_geometry::Mesh;
use wgpu::util::DeviceExt;

/// A mesh uploaded to the GPU
pub struct GpuMesh {
    /// Vertex buffer
    pub vertex_buffer: wgpu::Buffer,

    /// Index buffer
    pub index_buffer: wgpu::Buffer,

    /// Number of indices
    pub index_count: u32,

    /// Number of vertices
    pub vertex_count: u32,

    /// Bounding box minimum
    pub bounds_min: glam::Vec3,

    /// Bounding box maximum
    pub bounds_max: glam::Vec3,
}

impl GpuMesh {
    /// Create a GPU mesh from geometry mesh data
    pub fn from_mesh(device: &wgpu::Device, mesh: &Mesh) -> Result<Self> {
        if mesh.is_empty() {
            return Err(RenderError::BufferCreationFailed("Empty mesh".to_string()));
        }

        // Convert to vertex format
        let mut vertices = Vec::with_capacity(mesh.vertex_count());
        let mut bounds_min = glam::Vec3::splat(f32::MAX);
        let mut bounds_max = glam::Vec3::splat(f32::MIN);

        for i in 0..mesh.vertex_count() {
            let idx = i * 3;
            let pos = [mesh.positions[idx], mesh.positions[idx + 1], mesh.positions[idx + 2]];
            let normal = [mesh.normals[idx], mesh.normals[idx + 1], mesh.normals[idx + 2]];

            // Update bounds
            bounds_min = bounds_min.min(glam::Vec3::from(pos));
            bounds_max = bounds_max.max(glam::Vec3::from(pos));

            // Get UV if available
            let uv = if let Some(ref uvs) = mesh.uvs {
                let uv_idx = i * 2;
                [uvs[uv_idx], uvs[uv_idx + 1]]
            } else {
                [0.0, 0.0]
            };

            // Get color if available
            let color = if let Some(ref colors) = mesh.colors {
                let col_idx = i * 4;
                [colors[col_idx], colors[col_idx + 1], colors[col_idx + 2], colors[col_idx + 3]]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            vertices.push(Vertex {
                position: pos,
                normal,
                uv,
                color,
            });
        }

        // Create vertex buffer
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create index buffer
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_min,
            bounds_max,
        })
    }

    /// Create from raw vertex and index data
    pub fn from_data(
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Result<Self> {
        if vertices.is_empty() || indices.is_empty() {
            return Err(RenderError::BufferCreationFailed("Empty data".to_string()));
        }

        let mut bounds_min = glam::Vec3::splat(f32::MAX);
        let mut bounds_max = glam::Vec3::splat(f32::MIN);

        for v in vertices {
            bounds_min = bounds_min.min(glam::Vec3::from(v.position));
            bounds_max = bounds_max.max(glam::Vec3::from(v.position));
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            vertex_count: vertices.len() as u32,
            bounds_min,
            bounds_max,
        })
    }

    /// Create a simple cube mesh for testing
    pub fn create_cube(device: &wgpu::Device, size: f32) -> Result<Self> {
        let s = size / 2.0;

        #[rustfmt::skip]
        let vertices = [
            // Front face
            Vertex { position: [-s, -s,  s], normal: [ 0.0,  0.0,  1.0], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s, -s,  s], normal: [ 0.0,  0.0,  1.0], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s,  s,  s], normal: [ 0.0,  0.0,  1.0], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s,  s,  s], normal: [ 0.0,  0.0,  1.0], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            // Back face
            Vertex { position: [-s, -s, -s], normal: [ 0.0,  0.0, -1.0], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s,  s, -s], normal: [ 0.0,  0.0, -1.0], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s,  s, -s], normal: [ 0.0,  0.0, -1.0], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s, -s, -s], normal: [ 0.0,  0.0, -1.0], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            // Top face
            Vertex { position: [-s,  s, -s], normal: [ 0.0,  1.0,  0.0], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s,  s,  s], normal: [ 0.0,  1.0,  0.0], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s,  s,  s], normal: [ 0.0,  1.0,  0.0], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s,  s, -s], normal: [ 0.0,  1.0,  0.0], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            // Bottom face
            Vertex { position: [-s, -s, -s], normal: [ 0.0, -1.0,  0.0], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s, -s, -s], normal: [ 0.0, -1.0,  0.0], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s, -s,  s], normal: [ 0.0, -1.0,  0.0], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s, -s,  s], normal: [ 0.0, -1.0,  0.0], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            // Right face
            Vertex { position: [ s, -s, -s], normal: [ 1.0,  0.0,  0.0], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s,  s, -s], normal: [ 1.0,  0.0,  0.0], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s,  s,  s], normal: [ 1.0,  0.0,  0.0], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [ s, -s,  s], normal: [ 1.0,  0.0,  0.0], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            // Left face
            Vertex { position: [-s, -s, -s], normal: [-1.0,  0.0,  0.0], uv: [0.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s, -s,  s], normal: [-1.0,  0.0,  0.0], uv: [1.0, 0.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s,  s,  s], normal: [-1.0,  0.0,  0.0], uv: [1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
            Vertex { position: [-s,  s, -s], normal: [-1.0,  0.0,  0.0], uv: [0.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
        ];

        #[rustfmt::skip]
        let indices: [u32; 36] = [
            0, 1, 2, 2, 3, 0,       // Front
            4, 5, 6, 6, 7, 4,       // Back
            8, 9, 10, 10, 11, 8,   // Top
            12, 13, 14, 14, 15, 12, // Bottom
            16, 17, 18, 18, 19, 16, // Right
            20, 21, 22, 22, 23, 20, // Left
        ];

        Self::from_data(device, &vertices, &indices)
    }

    /// Get bounding box center
    pub fn center(&self) -> glam::Vec3 {
        (self.bounds_min + self.bounds_max) * 0.5
    }

    /// Get bounding box size
    pub fn size(&self) -> glam::Vec3 {
        self.bounds_max - self.bounds_min
    }
}
