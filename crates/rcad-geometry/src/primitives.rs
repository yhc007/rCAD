//! Primitive solid shapes
//!
//! Creates basic geometric primitives using truck B-Rep.

use crate::{GeometryError, Result, Solid};
use truck_modeling::*;
use std::f64::consts::PI;

// Re-export cgmath Point3 for convenience
pub use truck_modeling::cgmath::Point3;
pub use truck_modeling::cgmath::Vector3 as CgVector3;

/// Create a box (rectangular prism)
///
/// # Arguments
/// * `width` - Size in X direction
/// * `height` - Size in Y direction
/// * `depth` - Size in Z direction
///
/// The box is centered at the origin.
pub fn create_box(width: f64, height: f64, depth: f64) -> Result<Solid> {
    if width <= 0.0 || height <= 0.0 || depth <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Box dimensions must be positive".to_string(),
        ));
    }

    let half_w = width / 2.0;
    let half_h = height / 2.0;
    let half_d = depth / 2.0;

    // Create vertices using cgmath Point3
    let v0 = builder::vertex(Point3::new(-half_w, -half_h, -half_d));
    let v1 = builder::vertex(Point3::new(half_w, -half_h, -half_d));
    let v2 = builder::vertex(Point3::new(half_w, half_h, -half_d));
    let v3 = builder::vertex(Point3::new(-half_w, half_h, -half_d));
    let v4 = builder::vertex(Point3::new(-half_w, -half_h, half_d));
    let v5 = builder::vertex(Point3::new(half_w, -half_h, half_d));
    let v6 = builder::vertex(Point3::new(half_w, half_h, half_d));
    let v7 = builder::vertex(Point3::new(-half_w, half_h, half_d));

    // Create edges for bottom face
    let edge0 = builder::line(&v0, &v1);
    let edge1 = builder::line(&v1, &v2);
    let edge2 = builder::line(&v2, &v3);
    let edge3 = builder::line(&v3, &v0);

    // Create edges for top face
    let edge4 = builder::line(&v4, &v5);
    let edge5 = builder::line(&v5, &v6);
    let edge6 = builder::line(&v6, &v7);
    let edge7 = builder::line(&v7, &v4);

    // Create vertical edges
    let edge8 = builder::line(&v0, &v4);
    let edge9 = builder::line(&v1, &v5);
    let edge10 = builder::line(&v2, &v6);
    let edge11 = builder::line(&v3, &v7);

    // Create wires for each face
    let bottom_wire = Wire::from_iter(vec![
        edge0.clone(),
        edge1.clone(),
        edge2.clone(),
        edge3.clone(),
    ]);

    let top_wire = Wire::from_iter(vec![
        edge4.inverse(),
        edge7.inverse(),
        edge6.inverse(),
        edge5.inverse(),
    ]);

    let front_wire = Wire::from_iter(vec![
        edge0.inverse(),
        edge8.clone(),
        edge4.clone(),
        edge9.inverse(),
    ]);

    let back_wire = Wire::from_iter(vec![
        edge2.inverse(),
        edge10.clone(),
        edge6.clone(),
        edge11.inverse(),
    ]);

    let left_wire = Wire::from_iter(vec![
        edge3.inverse(),
        edge11.clone(),
        edge7.clone(),
        edge8.inverse(),
    ]);

    let right_wire = Wire::from_iter(vec![
        edge1.inverse(),
        edge9.clone(),
        edge5.clone(),
        edge10.inverse(),
    ]);

    // Create faces
    let bottom_face = builder::try_attach_plane(&[bottom_wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create bottom face: {:?}", e)))?;

    let top_face = builder::try_attach_plane(&[top_wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create top face: {:?}", e)))?;

    let front_face = builder::try_attach_plane(&[front_wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create front face: {:?}", e)))?;

    let back_face = builder::try_attach_plane(&[back_wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create back face: {:?}", e)))?;

    let left_face = builder::try_attach_plane(&[left_wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create left face: {:?}", e)))?;

    let right_face = builder::try_attach_plane(&[right_wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create right face: {:?}", e)))?;

    // Create shell and solid
    let shell = Shell::from_iter(vec![
        bottom_face,
        top_face,
        front_face,
        back_face,
        left_face,
        right_face,
    ]);

    let solid = truck_modeling::Solid::new(vec![shell]);

    Ok(Solid::new(solid))
}

/// Create a cylinder using translation sweep
///
/// # Arguments
/// * `radius` - Cylinder radius
/// * `height` - Cylinder height
///
/// The cylinder is centered at origin with axis along Z.
pub fn create_cylinder(radius: f64, height: f64) -> Result<Solid> {
    if radius <= 0.0 || height <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Cylinder radius and height must be positive".to_string(),
        ));
    }

    let half_h = height / 2.0;

    // Create a circular face at the bottom
    let center = Point3::new(0.0, 0.0, -half_h);

    // Create 4 vertices on the circle
    let v0 = builder::vertex(Point3::new(radius, 0.0, -half_h));
    let v1 = builder::vertex(Point3::new(0.0, radius, -half_h));
    let v2 = builder::vertex(Point3::new(-radius, 0.0, -half_h));
    let v3 = builder::vertex(Point3::new(0.0, -radius, -half_h));

    // Create arcs
    let arc0 = builder::circle_arc(&v0, &v1, center);
    let arc1 = builder::circle_arc(&v1, &v2, center);
    let arc2 = builder::circle_arc(&v2, &v3, center);
    let arc3 = builder::circle_arc(&v3, &v0, center);

    let wire = Wire::from_iter(vec![arc0, arc1, arc2, arc3]);

    let face = builder::try_attach_plane(&[wire])
        .map_err(|e| GeometryError::TopologyError(format!("Failed to create base: {:?}", e)))?;

    // Extrude the face along Z axis using tsweep
    let direction = CgVector3::new(0.0, 0.0, height);
    let solid = builder::tsweep(&face, direction);

    Ok(Solid::new(solid))
}

/// Create a sphere using rotational sweep
///
/// # Arguments
/// * `radius` - Sphere radius
///
/// The sphere is centered at the origin.
pub fn create_sphere(radius: f64) -> Result<Solid> {
    if radius <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Sphere radius must be positive".to_string(),
        ));
    }

    // Create a semicircle arc in the XZ plane from (radius, 0, 0) to (-radius, 0, 0)
    // passing through (0, 0, radius)
    let v_start = builder::vertex(Point3::new(radius, 0.0, 0.0));
    let v_top = builder::vertex(Point3::new(0.0, 0.0, radius));
    let v_end = builder::vertex(Point3::new(-radius, 0.0, 0.0));
    let v_bottom = builder::vertex(Point3::new(0.0, 0.0, -radius));

    let center = Point3::new(0.0, 0.0, 0.0);

    // Create semicircle as two quarter arcs
    let arc1 = builder::circle_arc(&v_start, &v_top, center);
    let arc2 = builder::circle_arc(&v_top, &v_end, center);
    let arc3 = builder::circle_arc(&v_end, &v_bottom, center);
    let arc4 = builder::circle_arc(&v_bottom, &v_start, center);

    let wire = Wire::from_iter(vec![arc1, arc2, arc3, arc4]);

    // Sweep around Z axis
    let axis = CgVector3::new(0.0, 0.0, 1.0);
    let shell = builder::rsweep(&wire, center, axis, Rad(2.0 * PI));

    let solid = truck_modeling::Solid::new(vec![shell]);
    Ok(Solid::new(solid))
}

/// Create a cone using rotational sweep of a closed profile
///
/// # Arguments
/// * `bottom_radius` - Radius at the bottom
/// * `top_radius` - Radius at the top (0 for a pointed cone)
/// * `height` - Cone height
///
/// The cone is centered at origin with axis along Z.
pub fn create_cone(bottom_radius: f64, top_radius: f64, height: f64) -> Result<Solid> {
    if bottom_radius < 0.0 || top_radius < 0.0 || height <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Cone dimensions must be non-negative".to_string(),
        ));
    }

    if bottom_radius == 0.0 && top_radius == 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Cone must have at least one non-zero radius".to_string(),
        ));
    }

    let half_h = height / 2.0;

    // Create a closed profile in the XZ plane:
    // For a pointed cone: triangle from axis to outer edge
    // For a frustum: trapezoid

    let origin = Point3::new(0.0, 0.0, 0.0);
    let axis = CgVector3::new(0.0, 0.0, 1.0);

    if top_radius == 0.0 {
        // Pointed cone - create triangle profile
        let v_axis_bottom = builder::vertex(Point3::new(0.0, 0.0, -half_h));
        let v_outer_bottom = builder::vertex(Point3::new(bottom_radius, 0.0, -half_h));
        let v_apex = builder::vertex(Point3::new(0.0, 0.0, half_h));

        let e_bottom = builder::line(&v_axis_bottom, &v_outer_bottom);
        let e_slant = builder::line(&v_outer_bottom, &v_apex);
        let e_axis = builder::line(&v_apex, &v_axis_bottom);

        let wire = Wire::from_iter(vec![e_bottom, e_slant, e_axis]);
        let face = builder::try_attach_plane(&[wire])
            .map_err(|e| GeometryError::TopologyError(format!("Failed to create profile: {:?}", e)))?;

        let solid = builder::rsweep(&face, origin, axis, Rad(2.0 * PI));
        Ok(Solid::new(solid))

    } else if bottom_radius == 0.0 {
        // Inverted pointed cone
        let v_apex = builder::vertex(Point3::new(0.0, 0.0, -half_h));
        let v_outer_top = builder::vertex(Point3::new(top_radius, 0.0, half_h));
        let v_axis_top = builder::vertex(Point3::new(0.0, 0.0, half_h));

        let e_slant = builder::line(&v_apex, &v_outer_top);
        let e_top = builder::line(&v_outer_top, &v_axis_top);
        let e_axis = builder::line(&v_axis_top, &v_apex);

        let wire = Wire::from_iter(vec![e_slant, e_top, e_axis]);
        let face = builder::try_attach_plane(&[wire])
            .map_err(|e| GeometryError::TopologyError(format!("Failed to create profile: {:?}", e)))?;

        let solid = builder::rsweep(&face, origin, axis, Rad(2.0 * PI));
        Ok(Solid::new(solid))

    } else {
        // Frustum (truncated cone) - create trapezoid profile
        let v_axis_bottom = builder::vertex(Point3::new(0.0, 0.0, -half_h));
        let v_outer_bottom = builder::vertex(Point3::new(bottom_radius, 0.0, -half_h));
        let v_outer_top = builder::vertex(Point3::new(top_radius, 0.0, half_h));
        let v_axis_top = builder::vertex(Point3::new(0.0, 0.0, half_h));

        let e_bottom = builder::line(&v_axis_bottom, &v_outer_bottom);
        let e_slant = builder::line(&v_outer_bottom, &v_outer_top);
        let e_top = builder::line(&v_outer_top, &v_axis_top);
        let e_axis = builder::line(&v_axis_top, &v_axis_bottom);

        let wire = Wire::from_iter(vec![e_bottom, e_slant, e_top, e_axis]);
        let face = builder::try_attach_plane(&[wire])
            .map_err(|e| GeometryError::TopologyError(format!("Failed to create profile: {:?}", e)))?;

        let solid = builder::rsweep(&face, origin, axis, Rad(2.0 * PI));
        Ok(Solid::new(solid))
    }
}

