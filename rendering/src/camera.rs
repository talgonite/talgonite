use glam::{Mat4, Vec2, Vec3};

#[derive(Debug)]
pub struct Camera {
    pub width: f32,
    pub height: f32,
    pub position: Vec2,
    pub zoom: f32,
}

impl Camera {
    pub fn new(width: f32, height: f32, zoom: f32) -> Self {
        Self {
            width,
            height,
            position: Vec2::new(0.0, 0.0),
            zoom,
        }
    }

    pub fn build_view_projection_matrix(&self) -> [[f32; 4]; 4] {
        // Z range [-1, 1] with reversed-Z (CompareFunction::Greater).
        // Higher z values map to lower NDC, but with Greater comparison and
        // clear to 0.0, higher world z (closer tiles) correctly wins.
        // Changed divisor from 1000 to 512 in calculate_tile_z for better precision.
        let projection = Mat4::orthographic_rh(0.0, self.width, self.height, 0.0, 1.0, -1.0);
        let center_translation =
            Mat4::from_translation(Vec3::new(self.width / 2.0, self.height / 2.0, 0.0).floor());
        let scale = Mat4::from_scale(Vec3::new(self.zoom, self.zoom, 1.0));
        let view_translation = Mat4::from_translation(-self.position.extend(0.0));

        (projection * center_translation * scale * view_translation).to_cols_array_2d()
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 2],
    pub xray_size: f32,
    pub _padding: f32,
    pub tint: [f32; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Mat4::default().to_cols_array_2d(),
            position: [0.0; 2],
            xray_size: 1.0,
            _padding: 0.0,
            tint: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix();
        self.position = camera.position.to_array();
    }
}
