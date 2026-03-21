use glam::{Mat4, Vec2, Vec3};

pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    pub aspect: f32,
    pub target_center: Vec2,
    pub target_zoom: f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 0.2,
            aspect,
            target_center: Vec2::ZERO,
            target_zoom: 0.2,
        }
    }

    pub fn view_proj(&self) -> Mat4 {
        let half_w = 10.0 / self.zoom;
        let half_h = half_w / self.aspect;
        let view = Mat4::look_at_rh(
            Vec3::new(self.center.x, self.center.y, 10.0),
            Vec3::new(self.center.x, self.center.y, 0.0),
            Vec3::Y,
        );
        let proj = Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, 0.1, 100.0);
        proj * view
    }

    /// Pan by screen-space pixel delta. window_size needed for correct scaling.
    pub fn pan(&mut self, delta: Vec2, window_size: Vec2) {
        // Convert pixel delta to world-space delta:
        // Screen width in pixels maps to 2 * half_w = 20.0 / zoom in world units
        let world_per_pixel_x = 20.0 / (self.target_zoom * window_size.x);
        let world_per_pixel_y = 20.0 / (self.target_zoom * self.aspect * window_size.y);
        self.target_center.x -= delta.x * world_per_pixel_x;
        self.target_center.y += delta.y * world_per_pixel_y; // flip Y (screen Y is down, world Y is up)
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.target_zoom = (self.target_zoom * factor).clamp(0.1, 50.0);
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    pub fn update(&mut self, dt: f32) {
        let speed = 8.0 * dt;
        let t = speed.min(1.0);
        self.center = self.center.lerp(self.target_center, t);
        self.zoom = self.zoom + (self.target_zoom - self.zoom) * t;
    }

    pub fn reset(&mut self) {
        self.target_center = Vec2::ZERO;
        self.target_zoom = 0.2;
    }

    /// Fit all nodes in view by computing bounding box and setting zoom/center accordingly.
    pub fn fit_to_scene(&mut self, positions: &[(f32, f32)]) {
        if positions.is_empty() {
            return;
        }
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for &(x, y) in positions {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        // Add padding
        let pad = 2.0;
        min_x -= pad;
        max_x += pad;
        min_y -= pad;
        max_y += pad;

        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        let width = max_x - min_x;
        let height = max_y - min_y;

        // Camera shows half_w = 10.0 / zoom on each side, so total = 20.0 / zoom
        let zoom_w = 20.0 / width.max(0.1);
        let zoom_h = 20.0 / (height * self.aspect).max(0.1);
        let zoom = zoom_w.min(zoom_h).clamp(0.01, 50.0);

        self.target_center = Vec2::new(center_x, center_y);
        self.target_zoom = zoom;
    }
}
