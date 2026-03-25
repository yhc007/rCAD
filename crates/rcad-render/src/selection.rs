//! Selection and picking system
//!
//! Provides GPU-based object selection using ID rendering.

use std::collections::HashSet;

/// Selection manager
pub struct SelectionManager {
    /// Currently selected entity IDs
    selected: HashSet<u32>,

    /// Currently hovered entity ID
    hovered: Option<u32>,

    /// Picking texture (stores entity IDs)
    picking_texture: wgpu::Texture,

    /// Picking texture view
    picking_view: wgpu::TextureView,

    /// Depth texture for picking
    picking_depth: wgpu::Texture,

    /// Picking depth view
    picking_depth_view: wgpu::TextureView,

    /// Staging buffer for reading pick results
    staging_buffer: wgpu::Buffer,

    /// Texture dimensions
    width: u32,
    height: u32,
}

impl SelectionManager {
    /// Create a new selection manager
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (picking_texture, picking_view) = Self::create_picking_texture(device, width, height);
        let (picking_depth, picking_depth_view) = Self::create_picking_depth(device, width, height);
        let staging_buffer = Self::create_staging_buffer(device);

        Self {
            selected: HashSet::new(),
            hovered: None,
            picking_texture,
            picking_view,
            picking_depth,
            picking_depth_view,
            staging_buffer,
            width,
            height,
        }
    }

    /// Resize the picking buffers
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let (picking_texture, picking_view) = Self::create_picking_texture(device, width, height);
        let (picking_depth, picking_depth_view) = Self::create_picking_depth(device, width, height);
        self.picking_texture = picking_texture;
        self.picking_view = picking_view;
        self.picking_depth = picking_depth;
        self.picking_depth_view = picking_depth_view;
    }

    fn create_picking_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Picking Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_picking_depth(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Picking Depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_staging_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Picking Staging Buffer"),
            size: 4, // Single u32
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })
    }

    /// Get the picking texture view
    pub fn picking_view(&self) -> &wgpu::TextureView {
        &self.picking_view
    }

    /// Get the picking depth view
    pub fn picking_depth_view(&self) -> &wgpu::TextureView {
        &self.picking_depth_view
    }

    /// Select an entity
    pub fn select(&mut self, id: u32) {
        self.selected.insert(id);
    }

    /// Deselect an entity
    pub fn deselect(&mut self, id: u32) {
        self.selected.remove(&id);
    }

    /// Toggle selection of an entity
    pub fn toggle_selection(&mut self, id: u32) {
        if self.selected.contains(&id) {
            self.selected.remove(&id);
        } else {
            self.selected.insert(id);
        }
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    /// Check if an entity is selected
    pub fn is_selected(&self, id: u32) -> bool {
        self.selected.contains(&id)
    }

    /// Get all selected entities
    pub fn selected_entities(&self) -> &HashSet<u32> {
        &self.selected
    }

    /// Get the number of selected entities
    pub fn selection_count(&self) -> usize {
        self.selected.len()
    }

    /// Set hovered entity
    pub fn set_hovered(&mut self, id: Option<u32>) {
        self.hovered = id;
    }

    /// Get hovered entity
    pub fn hovered(&self) -> Option<u32> {
        self.hovered
    }

    /// Read the entity ID at a screen position
    /// This queues a GPU read operation
    pub fn pick_at(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        x: u32,
        y: u32,
    ) {
        if x >= self.width || y >= self.height {
            return;
        }

        // Copy single pixel from picking texture to staging buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.picking_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Read the pick result from the staging buffer
    /// Call this after the pick_at command has been submitted
    pub async fn read_pick_result(&self, device: &wgpu::Device) -> Option<u32> {
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        device.poll(wgpu::Maintain::Wait);

        if rx.await.ok()?.is_ok() {
            let data = buffer_slice.get_mapped_range();
            let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            drop(data);
            self.staging_buffer.unmap();

            if id == 0 {
                None
            } else {
                Some(id)
            }
        } else {
            None
        }
    }

    /// Synchronously read pick result (blocks)
    pub fn read_pick_result_blocking(&self, device: &wgpu::Device) -> Option<u32> {
        let buffer_slice = self.staging_buffer.slice(..);

        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        device.poll(wgpu::Maintain::Wait);

        if rx.recv().ok()?.is_ok() {
            let data = buffer_slice.get_mapped_range();
            let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            drop(data);
            self.staging_buffer.unmap();

            if id == 0 {
                None
            } else {
                Some(id)
            }
        } else {
            None
        }
    }
}

/// Selection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Select single entity (clear previous selection)
    Single,

    /// Add to selection (Ctrl+click)
    Add,

    /// Toggle selection (Ctrl+click on selected)
    Toggle,

    /// Box select
    Box,

    /// Lasso select
    Lasso,
}

/// Selection filter - what types of entities can be selected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionFilter {
    /// Select any entity
    Any,

    /// Select only bodies/solids
    Body,

    /// Select only faces
    Face,

    /// Select only edges
    Edge,

    /// Select only vertices
    Vertex,

    /// Select only features
    Feature,
}
