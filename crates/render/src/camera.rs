//! Cámara FPS + frustum culling.

use glam::{IVec3, Mat4, Vec3, Vec4, Vec4Swizzles};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub aspect: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Vec3::new(0.0, 100.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            fov: 70f32.to_radians(),
            near: 0.1,
            far: 800.0,
            aspect: 16.0 / 9.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), -self.yaw.cos() * cp).normalize()
    }
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }
    pub fn view(&self) -> Mat4 {
        Mat4::look_to_rh(self.pos, self.forward(), Vec3::Y)
    }
    pub fn proj(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect.max(0.01), self.near, self.far)
    }
    pub fn view_proj(&self) -> Mat4 {
        self.proj() * self.view()
    }
    /// Planos del frustum (left, right, bottom, top, near, far).
    pub fn frustum_planes(&self) -> [Vec4; 6] {
        let m = self.view_proj();
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        [
            r3 + r0,
            r3 - r0,
            r3 + r1,
            r3 - r1,
            r3 + r2,
            r3 - r2,
        ]
        .map(|p| p.normalize())
    }
}

/// Test esfera vs frustum.
pub fn sphere_in_frustum(planes: &[Vec4; 6], center: Vec3, radius: f32) -> bool {
    for p in planes {
        let d = p.xyz().dot(center) + p.w;
        if d < -radius {
            return false;
        }
    }
    true
}

/// Test AABB vs frustum.
pub fn aabb_in_frustum(planes: &[Vec4; 6], min: Vec3, max: Vec3) -> bool {
    for p in planes {
        let px = if p.x >= 0.0 { max.x } else { min.x };
        let py = if p.y >= 0.0 { max.y } else { min.y };
        let pz = if p.z >= 0.0 { max.z } else { min.z };
        if p.x * px + p.y * py + p.z * pz + p.w < 0.0 {
            return false;
        }
    }
    true
}

/// Origen mundial del chunk.
pub fn chunk_origin(chunk: IVec3) -> Vec3 {
    Vec3::new(
        (chunk.x * 32) as f32,
        (chunk.y * 32) as f32,
        (chunk.z * 32) as f32,
    )
}

/// Distancia de LOD en chunks a partir de la posición del jugador.
pub fn lod_of(chunk: IVec3, player_chunk: IVec3) -> u8 {
    let d = (chunk - player_chunk).abs();
    let m = d.x.max(d.y).max(d.z);
    match m {
        0..=4 => 0,
        5..=8 => 1,
        9..=14 => 2,
        15..=22 => 3,
        _ => 4,
    }
}
