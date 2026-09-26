// Geometry builders. Everything is procedural: vertex colors only, zero textures.
// This keeps the game tiny, load-free and extremely fast on weak GPUs.

use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub col: [f32; 3],
}

pub struct MeshBuilder {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u16>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self { verts: Vec::new(), idx: Vec::new() }
    }

    pub fn v(&mut self, p: Vec3, n: Vec3, c: Vec3) -> u16 {
        self.verts.push(Vertex {
            pos: p.to_array(),
            nrm: n.to_array(),
            col: c.to_array(),
        });
        (self.verts.len() - 1) as u16
    }

    pub fn tri(&mut self, a: u16, b: u16, c: u16) {
        self.idx.extend_from_slice(&[a, b, c]);
    }

    pub fn quad(&mut self, a: u16, b: u16, c: u16, d: u16) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// Axis-aligned box from min to max.
    pub fn cube(&mut self, min: Vec3, max: Vec3, col: Vec3) {
        let corners = |x: f32, y: f32, z: f32| Vec3::new(
            if x == 0.0 { min.x } else { max.x },
            if y == 0.0 { min.y } else { max.y },
            if z == 0.0 { min.z } else { max.z },
        );
        let mut face = |selfm: &mut Self, pts: [Vec3; 4], n: Vec3| {
            let a = selfm.v(pts[0], n, col);
            let b = selfm.v(pts[1], n, col);
            let c = selfm.v(pts[2], n, col);
            let d = selfm.v(pts[3], n, col);
            selfm.quad(a, b, c, d);
        };
        // +Z, -Z
        face(self, [corners(0.0,0.0,1.0), corners(1.0,0.0,1.0), corners(1.0,1.0,1.0), corners(0.0,1.0,1.0)], Vec3::Z);
        face(self, [corners(1.0,0.0,0.0), corners(0.0,0.0,0.0), corners(0.0,1.0,0.0), corners(1.0,1.0,0.0)], -Vec3::Z);
        // +X, -X
        face(self, [corners(1.0,0.0,1.0), corners(1.0,0.0,0.0), corners(1.0,1.0,0.0), corners(1.0,1.0,1.0)], Vec3::X);
        face(self, [corners(0.0,0.0,0.0), corners(0.0,0.0,1.0), corners(0.0,1.0,1.0), corners(0.0,1.0,0.0)], -Vec3::X);
        // +Y, -Y
        face(self, [corners(0.0,1.0,1.0), corners(1.0,1.0,1.0), corners(1.0,1.0,0.0), corners(0.0,1.0,0.0)], Vec3::Y);
        face(self, [corners(0.0,0.0,0.0), corners(1.0,0.0,0.0), corners(1.0,0.0,1.0), corners(0.0,0.0,1.0)], -Vec3::Y);
    }

    /// Cylinder along Y from y0 to y1.
    pub fn cylinder(&mut self, r: f32, y0: f32, y1: f32, segs: usize, col: Vec3) {
        for i in 0..segs {
            let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
            let n0 = Vec3::new(a0.cos(), 0.0, a0.sin());
            let n1 = Vec3::new(a1.cos(), 0.0, a1.sin());
            let p00 = Vec3::new(n0.x * r, y0, n0.z * r);
            let p01 = Vec3::new(n0.x * r, y1, n0.z * r);
            let p10 = Vec3::new(n1.x * r, y0, n1.z * r);
            let p11 = Vec3::new(n1.x * r, y1, n1.z * r);
            let a = self.v(p00, n0, col);
            let b = self.v(p01, n0, col);
            let c = self.v(p11, n1, col);
            let d = self.v(p10, n1, col);
            self.quad(a, b, c, d);
        }
    }

    /// Cone along Y, base radius r at y0, tip at y1.
    pub fn cone(&mut self, r: f32, y0: f32, y1: f32, segs: usize, col: Vec3) {
        let tip = Vec3::new(0.0, y1, 0.0);
        for i in 0..segs {
            let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
            let n0 = Vec3::new(a0.cos(), 0.0, a0.sin());
            let n1 = Vec3::new(a1.cos(), 0.0, a1.sin());
            let p0 = Vec3::new(n0.x * r, y0, n0.z * r);
            let p1 = Vec3::new(n1.x * r, y0, n1.z * r);
            let m = (p0 + p1 + tip) / 3.0;
            let n = (m - Vec3::new(0.0, y0 - r * 0.35, 0.0)).normalize_or_zero();
            let a = self.v(p0, n, col);
            let b = self.v(p1, n, col);
            let t = self.v(tip, n, col);
            self.tri(a, b, t);
        }
    }

    /// UV sphere.
    pub fn sphere(&mut self, r: f32, segs: usize, rings: usize, col: Vec3) {
        for j in 0..rings {
            let p0 = std::f32::consts::FRAC_PI_2 * (j as f32 / rings as f32);
            let p1 = std::f32::consts::FRAC_PI_2 * ((j + 1) as f32 / rings as f32);
            for i in 0..segs {
                let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
                let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
                let pt = |ph: f32, th: f32| {
                    Vec3::new(ph.cos() * th.cos(), ph.sin(), ph.cos() * th.sin()) * r
                };
                let n = |ph: f32, th: f32| {
                    Vec3::new(ph.cos() * th.cos(), ph.sin(), ph.cos() * th.sin())
                };
                let p00 = pt(p0, a0);
                let p01 = pt(p0, a1);
                let p10 = pt(p1, a0);
                let p11 = pt(p1, a1);
                if j == 0 {
                    let a = self.v(p00, n(p0, a0), col);
                    let c = self.v(p10, n(p1, a0), col);
                    let d = self.v(p11, n(p1, a1), col);
                    self.tri(a, c, d);
                } else if j + 1 == rings {
                    let a = self.v(p00, n(p0, a0), col);
                    let b = self.v(p01, n(p0, a1), col);
                    let c = self.v(p10, n(p1, a0), col);
                    self.tri(a, b, c);
                } else {
                    let a = self.v(p00, n(p0, a0), col);
                    let b = self.v(p01, n(p0, a1), col);
                    let c = self.v(p10, n(p1, a0), col);
                    let d = self.v(p11, n(p1, a1), col);
                    self.quad(a, b, c, d);
                }
            }
        }
    }

    /// Torus in the XZ plane (can be flattened along Y).
    pub fn torus(&mut self, ring_r: f32, tube_r: f32, segs: usize, sides: usize, y_flat: f32, col: Vec3) {
        for i in 0..segs {
            let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
            for j in 0..sides {
                let b0 = (j as f32) / sides as f32 * std::f32::consts::TAU;
                let b1 = ((j + 1) as f32) / sides as f32 * std::f32::consts::TAU;
                let pt = |a: f32, b: f32| {
                    let cx = a.cos();
                    let cz = a.sin();
                    let nx = b.cos() * cx;
                    let ny = b.sin() * y_flat;
                    let nz = b.cos() * cz;
                    Vec3::new(cx * ring_r + nx * tube_r, ny * tube_r, cz * ring_r + nz * tube_r)
                };
                let n = |a: f32, b: f32| Vec3::new(b.cos() * a.cos(), b.sin(), b.cos() * a.sin());
                let a = self.v(pt(a0, b0), n(a0, b0), col);
                let b = self.v(pt(a1, b0), n(a1, b0), col);
                let c = self.v(pt(a1, b1), n(a1, b1), col);
                let d = self.v(pt(a0, b1), n(a0, b1), col);
                self.quad(a, b, c, d);
            }
        }
    }

    /// Semi-ellipsoid dome (the haystack core), radius R, height H.
    pub fn dome(&mut self, r: f32, h: f32, segs: usize, rings: usize, col: Vec3) {
        let center = Vec3::new(0.0, h, 0.0);
        for j in 0..rings {
            let p0 = std::f32::consts::FRAC_PI_2 * (j as f32 / rings as f32);
            let p1 = std::f32::consts::FRAC_PI_2 * ((j + 1) as f32 / rings as f32);
            for i in 0..segs {
                let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
                let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
                let pt = |ph: f32, th: f32| {
                    Vec3::new(ph.cos() * th.cos() * r, ph.sin() * h, ph.cos() * th.sin() * r)
                };
                let n = |ph: f32, th: f32| {
                    let n = Vec3::new(
                        ph.cos() * th.cos() / r,
                        ph.sin() / h,
                        ph.cos() * th.sin() / r,
                    );
                    n.normalize_or_zero()
                };
                let p00 = pt(p0, a0);
                let p01 = pt(p0, a1);
                let p10 = pt(p1, a0);
                let p11 = pt(p1, a1);
                // position-hash clumpy variation: hides the triangulation completely
                let hx = (p00.x * 12.9898 + p00.z * 78.2330 + p00.y * 37.7190).sin() * 43758.5453;
                let h = hx.fract();
                let base = col * (0.78 + 0.55 * h) * (0.88 + 0.24 * (1.0 - p0.sin()));
                if j == 0 {
                    let a = self.v(p00, n(p0, a0), base);
                    let c = self.v(p10, n(p1, a0), base);
                    let d = self.v(p11, n(p1, a1), base);
                    self.tri(a, d, c);
                } else if j + 1 == rings {
                    let a = self.v(p00, n(p0, a0), base);
                    let b = self.v(p01, n(p0, a1), base);
                    let c = self.v(p10, n(p1, a0), base);
                    self.tri(a, c, b);
                } else {
                    let a = self.v(p00, n(p0, a0), base);
                    let b = self.v(p01, n(p0, a1), base);
                    let c = self.v(p10, n(p1, a0), base);
                    let d = self.v(p11, n(p1, a1), base);
                    self.quad(c, b, a, d);
                }
            }
        }
        let _ = center;
    }

    pub fn build(self) -> (Vec<Vertex>, Vec<u16>) {
        (self.verts, self.idx)
    }
}