/// Create a torus using rotational sweep
///
/// # Arguments
/// * `major_radius` - Distance from center of torus to center of tube
/// * `minor_radius` - Radius of the tube
///
/// The torus is centered at origin with the main ring in the XY plane.
pub fn create_torus(major_radius: f64, minor_radius: f64) -> Result<Solid> {
    if major_radius <= 0.0 || minor_radius <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Torus radii must be positive".to_string(),
        ));
    }

    if minor_radius >= major_radius {
        return Err(GeometryError::InvalidParameter(
            "Minor radius must be less than major radius".to_string(),
        ));
    }

    // Create a circle in the XZ plane centered at (major_radius, 0, 0)
    let circle_center = Point3::new(major_radius, 0.0, 0.0);

    // Create 4 points on the circle
    let v0 = builder::vertex(Point3::new(major_radius + minor_radius, 0.0, 0.0));
    let v1 = builder::vertex(Point3::new(major_radius, 0.0, minor_radius));
    let v2 = builder::vertex(Point3::new(major_radius - minor_radius, 0.0, 0.0));
    let v3 = builder::vertex(Point3::new(major_radius, 0.0, -minor_radius));

    // Create arcs to form circle
    let arc0 = builder::circle_arc(&v0, &v1, circle_center);
    let arc1 = builder::circle_arc(&v1, &v2, circle_center);
    let arc2 = builder::circle_arc(&v2, &v3, circle_center);
    let arc3 = builder::circle_arc(&v3, &v0, circle_center);

    let wire = Wire::from_iter(vec![arc0, arc1, arc2, arc3]);

    // Sweep around Z axis to create torus
    let origin = Point3::new(0.0, 0.0, 0.0);
    let axis = CgVector3::new(0.0, 0.0, 1.0);
    let shell = builder::rsweep(&wire, origin, axis, Rad(2.0 * PI));

    let solid = truck_modeling::Solid::new(vec![shell]);
    Ok(Solid::new(solid))
}

