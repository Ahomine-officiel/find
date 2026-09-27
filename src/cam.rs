use glam::{Mat4, Vec3};

// First-person camera with smooth inspect zoom (right mouse button).
pub struct Camera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,   // 1.0 = normal, lower = zoomed in
    pub fov: f32,    // base vertical fov (degrees)
}

impl Camera {
    pub fn new(fov: f32) -> Self {
        Self {
            pos: Vec3::new(0.0, 1.62, 46.0),
            yaw: 0.0, // looking towards -Z (pile at origin)
            pitch: -0.06,
            zoom: 1.0,
            fov,
        }
    }

    pub fn fwd(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), -self.yaw.cos() * cp)
    }

    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin())
    }

    pub fn up(&self) -> Vec3 {
        self.right().cross(self.fwd())
    }

    pub fn effective_fov(&self) -> f32 {
        // Zoom narrows fov down to ~9 degrees for needle inspection.
        let z = self.zoom.clamp(0.12, 1.0);
        let target = self.fov.to_radians();
        let zoomed = 8.0f32.to_radians();
        // exponential blend so zooming feels smooth at both ends
        target * (zoomed / target).powf(1.0 - z)
    }

    pub fn view_proj(&self, aspect: f32, near: f32, far: f32) -> Mat4 {
        let proj = Mat4::perspective_rh(self.effective_fov(), aspect, near, far);
        let view = Mat4::look_to_rh(self.pos, self.fwd(), Vec3::Y);
        proj * view
    }

    pub fn near(&self) -> f32 {
        // Small near plane when zoomed so the needle never clips.
        let z = self.zoom.clamp(0.12, 1.0);
        0.06 + (0.004 - 0.06) * (1.0 - z)
    }
}
