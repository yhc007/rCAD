//! PBR materials
//!
//! Provides physically-based rendering materials.

use wgpu::util::DeviceExt;

/// PBR material parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniforms {
    /// Base color (RGBA)
    pub base_color: [f32; 4],

    /// Metallic factor
    pub metallic: f32,

    /// Roughness factor
    pub roughness: f32,

    /// Ambient occlusion factor
    pub ao: f32,

    /// Emissive strength
    pub emissive: f32,
}

impl Default for MaterialUniforms {
    fn default() -> Self {
        Self {
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            emissive: 0.0,
        }
    }
}

/// A PBR material
pub struct Material {
    /// Material uniform buffer
    pub uniform_buffer: wgpu::Buffer,

    /// Bind group for this material
    pub bind_group: wgpu::BindGroup,

    /// Material parameters (for editing)
    pub params: MaterialUniforms,
}

impl Material {
    /// Create a new material
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        params: MaterialUniforms,
    ) -> Self {
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Uniform Buffer"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Material Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            uniform_buffer,
            bind_group,
            params,
        }
    }

    /// Create a default material
    pub fn default_material(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        Self::new(device, layout, MaterialUniforms::default())
    }

    /// Update the material parameters
    pub fn update(&mut self, queue: &wgpu::Queue, params: MaterialUniforms) {
        self.params = params;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[params]));
    }

    /// Set base color
    pub fn set_base_color(&mut self, queue: &wgpu::Queue, color: [f32; 4]) {
        self.params.base_color = color;
        self.update(queue, self.params);
    }

    /// Set metallic value
    pub fn set_metallic(&mut self, queue: &wgpu::Queue, metallic: f32) {
        self.params.metallic = metallic.clamp(0.0, 1.0);
        self.update(queue, self.params);
    }

    /// Set roughness value
    pub fn set_roughness(&mut self, queue: &wgpu::Queue, roughness: f32) {
        self.params.roughness = roughness.clamp(0.0, 1.0);
        self.update(queue, self.params);
    }
}

/// Predefined materials for common CAD use cases
pub struct MaterialLibrary;

impl MaterialLibrary {
    /// Plastic-like material
    pub fn plastic(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, color: [f32; 3]) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [color[0], color[1], color[2], 1.0],
                metallic: 0.0,
                roughness: 0.4,
                ao: 1.0,
                emissive: 0.0,
            },
        )
    }

    /// Metal material
    pub fn metal(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, color: [f32; 3]) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [color[0], color[1], color[2], 1.0],
                metallic: 1.0,
                roughness: 0.3,
                ao: 1.0,
                emissive: 0.0,
            },
        )
    }

    /// Brushed metal
    pub fn brushed_metal(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, color: [f32; 3]) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [color[0], color[1], color[2], 1.0],
                metallic: 1.0,
                roughness: 0.5,
                ao: 1.0,
                emissive: 0.0,
            },
        )
    }

    /// Rubber material
    pub fn rubber(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, color: [f32; 3]) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [color[0], color[1], color[2], 1.0],
                metallic: 0.0,
                roughness: 0.9,
                ao: 1.0,
                emissive: 0.0,
            },
        )
    }

    /// Glass/transparent material
    pub fn glass(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [1.0, 1.0, 1.0, 0.3],
                metallic: 0.0,
                roughness: 0.1,
                ao: 1.0,
                emissive: 0.0,
            },
        )
    }

    /// Selection highlight material
    pub fn selection(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [0.2, 0.6, 1.0, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                ao: 1.0,
                emissive: 0.3,
            },
        )
    }

    /// Hover highlight material
    pub fn hover(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Material::new(
            device,
            layout,
            MaterialUniforms {
                base_color: [1.0, 0.8, 0.2, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                ao: 1.0,
                emissive: 0.2,
            },
        )
    }

    /// Aluminum
    pub fn aluminum(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Self::metal(device, layout, [0.91, 0.92, 0.92])
    }

    /// Steel
    pub fn steel(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Self::metal(device, layout, [0.56, 0.57, 0.58])
    }

    /// Gold
    pub fn gold(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Self::metal(device, layout, [1.0, 0.76, 0.33])
    }

    /// Copper
    pub fn copper(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Material {
        Self::metal(device, layout, [0.95, 0.64, 0.54])
    }
}
