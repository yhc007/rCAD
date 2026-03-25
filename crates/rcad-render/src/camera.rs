//! Camera system
//!
//! Provides orbit camera controls for 3D CAD viewing.

use crate::CameraUniforms;
use glam::{Mat4, Vec3};
use std::f32::consts::PI;

/// Camera for 3D viewing
#[derive(Debug, Clone)]
pub struct Camera {
    /// Camera position
    pub position: Vec3,

    /// Target/look-at point
    pub target: Vec3,

    /// Up vector
    pub up: Vec3,

    /// Field of view (radians)
    pub fov: f32,

    /// Aspect ratio (width/height)
    pub aspect: f32,

    /// Near clipping plane
    pub near: f32,

    /// Far clipping plane
    pub far: f32,

    /// Orbit distance from target
    orbit_distance: f32,

    /// Orbit azimuth angle (radians)
    orbit_azimuth: f32,

    /// Orbit elevation angle (radians)
    orbit_elevation: f32,
}

impl Camera {
    /// Create a new camera with default settings
    pub fn new(aspect: f32) -> Self {
        let mut camera = Self {
            position: Vec3::new(100.0, 100.0, 100.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: 45.0_f32.to_radians(),
            aspect,
            near: 0.1,
            far: 10000.0,
            orbit_distance: 200.0,
            orbit_azimuth: PI / 4.0,
            orbit_elevation: PI / 6.0,
        };
        camera.update_position();
        camera
    }

    /// Set aspect ratio
    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    /// Get view matrix
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    /// Get projection matrix
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    /// Get view-projection matrix
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Get camera uniforms for GPU
    pub fn uniforms(&self) -> CameraUniforms {
        let view = self.view_matrix();
        let proj = self.projection_matrix();
        let view_proj = proj * view;

        CameraUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            proj: proj.to_cols_array_2d(),
            camera_pos: [self.position.x, self.position.y, self.position.z, 1.0],
        }
    }

    /// Orbit the camera around the target
    pub fn orbit(&mut self, delta_azimuth: f32, delta_elevation: f32) {
        self.orbit_azimuth += delta_azimuth;
        self.orbit_elevation = (self.orbit_elevation + delta_elevation)
            .clamp(-PI / 2.0 + 0.01, PI / 2.0 - 0.01);
        self.update_position();
    }

    /// Pan the camera (move target)
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let view = self.view_matrix();
        let right = Vec3::new(view.col(0).x, view.col(0).y, view.col(0).z);
        let up = Vec3::new(view.col(1).x, view.col(1).y, view.col(1).z);