pub fn transform_mesh(verts: &mut [Vertex], m: Mat4) {
    for v in verts.iter_mut() {
        let p = m.transform_point3(Vec3::from(v.pos));
        let n = m.transform_vector3(Vec3::from(v.nrm)).normalize_or_zero();
        v.pos = p.to_array();
        v.nrm = n.to_array();
    }
}

// ---------------------------------------------------------------------------
// Game meshes
// ---------------------------------------------------------------------------

pub struct StrawMesh;
impl StrawMesh {
    /// 6 verts (two crossed triangles), local Y = length axis, unit length.
    /// col.r encodes 0 = base / 1 = tip for the shader gradient.
    pub fn build() -> (Vec<Vertex>, Vec<u16>) {
        let t = 0.5; // half thickness in local units (scaled by shader to ~1.2cm)
        let bend = 0.10;
        let tip = Vec3::new(0.0, 1.0, bend);
        let gold_a = Vec3::new(0.0, 0.0, 0.0);
        let gold_b = Vec3::new(1.0, 0.0, 0.0); // tip flag in .r
        let mut mb = MeshBuilder::new();
        // tri A (XY plane)
        let a0 = mb.v(Vec3::new(-t, 0.0, 0.0), Vec3::Z, gold_a);
        let a1 = mb.v(Vec3::new(t, 0.0, 0.0), Vec3::Z, gold_a);
        let a2 = mb.v(tip, Vec3::Z, gold_b);
        mb.tri(a0, a1, a2);
        // tri B (ZY plane)
        let b0 = mb.v(Vec3::new(0.0, 0.0, -t), Vec3::X, gold_a);
        let b1 = mb.v(Vec3::new(0.0, 0.0, t), Vec3::X, gold_a);
        let b2 = mb.v(tip, Vec3::X, gold_b);
        mb.tri(b0, b1, b2);
        (mb.verts, mb.idx)
    }
}