/// Create a wedge (triangular prism)
///
/// # Arguments
/// * `width` - Size in X direction
/// * `height` - Size in Y direction (the triangular face is in XY)
/// * `depth` - Size in Z direction (extrusion depth)
///
/// The wedge is a right-angled triangle extruded along Z.
pub fn create_wedge(width: f64, height: f64, depth: f64) -> Result<Solid> {
    if width <= 0.0 || height <= 0.0 || depth <= 0.0 {
        return Err(GeometryError::InvalidParameter(
            "Wedge dimensions must be positive".to_string(),
        ));
    }

    let half_d = depth / 2.0;

    // Create front triangle vertices (z = -half_d)
    let v0 = builder::vertex(Point3::new(0.0, 0.0, -half_d));
    let v1 = builder::vertex(Point3::new(width, 0.0, -half_d));
    let v2 = builder::vertex(Point3::new(0.0, height, -half_d));

    // Create back triangle vertices (z = half_d)
    let v3 = builder::vertex(Point3::new(0.0, 0.0, half_d));
    let v4 = builder::vertex(Point3::new(width, 0.0, half_d));
    let v5 = builder::vertex(Point3::new(0.0, height, half_d));

    // Create edges
    let e_f0 = builder::line(&v0, &v1);
    let e_f1 = builder::line(&v1, &v2);
    let e_f2 = builder::line(&v2, &v0);

    let e_b0 = builder::line(&v3, &v4);
    let e_b1 = builder::line(&v4, &v5);
    let e_b2 = builder::line(&v5, &v3);

    let e_s0 = builder::line(&v0, &v3);
    let e_s1 = builder::line(&v1, &v4);
    let e_s2 = builder::line(&v2, &v5);

    // Create wires
    let front_wire = Wire::from_iter(vec![e_f0.clone(), e_f1.clone(), e_f2.clone()]);
    let back_wire = Wire::from_iter(vec![e_b2.inverse(), e_b1.inverse(), e_b0.inverse()]);
    let bottom_wire = Wire::from_iter(vec![e_f0.inverse(), e_s0.clone(), e_b0.clone(), e_s1.inverse()]);
    let slope_wire = Wire::from_iter(vec![e_f1.inverse(), e_s1.clone(), e_b1.clone(), e_s2.inverse()]);
    let left_wire = Wire::from_iter(vec![e_f2.inverse(), e_s2.clone(), e_b2.clone(), e_s0.inverse()]);

    // Create faces
    let front_face = builder::try_attach_plane(&[front_wire])
        .map_err(|e| GeometryError::TopologyError(format!("{:?}", e)))?;
    let back_face = builder::try_attach_plane(&[back_wire])
        .map_err(|e| GeometryError::TopologyError(format!("{:?}", e)))?;
    let bottom_face = builder::try_attach_plane(&[bottom_wire])
        .map_err(|e| GeometryError::TopologyError(format!("{:?}", e)))?;
    let slope_face = builder::try_attach_plane(&[slope_wire])
        .map_err(|e| GeometryError::TopologyError(format!("{:?}", e)))?;
    let left_face = builder::try_attach_plane(&[left_wire])
        .map_err(|e| GeometryError::TopologyError(format!("{:?}", e)))?;

    let shell = Shell::from_iter(vec![front_face, back_face, bottom_face, slope_face, left_face]);
    let solid = truck_modeling::Solid::new(vec![shell]);

    Ok(Solid::new(solid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_box() {
        let solid = create_box(10.0, 20.0, 30.0).unwrap();
        assert!(solid.face_count() > 0);
        assert_eq!(solid.face_count(), 6);
    }

    #[test]
    fn test_create_box_invalid() {
        assert!(create_box(0.0, 10.0, 10.0).is_err());
        assert!(create_box(-1.0, 10.0, 10.0).is_err());
    }

    #[test]
    fn test_create_sphere() {
        let solid = create_sphere(10.0).unwrap();
        assert!(solid.face_count() > 0);
    }

    #[test]
    fn test_create_cylinder() {
        let solid = create_cylinder(5.0, 20.0).unwrap();
        assert!(solid.face_count() > 0);
    }

    #[test]
    fn test_create_cone() {
        // Pointed cone
        let solid = create_cone(10.0, 0.0, 20.0).unwrap();
        assert!(solid.face_count() > 0);

        // Frustum
        let frustum = create_cone(10.0, 5.0, 20.0).unwrap();
        assert!(frustum.face_count() > 0);
    }

    #[test]
    fn test_create_torus() {
        let solid = create_torus(20.0, 5.0).unwrap();
        assert!(solid.face_count() > 0);
    }

    #[test]
    fn test_create_wedge() {
        let solid = create_wedge(10.0, 10.0, 5.0).unwrap();
        assert!(solid.face_count() > 0);
        assert_eq!(solid.face_count(), 5); // 2 triangles + 3 rectangles
    }
}
