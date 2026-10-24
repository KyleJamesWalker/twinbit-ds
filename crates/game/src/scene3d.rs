//! Lit 3D meshes, built in code, plus the per-frame scene a backend draws under the 2D shapes.

use std::f32::consts::TAU;

use glam::{Mat4, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex3 {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeshId {
    Die,
    Pips,
    Tray,
    Shadow,
}

impl MeshId {
    pub const ALL: [MeshId; 4] = [MeshId::Die, MeshId::Pips, MeshId::Tray, MeshId::Shadow];

    /// Decals lie flat on another surface and need a depth offset to win against it.
    pub fn is_decal(self) -> bool {
        matches!(self, MeshId::Pips | MeshId::Shadow)
    }

    pub fn build(self) -> Vec<Vertex3> {
        let mut out = Vec::new();
        match self {
            MeshId::Die => cuboid(&mut out, Vec3::ZERO, Vec3::splat(DIE_HALF), IVORY),
            MeshId::Pips => pip_decals(&mut out),
            MeshId::Tray => tray(&mut out),
            MeshId::Shadow => disc(&mut out, Vec3::ZERO, Vec3::X, Vec3::NEG_Z, 1.0, [0.03, 0.16, 0.08]),
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct Scene3d {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub light: Vec3,
    pub draws: Vec<(MeshId, Mat4)>,
}

pub const DIE_HALF: f32 = 0.5;
/// Inner half-extents of the tray in x and z; the floor is at y = 0.
pub const TRAY_X: f32 = 4.0;
pub const TRAY_Z: f32 = 3.0;

const IVORY: [f32; 3] = [0.95, 0.93, 0.86];
const PIP: [f32; 3] = [0.08, 0.08, 0.1];
const PIP_ONE: [f32; 3] = [0.75, 0.08, 0.1];
const FELT: [f32; 3] = [0.08, 0.38, 0.2];
const WOOD: [f32; 3] = [0.45, 0.27, 0.14];
const PIP_R: f32 = 0.085;
const PIP_OFFSET: f32 = 0.25;
/// Enough to clear a 16-bit depth buffer from the overhead camera.
const PIP_LIFT: f32 = 0.02;

/// Face normals and their values; opposite faces sum to seven.
pub const FACES: [(Vec3, u8); 6] = [
    (Vec3::Y, 1),
    (Vec3::NEG_Y, 6),
    (Vec3::X, 2),
    (Vec3::NEG_X, 5),
    (Vec3::Z, 3),
    (Vec3::NEG_Z, 4),
];

fn tri(out: &mut Vec<Vertex3>, a: Vec3, b: Vec3, c: Vec3, n: Vec3, color: [f32; 3]) {
    for p in [a, b, c] {
        out.push(Vertex3 { pos: p.to_array(), normal: n.to_array(), color });
    }
}

fn axes(n: Vec3) -> (Vec3, Vec3) {
    let u = if n.y.abs() > 0.5 { Vec3::X } else { Vec3::Y.cross(n).normalize() };
    (u, n.cross(u))
}

fn quad(out: &mut Vec<Vertex3>, center: Vec3, u: Vec3, v: Vec3, n: Vec3, color: [f32; 3]) {
    let [a, b, c, d] = [center - u - v, center + u - v, center + u + v, center - u + v];
    tri(out, a, b, c, n, color);
    tri(out, a, c, d, n, color);
}

fn cuboid(out: &mut Vec<Vertex3>, center: Vec3, half: Vec3, color: [f32; 3]) {
    for (n, _) in FACES {
        let (u, v) = axes(n);
        quad(out, center + n * (n.abs() * half).length(), u * (u.abs() * half).length(), v * (v.abs() * half).length(), n, color);
    }
}

fn disc(out: &mut Vec<Vertex3>, center: Vec3, u: Vec3, v: Vec3, r: f32, color: [f32; 3]) {
    let n = u.cross(v);
    const SEGMENTS: usize = 12;
    for i in 0..SEGMENTS {
        let (a0, a1) = (i as f32 / SEGMENTS as f32 * TAU, (i + 1) as f32 / SEGMENTS as f32 * TAU);
        let p0 = center + (u * a0.cos() + v * a0.sin()) * r;
        let p1 = center + (u * a1.cos() + v * a1.sin()) * r;
        tri(out, center, p0, p1, n, color);
    }
}

fn pips(value: u8) -> &'static [(f32, f32)] {
    const O: f32 = PIP_OFFSET;
    match value {
        1 => &[(0.0, 0.0)],
        2 => &[(-O, -O), (O, O)],
        3 => &[(-O, -O), (0.0, 0.0), (O, O)],
        4 => &[(-O, -O), (O, -O), (-O, O), (O, O)],
        5 => &[(-O, -O), (O, -O), (0.0, 0.0), (-O, O), (O, O)],
        _ => &[(-O, -O), (O, -O), (-O, 0.0), (O, 0.0), (-O, O), (O, O)],
    }
}

fn pip_decals(out: &mut Vec<Vertex3>) {
    for (n, value) in FACES {
        let (u, v) = axes(n);
        let color = if value == 1 { PIP_ONE } else { PIP };
        let r = if value == 1 { PIP_R * 1.6 } else { PIP_R };
        for &(a, b) in pips(value) {
            disc(out, n * (DIE_HALF + PIP_LIFT) + u * a + v * b, u, v, r, color);
        }
    }
}

fn tray(out: &mut Vec<Vertex3>) {
    quad(out, Vec3::ZERO, Vec3::X * (TRAY_X + 3.0), Vec3::NEG_Z * (TRAY_Z + 3.0), Vec3::Y, FELT);
    let (t, h) = (0.15, 0.35);
    cuboid(out, Vec3::new(0.0, h, -(TRAY_Z + t)), Vec3::new(TRAY_X + 2.0 * t, h, t), WOOD);
    cuboid(out, Vec3::new(0.0, h, TRAY_Z + t), Vec3::new(TRAY_X + 2.0 * t, h, t), WOOD);
    cuboid(out, Vec3::new(-(TRAY_X + t), h, 0.0), Vec3::new(t, h, TRAY_Z), WOOD);
    cuboid(out, Vec3::new(TRAY_X + t, h, 0.0), Vec3::new(t, h, TRAY_Z), WOOD);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_has_twenty_one_pips() {
        assert_eq!(MeshId::Pips.build().len() / 3, 21 * 12);
    }

    #[test]
    fn pips_sit_on_their_face() {
        for v in MeshId::Pips.build() {
            let p = Vec3::from_array(v.pos);
            let n = Vec3::from_array(v.normal);
            assert!((p.dot(n) - (DIE_HALF + PIP_LIFT)).abs() < 1e-4);
        }
    }
}