/// Merge a MeshBuilder into another, applying a transform to its vertices.
fn append_mesh(dst: &mut MeshBuilder, src: MeshBuilder, m: Mat4) {
    let mut verts = src.verts;
    transform_mesh(&mut verts, m);
    let base = dst.verts.len() as u16;
    for mut v in verts {
        let n = Vec3::from(v.nrm).normalize_or_zero();
        v.nrm = n.to_array();
        dst.verts.push(v);
    }
    for i in src.idx {
        dst.idx.push(i + base);
    }
}

/// Real sewing needle proportions: ~70 mm long, ~1.8 mm shaft diameter.
/// Built along +X, lying flat (centered at origin).
pub fn needle_mesh() -> (Vec<Vertex>, Vec<u16>) {
    const LEN: f32 = 0.070;
    const R: f32 = 0.0009;
    const EYE_R: f32 = 0.0021;
    const STEEL: Vec3 = Vec3::new(0.80, 0.82, 0.85);
    const STEEL_D: Vec3 = Vec3::new(0.62, 0.65, 0.70);

    let body_end = LEN - 0.010;
    let mut mb = MeshBuilder::new();

    // Shaft: cylinder built along Y, rotated onto +X starting at x=0.
    let mut cyl = MeshBuilder::new();
    cyl.cylinder(R, 0.0, body_end - EYE_R * 2.2, 8, STEEL);
    let rot = Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(0.0, EYE_R * 2.2, 0.0));
    append_mesh(&mut mb, cyl, rot);

    // Tapered tip cone at the end.
    let mut cone = MeshBuilder::new();
    cone.cone(R, 0.0, 0.010, 8, STEEL_D);
    let rot = Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(0.0, body_end, 0.0));
    append_mesh(&mut mb, cone, rot);

    // Eye: flattened torus standing in the YZ plane at the back end.
    let mut eye = MeshBuilder::new();
    eye.torus(EYE_R, 0.0006, 10, 6, 0.6, STEEL_D);
    // torus is built in the XZ plane; rotate onto the YZ plane, center at x=EYE_R.
    let rot = Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(EYE_R + 0.0010, 0.0, 0.0));
    append_mesh(&mut mb, eye, rot);

    (mb.verts, mb.idx)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValuableKind {
    Coin,
    Button,
    Thimble,
    Key,
    Spoon,
    Fork,
    Horseshoe,
    Ring,
    Locket,
    Watch,
}