        let pan_speed = self.orbit_distance * 0.001;
        self.target += right * delta_x * pan_speed;
        self.target += up * delta_y * pan_speed;
        self.update_position();
    }

    /// Zoom (change orbit distance)
    pub fn zoom(&mut self, delta: f32) {
        self.orbit_distance *= 1.0 - delta * 0.1;
        self.orbit_distance = self.orbit_distance.clamp(1.0, 10000.0);
        self.update_position();
    }

    /// Fit camera to view a bounding box
    pub fn fit_to_bounds(&mut self, min: Vec3, max: Vec3) {
        let center = (min + max) * 0.5;
        let size = (max - min).length();

        self.target = center;
        self.orbit_distance = size * 2.0;
        self.update_position();
    }

    /// Set to front view (looking at -Y)
    pub fn set_front_view(&mut self) {
        self.orbit_azimuth = 0.0;
        self.orbit_elevation = 0.0;
        self.update_position();
    }

    /// Set to back view (looking at +Y)
    pub fn set_back_view(&mut self) {
        self.orbit_azimuth = PI;
        self.orbit_elevation = 0.0;
        self.update_position();
    }

    /// Set to left view (looking at +X)
    pub fn set_left_view(&mut self) {
        self.orbit_azimuth = -PI / 2.0;
        self.orbit_elevation = 0.0;
        self.update_position();
    }

    /// Set to right view (looking at -X)
    pub fn set_right_view(&mut self) {
        self.orbit_azimuth = PI / 2.0;
        self.orbit_elevation = 0.0;
        self.update_position();
    }

    /// Set to top view (looking at -Z)
    pub fn set_top_view(&mut self) {
        self.orbit_azimuth = 0.0;
        self.orbit_elevation = PI / 2.0 - 0.01;
        self.update_position();
    }

    /// Set to bottom view (looking at +Z)
    pub fn set_bottom_view(&mut self) {
        self.orbit_azimuth = 0.0;
        self.orbit_elevation = -PI / 2.0 + 0.01;
        self.update_position();
    }

    /// Set to isometric view
    pub fn set_isometric_view(&mut self) {
        self.orbit_azimuth = PI / 4.0;
        self.orbit_elevation = (1.0_f32 / 3.0_f32.sqrt()).atan();
        self.update_position();
    }

    /// Update camera position from orbit parameters
    fn update_position(&mut self) {
        let x = self.orbit_distance * self.orbit_elevation.cos() * self.orbit_azimuth.sin();
        let y = self.orbit_distance * self.orbit_elevation.sin();
        let z = self.orbit_distance * self.orbit_elevation.cos() * self.orbit_azimuth.cos();

        self.position = self.target + Vec3::new(x, y, z);
    }

    /// Convert screen coordinates to world ray
    pub fn screen_to_ray(&self, screen_x: f32, screen_y: f32, screen_width: f32, screen_height: f32) -> Ray {
        // Convert screen coordinates to NDC
        let ndc_x = (2.0 * screen_x / screen_width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / screen_height);

        // Get inverse view-projection matrix
        let inv_view_proj = self.view_projection_matrix().inverse();

        // Create points in clip space
        let near_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 0.0));
        let far_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));

        let direction = (far_point - near_point).normalize();

        Ray {
            origin: self.position,
            direction,
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(16.0 / 9.0)
    }
}

/// A ray in 3D space
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    /// Ray origin
    pub origin: Vec3,

    /// Ray direction (normalized)
    pub direction: Vec3,
}

impl Ray {
    /// Create a new ray
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Get a point along the ray at distance t
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// Intersect with a plane
    pub fn intersect_plane(&self, plane_point: Vec3, plane_normal: Vec3) -> Option<f32> {
        let denom = plane_normal.dot(self.direction);
        if denom.abs() < 1e-6 {
            return None;
        }

        let t = (plane_point - self.origin).dot(plane_normal) / denom;
        if t >= 0.0 {
            Some(t)
        } else {
            None
        }
    }

    /// Intersect with axis-aligned bounding box
    pub fn intersect_aabb(&self, min: Vec3, max: Vec3) -> Option<f32> {
        let inv_dir = Vec3::new(1.0 / self.direction.x, 1.0 / self.direction.y, 1.0 / self.direction.z);

        let t1 = (min - self.origin) * inv_dir;
        let t2 = (max - self.origin) * inv_dir;

        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        let tmin_max = tmin.x.max(tmin.y).max(tmin.z);
        let tmax_min = tmax.x.min(tmax.y).min(tmax.z);

        if tmax_min >= tmin_max && tmax_min >= 0.0 {
            Some(tmin_max.max(0.0))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_creation() {
        let camera = Camera::new(16.0 / 9.0);
        assert!((camera.aspect - 16.0 / 9.0).abs() < 1e-6);
    }

    #[test]
    fn test_orbit() {
        let mut camera = Camera::new(1.0);
        let initial_pos = camera.position;

        camera.orbit(0.1, 0.0);
        assert_ne!(camera.position, initial_pos);
    }

    #[test]
    fn test_zoom() {
        let mut camera = Camera::new(1.0);
        let initial_distance = camera.orbit_distance;

        camera.zoom(0.5);
        assert!(camera.orbit_distance < initial_distance);
    }

    #[test]
    fn test_ray_plane_intersection() {
        let ray = Ray::new(Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let t = ray.intersect_plane(Vec3::ZERO, Vec3::Y);

        assert!(t.is_some());
        assert!((t.unwrap() - 10.0).abs() < 1e-6);
    }
}
