//! Main renderer implementation
//!
//! Handles WebGPU device initialization, render loop, and frame submission.

use crate::{
    CameraUniforms, LightUniforms, ModelUniforms, RenderConfig, RenderError, Result, Vertex,
    camera::Camera,
    materials::Material,
    mesh::GpuMesh,
    pipeline::Pipelines,
    selection::SelectionManager,
};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Main render engine
pub struct RenderEngine {
    /// GPU device
    pub device: Arc<wgpu::Device>,

    /// Command queue
    pub queue: Arc<wgpu::Queue>,

    /// Render pipelines
    pub pipelines: Pipelines,

    /// Camera
    pub camera: Camera,

    /// Render configuration
    pub config: RenderConfig,

    /// Light uniforms
    pub lights: LightUniforms,

    /// Camera uniform buffer
    camera_buffer: wgpu::Buffer,

    /// Light uniform buffer
    light_buffer: wgpu::Buffer,

    /// Camera bind group
    camera_bind_group: wgpu::BindGroup,

    /// Depth texture
    depth_texture: wgpu::Texture,

    /// Depth texture view
    depth_view: wgpu::TextureView,

    /// MSAA texture (if enabled)
    msaa_texture: Option<wgpu::Texture>,

    /// MSAA view
    msaa_view: Option<wgpu::TextureView>,

    /// Selection manager
    pub selection: SelectionManager,

    /// Current surface format
    surface_format: wgpu::TextureFormat,

    /// Current viewport size
    viewport_size: (u32, u32),
}

impl RenderEngine {
    /// Create a new render engine
    pub async fn new(
        surface: &wgpu::Surface<'_>,
        adapter: &wgpu::Adapter,
        width: u32,
        height: u32,
        config: RenderConfig,
    ) -> Result<Self> {
        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("rCAD Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|_| RenderError::DeviceRequestFailed)?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Get surface format
        let surface_caps = surface.get_capabilities(adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Configure surface
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create pipelines
        let pipelines = Pipelines::new(&device, surface_format, config.msaa_samples)?;

        // Create camera uniform buffer
        let camera = Camera::new(width as f32 / height as f32);
        let camera_uniforms = camera.uniforms();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create light uniform buffer
        let lights = LightUniforms::default();
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Buffer"),
            contents: bytemuck::cast_slice(&[lights]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create camera bind group
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &pipelines.camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // Create depth texture
        let (depth_texture, depth_view) =
            Self::create_depth_texture(&device, width, height, config.msaa_samples);

        // Create MSAA texture if needed
        let (msaa_texture, msaa_view) = if config.msaa_samples > 1 {
            let (tex, view) =
                Self::create_msaa_texture(&device, width, height, surface_format, config.msaa_samples);
            (Some(tex), Some(view))
        } else {
            (None, None)
        };

        // Create selection manager
        let selection = SelectionManager::new(&device, width, height);

        Ok(Self {
            device,
            queue,
            pipelines,
            camera,
            config,
            lights,
            camera_buffer,
            light_buffer,
            camera_bind_group,
            depth_texture,
            depth_view,
            msaa_texture,
            msaa_view,
            selection,
            surface_format,
            viewport_size: (width, height),
        })
    }

    /// Resize the viewport
    pub fn resize(&mut self, surface: &wgpu::Surface<'_>, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.viewport_size = (width, height);

        // Reconfigure surface
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &surface_config);

        // Recreate depth texture
        let (depth_texture, depth_view) =
            Self::create_depth_texture(&self.device, width, height, self.config.msaa_samples);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;

        // Recreate MSAA texture if needed
        if self.config.msaa_samples > 1 {
            let (tex, view) = Self::create_msaa_texture(
                &self.device,
                width,
                height,
                self.surface_format,
                self.config.msaa_samples,
            );
            self.msaa_texture = Some(tex);
            self.msaa_view = Some(view);
        }

        // Update camera aspect ratio
        self.camera.set_aspect(width as f32 / height as f32);

        // Resize selection buffer
        self.selection.resize(&self.device, width, height);
    }

    /// Update camera uniforms
    pub fn update_camera(&mut self) {
        let uniforms = self.camera.uniforms();
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Update light uniforms
    pub fn update_lights(&mut self) {
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(&[self.lights]));
    }

    /// Render a frame
    pub fn render(
        &mut self,
        surface: &wgpu::Surface<'_>,
        meshes: &[(GpuMesh, glam::Mat4, Material)],
    ) -> Result<()> {
        // Get current texture
        let output = surface
            .get_current_texture()
            .map_err(|e| RenderError::SurfaceError(format!("{:?}", e)))?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Update camera
        self.update_camera();

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Begin render pass
        {
            let color_attachment = if let Some(ref msaa_view) = self.msaa_view {
                wgpu::RenderPassColorAttachment {
                    view: msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.clear_color[0] as f64,
                            g: self.config.clear_color[1] as f64,
                            b: self.config.clear_color[2] as f64,
                            a: self.config.clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                }
            } else {
                wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.config.clear_color[0] as f64,
                            g: self.config.clear_color[1] as f64,
                            b: self.config.clear_color[2] as f64,
                            a: self.config.clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                }
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(color_attachment)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render grid if enabled
            if self.config.show_grid {
                self.render_grid(&mut render_pass);
            }

            // Render axes if enabled
            if self.config.show_axes {
                self.render_axes(&mut render_pass);
            }

            // Render meshes
            render_pass.set_pipeline(&self.pipelines.pbr);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            for (mesh, transform, material) in meshes {
                self.render_mesh(&mut render_pass, mesh, transform, material);
            }

            // Render wireframe overlay if enabled
            if self.config.show_wireframe {
                render_pass.set_pipeline(&self.pipelines.wireframe);
                for (mesh, transform, _) in meshes {
                    self.render_mesh_wireframe(&mut render_pass, mesh, transform);
                }
            }
        }

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn render_mesh(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        mesh: &GpuMesh,
        transform: &glam::Mat4,
        material: &Material,
    ) {
        // Set model uniform
        // In a full implementation, we'd have a model uniform buffer per object
        // For simplicity, we'll skip the model transform in the shader for now

        render_pass.set_bind_group(1, &material.bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    fn render_mesh_wireframe(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        mesh: &GpuMesh,
        transform: &glam::Mat4,
    ) {
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }

    fn render_grid(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_pipeline(&self.pipelines.grid);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }

    fn render_axes(&self, _render_pass: &mut wgpu::RenderPass<'_>) {
        // TODO: Implement axes rendering
        // Would draw X (red), Y (green), Z (blue) axis lines
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    fn create_msaa_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Get viewport size
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
    }
}