impl ValuableKind {
    pub const ALL: [ValuableKind; 10] = [
        ValuableKind::Coin, ValuableKind::Button, ValuableKind::Thimble, ValuableKind::Key,
        ValuableKind::Spoon, ValuableKind::Fork, ValuableKind::Horseshoe, ValuableKind::Ring,
        ValuableKind::Locket, ValuableKind::Watch,
    ];
    pub fn name(&self) -> &'static str {
        match self {
            ValuableKind::Coin => "OLD COIN",
            ValuableKind::Button => "BRASS BUTTON",
            ValuableKind::Thimble => "SILVER THIMBLE",
            ValuableKind::Key => "RUSTY KEY",
            ValuableKind::Spoon => "SPOON",
            ValuableKind::Fork => "FORK",
            ValuableKind::Horseshoe => "HORSESHOE",
            ValuableKind::Ring => "GOLD RING",
            ValuableKind::Locket => "LOCKET",
            ValuableKind::Watch => "POCKET WATCH",
        }
    }
    pub fn value(&self) -> i32 {
        match self {
            ValuableKind::Coin => 12,
            ValuableKind::Button => 4,
            ValuableKind::Thimble => 8,
            ValuableKind::Key => 22,
            ValuableKind::Spoon => 14,
            ValuableKind::Fork => 18,
            ValuableKind::Horseshoe => 45,
            ValuableKind::Ring => 70,
            ValuableKind::Locket => 95,
            ValuableKind::Watch => 130,
        }
    }
    pub fn mesh_index(&self) -> usize {
        match self {
            ValuableKind::Coin => 3,
            ValuableKind::Button => 4,
            ValuableKind::Thimble => 5,
            ValuableKind::Key => 6,
            ValuableKind::Spoon => 7,
            ValuableKind::Fork => 8,
            ValuableKind::Horseshoe => 9,
            ValuableKind::Ring => 10,
            ValuableKind::Locket => 11,
            ValuableKind::Watch => 12,
        }
    }
}

