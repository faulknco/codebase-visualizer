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
            zoom: 1.0,
            aspect,
            target_center: Vec2::ZERO,
            target_zoom: 1.0,
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

    pub fn pan(&mut self, delta: Vec2) {
        self.target_center -= delta / self.zoom;
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
        self.target_zoom = 1.0;
    }
}