/// All prop meshes, indexed. 0 = haystack core dome, 1 = needle, 2 = detector
/// viewmodel, 3.. = valuables, 13 = baler machine, 14 = vacuum machine.
pub fn build_all_meshes() -> Vec<(Vec<Vertex>, Vec<u16>)> {
    let mut out: Vec<(Vec<Vertex>, Vec<u16>)> = Vec::new();

    // 0: haystack dark core (semi-ellipsoid slightly inside the straws)
    let mut mb = MeshBuilder::new();
    mb.dome(0.78, 0.78, 48, 12, Vec3::new(0.16, 0.11, 0.05));
    out.push(mb.build());

    // 1: needle
    out.push(needle_mesh());

    // 2: handheld metal detector (yellow/black like the real one)
    let mut mb = MeshBuilder::new();
    let black = Vec3::new(0.06, 0.06, 0.06);
    let yellow = Vec3::new(0.95, 0.78, 0.10);
    mb.cube(Vec3::new(-0.025, 0.0, -0.018), Vec3::new(0.025, 0.13, 0.018), black); // handle
    mb.cube(Vec3::new(-0.022, 0.13, -0.016), Vec3::new(0.022, 0.30, 0.016), yellow); // shaft
    mb.cube(Vec3::new(-0.008, 0.30, -0.006), Vec3::new(0.008, 0.40, 0.006), black); // neck
    // flat elliptical search head
    let mut head = MeshBuilder::new();
    head.cylinder(0.055, -0.012, 0.012, 12, yellow);
    let (mut hv, hi) = head.build();
    transform_mesh(&mut hv, Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0)));
    let base = mb.verts.len() as u16;
    mb.verts.extend(hv);
    for i in hi { mb.idx.push(i + base); }
    out.push(mb.build());

    // 3: coin
    let mut mb = MeshBuilder::new();
    mb.cylinder(0.011, 0.0, 0.0022, 10, Vec3::new(0.85, 0.65, 0.20));
    out.push(mb.build());
    // 4: button
    let mut mb = MeshBuilder::new();
    mb.cylinder(0.0075, 0.0, 0.0018, 8, Vec3::new(0.72, 0.58, 0.22));
    out.push(mb.build());
    // 5: thimble
    let mut mb = MeshBuilder::new();
    mb.sphere(0.009, 8, 4, Vec3::new(0.75, 0.75, 0.78));
    out.push(mb.build());
    // 6: key
    let mut mb = MeshBuilder::new();
    mb.cube(Vec3::new(0.0, 0.0, -0.0012), Vec3::new(0.045, 0.0024, 0.0012), Vec3::new(0.60, 0.45, 0.25));
    mb.cube(Vec3::new(-0.008, 0.0, -0.006), Vec3::new(0.004, 0.0024, 0.008), Vec3::new(0.60, 0.45, 0.25));
    out.push(mb.build());
    // 7: spoon
    let mut mb = MeshBuilder::new();
    mb.cube(Vec3::new(0.0, 0.0, -0.0025), Vec3::new(0.075, 0.002, 0.0025), Vec3::new(0.78, 0.80, 0.83));
    mb.sphere(0.010, 8, 4, Vec3::new(0.78, 0.80, 0.83));
    out.push(mb.build());
    // 8: fork
    let mut mb = MeshBuilder::new();
    mb.cube(Vec3::new(0.0, 0.0, -0.0022), Vec3::new(0.070, 0.002, 0.0022), Vec3::new(0.78, 0.80, 0.83));
    for k in 0..4 {
        let z = -0.0066 + k as f32 * 0.0044;
        mb.cube(Vec3::new(0.070, 0.0, z - 0.0008), Vec3::new(0.012, 0.002, z + 0.0008), Vec3::new(0.78, 0.80, 0.83));
    }
    out.push(mb.build());
    // 9: horseshoe
    let mut mb = MeshBuilder::new();
    mb.torus(0.045, 0.007, 14, 6, 0.35, Vec3::new(0.45, 0.42, 0.40));
    out.push(mb.build());
    // 10: ring
    let mut mb = MeshBuilder::new();
    mb.torus(0.0095, 0.0014, 10, 6, 1.0, Vec3::new(0.88, 0.70, 0.22));
    out.push(mb.build());
    // 11: locket
    let mut mb = MeshBuilder::new();
    mb.sphere(0.013, 8, 4, Vec3::new(0.85, 0.68, 0.25));
    out.push(mb.build());
    // 12: pocket watch
    let mut mb = MeshBuilder::new();
    mb.cylinder(0.016, 0.0, 0.006, 12, Vec3::new(0.80, 0.78, 0.72));
    out.push(mb.build());

    // 13: hay baler machine (big green box with funnel + wheels)
    let mut mb = MeshBuilder::new();
    let green = Vec3::new(0.16, 0.42, 0.20);
    let dark = Vec3::new(0.10, 0.10, 0.11);
    mb.cube(Vec3::new(-0.9, 0.25, -0.7), Vec3::new(0.9, 1.6, 0.7), green);
    mb.cube(Vec3::new(-0.55, 1.6, -0.45), Vec3::new(0.55, 1.85, 0.45), dark);
    mb.cone(0.5, 1.85, 2.3, 8, dark); // intake funnel (inverted look is fine)
    mb.cylinder(0.3, 0.0, 0.25, 8, dark); // ground roller
    out.push(mb.build());

    // 14: hay vacuum (big orange industrial unit)
    let mut mb = MeshBuilder::new();
    let orange = Vec3::new(0.85, 0.42, 0.10);
    mb.cube(Vec3::new(-1.1, 0.2, -0.8), Vec3::new(1.1, 2.1, 0.8), orange);
    mb.cube(Vec3::new(-0.4, 2.1, -0.4), Vec3::new(0.4, 2.5, 0.4), dark);
    mb.cylinder(0.16, 0.0, 2.2, 8, dark); // intake pipe
    out.push(mb.build());

    out
}
