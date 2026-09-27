// Geometry builders. Everything is procedural: vertex colors only, zero textures.
// This keeps the game tiny, load-free and extremely fast on weak GPUs.
//
// Vertex.col.w carries a per-vertex MATERIAL FLAG used by main.wgsl:
//   0.998 checker grid | 0.997 corrugated metal | 0.996 sand floor
//   0.995 wood planks  | 0.994 grass           | 0.993 conveyor belt
//   0.90  emissive red LED | 0.91 emissive green LED | 0.5 needle sparkle
//   0.0 plain lit material

use glam::{Mat4, Vec3};

pub const FLAG_GRID: f32 = 0.998;
pub const FLAG_CORRUGATED: f32 = 0.997;
pub const FLAG_SAND: f32 = 0.996;
pub const FLAG_PLANKS: f32 = 0.995;
pub const FLAG_GRASS: f32 = 0.994;
pub const FLAG_BELT: f32 = 0.993;
pub const FLAG_LED_RED: f32 = 0.90;
pub const FLAG_LED_GREEN: f32 = 0.91;
pub const FLAG_NEEDLE: f32 = 0.5;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub col: [f32; 4],
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
        self.vf(p, n, c, 0.0)
    }

    pub fn vf(&mut self, p: Vec3, n: Vec3, c: Vec3, flag: f32) -> u16 {
        self.verts.push(Vertex {
            pos: p.to_array(),
            nrm: n.to_array(),
            col: [c.x, c.y, c.z, flag],
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
    pub fn box_(&mut self, min: Vec3, max: Vec3, col: Vec3) {
        self.boxf(min, max, col, 0.0);
    }

    pub fn boxf(&mut self, min: Vec3, max: Vec3, col: Vec3, flag: f32) {
        let corners = |x: f32, y: f32, z: f32| Vec3::new(
            if x == 0.0 { min.x } else { max.x },
            if y == 0.0 { min.y } else { max.y },
            if z == 0.0 { min.z } else { max.z },
        );
        let face = |selfm: &mut Self, pts: [Vec3; 4], n: Vec3| {
            let a = selfm.vf(pts[0], n, col, flag);
            let b = selfm.vf(pts[1], n, col, flag);
            let c = selfm.vf(pts[2], n, col, flag);
            let d = selfm.vf(pts[3], n, col, flag);
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

    /// Cylinder along Y from y0 to y1, open at both ends (no caps).
    pub fn tube(&mut self, r: f32, y0: f32, y1: f32, segs: usize, col: Vec3) {
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

    /// Flat ring annulus in the XZ plane between r0 and r1 (both optional caps).
    pub fn ring_flat(&mut self, r0: f32, r1: f32, y: f32, segs: usize, col: Vec3, flag: f32, up: bool) {
        let n = if up { Vec3::Y } else { -Vec3::Y };
        for i in 0..segs {
            let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
            let p00 = Vec3::new(a0.cos() * r0, y, a0.sin() * r0);
            let p01 = Vec3::new(a0.cos() * r1, y, a0.sin() * r1);
            let p10 = Vec3::new(a1.cos() * r0, y, a1.sin() * r0);
            let p11 = Vec3::new(a1.cos() * r1, y, a1.sin() * r1);
            let (a, b, c, d) = if up { (p00, p01, p11, p10) } else { (p00, p10, p11, p01) };
            let a = self.vf(a, n, col, flag);
            let b = self.vf(b, n, col, flag);
            let c = self.vf(c, n, col, flag);
            let d = self.vf(d, n, col, flag);
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

    /// Inverted cone (funnel): rim radius r at y0 tapering DOWN to small r2 at y1.
    pub fn funnel(&mut self, r: f32, y0: f32, r2: f32, y1: f32, segs: usize, col: Vec3) {
        for i in 0..segs {
            let a0 = (i as f32) / segs as f32 * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / segs as f32 * std::f32::consts::TAU;
            let n0 = Vec3::new(a0.cos(), 0.35, a0.sin()).normalize();
            let n1 = Vec3::new(a1.cos(), 0.35, a1.sin()).normalize();
            let p00 = Vec3::new(a0.cos() * r, y0, a0.sin() * r);
            let p01 = Vec3::new(a1.cos() * r, y0, a1.sin() * r);
            let p10 = Vec3::new(a0.cos() * r2, y1, a0.sin() * r2);
            let p11 = Vec3::new(a1.cos() * r2, y1, a1.sin() * r2);
            let a = self.v(p00, n0, col);
            let b = self.v(p01, n1, col);
            let c = self.v(p11, n1, col);
            let d = self.v(p10, n0, col);
            self.quad(a, b, c, d);
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
                let base = col * (0.8 + 0.4 * (1.0 - (p0.sin())));
                if j == 0 {
                    let a = self.v(p00, n(p0, a0), base);
                    let c = self.v(p10, n(p1, a0), col * 0.6);
                    let d = self.v(p11, n(p1, a1), col * 0.6);
                    self.tri(a, c, d);
                } else if j + 1 == rings {
                    let a = self.v(p00, n(p0, a0), base);
                    let b = self.v(p01, n(p0, a1), base);
                    let c = self.v(p10, n(p1, a0), base);
                    self.tri(a, b, c);
                } else {
                    let a = self.v(p00, n(p0, a0), base);
                    let b = self.v(p01, n(p0, a1), base);
                    let c = self.v(p10, n(p1, a0), base);
                    let d = self.v(p11, n(p1, a1), base);
                    self.quad(a, b, c, d);
                }
            }
        }
    }

    /// Flat quad (two triangles) with an arbitrary frame: origin, right, up.
    pub fn quad_f(&mut self, origin: Vec3, right: Vec3, up: Vec3, col: Vec3, flag: f32) {
        let n = right.cross(up).normalize_or_zero();
        let a = self.vf(origin, n, col, flag);
        let b = self.vf(origin + right, n, col, flag);
        let c = self.vf(origin + right + up, n, col, flag);
        let d = self.vf(origin + up, n, col, flag);
        self.quad(a, b, c, d);
    }

    /// Bakes REAL text into the mesh using the 5x7 HUD bitmap font as tiny boxes.
    /// origin = left baseline start, right/up = advance vectors, h = glyph pixel size.
    pub fn text3d(&mut self, text: &str, origin: Vec3, right: Vec3, up: Vec3, h: f32, depth: f32, col: Vec3) {
        let mut off = 0.0f32;
        for ch in text.chars() {
            if ch == ' ' {
                off += 6.0 * h;
                continue;
            }
            let cu = ch.to_ascii_uppercase();
            let Some((_, rows)) = crate::hud::FONT.iter().find(|(fc, _)| *fc == cu) else {
                off += 6.0 * h;
                continue;
            };
            for (ry, bits) in rows.iter().enumerate() {
                for rx in 0..5 {
                    if bits & (1 << (4 - rx)) != 0 {
                        let p0 = origin + right * (off + rx as f32 * h) + up * ((6 - ry) as f32 * h);
                        let r = right * h;
                        let u = up * h;
                        let d = right.cross(up).normalize_or_zero() * depth;
                        // small 3d box for this pixel
                        let c000 = p0;
                        let c100 = p0 + r;
                        let c110 = p0 + r + u;
                        let c010 = p0 + u;
                        let c001 = p0 + d;
                        let c101 = p0 + r + d;
                        let c111 = p0 + r + u + d;
                        let c011 = p0 + u + d;
                        let nrm = right.cross(up).normalize_or_zero();
                        // front + back + minimal sides
                        let a = self.v(c000, -nrm, col); let b = self.v(c100, -nrm, col);
                        let c = self.v(c110, -nrm, col); let e = self.v(c010, -nrm, col);
                        self.quad(a, b, c, e);
                        let a = self.v(c001, nrm, col); let b = self.v(c101, nrm, col);
                        let c = self.v(c111, nrm, col); let e = self.v(c011, nrm, col);
                        self.quad(a, e, c, b);
                        let n2 = up.cross(right).normalize_or_zero();
                        let a = self.v(c010, n2, col); let b = self.v(c110, n2, col);
                        let c = self.v(c111, n2, col); let e = self.v(c011, n2, col);
                        self.quad(a, e, c, b);
                    }
                }
            }
            off += 6.0 * h;
        }
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
        let b1 = mb.v(Vec3::new(0.0, 0.0, t), Vec3::X, gold_b);
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
    let rot = Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(EYE_R + 0.0010, 0.0, 0.0));
    append_mesh(&mut mb, eye, rot);

    // FLAG: sparkle on all verts
    for v in mb.verts.iter_mut() {
        v.col[3] = FLAG_NEEDLE;
    }

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
    pub fn value(&self) -> i64 {
        match self {
            ValuableKind::Coin => 1200,
            ValuableKind::Button => 400,
            ValuableKind::Thimble => 800,
            ValuableKind::Key => 2200,
            ValuableKind::Spoon => 1400,
            ValuableKind::Fork => 1800,
            ValuableKind::Horseshoe => 4500,
            ValuableKind::Ring => 7000,
            ValuableKind::Locket => 9500,
            ValuableKind::Watch => 13000,
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

/// ---------------------------------------------------------------------------
/// THE BARN - the game's single most iconic set piece.
/// Brown corrugated walls, curved roof beams, open gable ends, sandy floor,
/// sliding door with red LED clock + "HAYWAY CO." sign. The endless pile
/// pierces the roof through a central opening.
/// Interior: x in [-23,23], z in [-17,17], eaves at 8.5m, roof opening ~6m.
/// ---------------------------------------------------------------------------
pub fn barn_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();

    const X: f32 = 23.0;
    const Z: f32 = 17.0;
    const WALL_H: f32 = 8.5;

    let corr_brown = Vec3::new(0.40, 0.29, 0.17);
    let corr_light = Vec3::new(0.47, 0.35, 0.21);
    let wood = Vec3::new(0.36, 0.25, 0.15);
    let wood_dark = Vec3::new(0.28, 0.19, 0.11);
    let steel = Vec3::new(0.32, 0.28, 0.24);

    // ---- sand floor (barn interior + apron) ----
    mb.quad_f(
        Vec3::new(-X - 8.0, 0.0, -Z - 8.0),
        Vec3::new((X + 8.0) * 2.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, (Z + 8.0) * 2.0),
        Vec3::new(0.80, 0.68, 0.50),
        FLAG_SAND,
    );

    // ---- walls: corrugated above a plank base band ----
    // -Z wall (front, has the door)
    wall_band(&mut mb, Vec3::new(-X, 0.0, -Z), Vec3::new(X * 2.0, 0.0, 0.0), 0.0, 2.6, wood, FLAG_PLANKS);
    wall_band(&mut mb, Vec3::new(-X, 2.6, -Z), Vec3::new(X * 2.0, 0.0, 0.0), 2.6, WALL_H - 2.6, corr_brown, FLAG_CORRUGATED);
    // +Z wall (back)
    wall_band(&mut mb, Vec3::new(-X, 0.0, Z), Vec3::new(X * 2.0, 0.0, 0.0), 0.0, 2.6, wood, FLAG_PLANKS);
    wall_band(&mut mb, Vec3::new(-X, 2.6, Z), Vec3::new(X * 2.0, 0.0, 0.0), 2.6, WALL_H - 2.6, corr_light, FLAG_CORRUGATED);
    // -X wall
    wall_band(&mut mb, Vec3::new(-X, 0.0, -Z), Vec3::new(0.0, 0.0, Z * 2.0), 0.0, 2.6, wood, FLAG_PLANKS);
    wall_band(&mut mb, Vec3::new(-X, 2.6, -Z), Vec3::new(0.0, 0.0, Z * 2.0), 2.6, WALL_H - 2.6, corr_light, FLAG_CORRUGATED);
    // +X wall
    wall_band(&mut mb, Vec3::new(X, 0.0, -Z), Vec3::new(0.0, 0.0, Z * 2.0), 0.0, 2.6, wood, FLAG_PLANKS);
    wall_band(&mut mb, Vec3::new(X, 2.6, -Z), Vec3::new(0.0, 0.0, Z * 2.0), 2.6, WALL_H - 2.6, corr_brown, FLAG_CORRUGATED);

    // Wall top rails
    for (a, b) in [
        (Vec3::new(-X - 0.3, WALL_H, -Z - 0.3), Vec3::new(X + 0.3, WALL_H + 0.35, -Z + 0.05)),
        (Vec3::new(-X - 0.3, WALL_H, Z - 0.05), Vec3::new(X + 0.3, WALL_H + 0.35, Z + 0.3)),
        (Vec3::new(-X - 0.3, WALL_H, -Z - 0.3), Vec3::new(-X + 0.05, WALL_H + 0.35, Z + 0.3)),
        (Vec3::new(X - 0.05, WALL_H, -Z - 0.3), Vec3::new(X + 0.3, WALL_H + 0.35, Z + 0.3)),
    ] {
        mb.box_(a, b, wood_dark);
    }

    // ---- vertical wall studs (brown, every 4.6m) ----
    let mut px = -X;
    while px <= X + 0.01 {
        mb.box_(Vec3::new(px - 0.12, 0.0, -Z - 0.12), Vec3::new(px + 0.12, WALL_H, -Z - 0.02), wood_dark);
        mb.box_(Vec3::new(px - 0.12, 0.0, Z + 0.02), Vec3::new(px + 0.12, WALL_H, Z + 0.12), wood_dark);
        px += 4.6;
    }
    let mut pz = -Z;
    while pz <= Z + 0.01 {
        mb.box_(Vec3::new(-X - 0.12, 0.0, pz - 0.12), Vec3::new(-X - 0.02, WALL_H, pz + 0.12), wood_dark);
        mb.box_(Vec3::new(X + 0.02, 0.0, pz - 0.12), Vec3::new(X + 0.12, WALL_H, pz + 0.12), wood_dark);
        pz += 4.6;
    }

    // ---- curved roof: arch beams across X (like shot 1/7), roof opening in middle ----
    // Beams span the full X width; the roof surface is a series of quads following
    // the same arc profile, leaving a central rectangular opening (6m x 5.2m).
    let rise = 5.2;
    let y0 = WALL_H;
    // 9 beams along Z
    let nb = 9;
    for i in 0..nb {
        let z = -Z + (2.0 * Z) * (i as f32 / (nb - 1) as f32);
        roof_arc(&mut mb, X, y0, rise, 0.28, 0.34, 14, steel, z);
    }
    // ridge purlins following the arc along Z (3 lines: at apex sides)
    for xr in [-14.0f32, 0.0, 14.0] {
        for i in 0..nb - 1 {
            let za = -Z + (2.0 * Z) * (i as f32 / (nb - 1) as f32);
            let zb = -Z + (2.0 * Z) * ((i + 1) as f32 / (nb - 1) as f32);
            let ya = arc_y(xr, X, y0, rise);
            let _yb = arc_y(xr, X, y0, rise);
            mb.box_(Vec3::new(xr - 0.07, ya - 0.07, za.min(zb)), Vec3::new(xr + 0.07, ya + 0.07, za.max(zb) + 0.001), steel);
        }
    }

    // Roof sheet panels: follow the arc between opening edge and eaves, on both sides.
    // Opening: |x| <= 3.0. Panels from x=3 to x=23 (and mirrored), segmented to follow arc.
    for side in [-1.0f32, 1.0] {
        let segs = 10;
        for i in 0..segs {
            let x0 = 3.0 + (X - 3.0) * (i as f32 / segs as f32);
            let x1 = 3.0 + (X - 3.0) * ((i + 1) as f32 / segs as f32);
            let ya0 = arc_y(side * x0, X, y0, rise);
            let ya1 = arc_y(side * x1, X, y0, rise);
            // panel strip across full Z, slightly above the beams
            let a = Vec3::new(side * x0 - 0.0, ya0 + 0.16, -Z - 0.3);
            let b = Vec3::new(side * x1 - 0.0, ya1 + 0.16, -Z - 0.3);
            let c = Vec3::new(side * x1, ya1 + 0.16, Z + 0.3);
            let d = Vec3::new(side * x0, ya0 + 0.16, Z + 0.3);
            push_quad_4(&mut mb, a, b, c, d, corr_brown * 1.05, FLAG_CORRUGATED);
        }
    }
    // Roof edge fascia
    mb.box_(Vec3::new(-X - 0.4, y0 - 0.1, -Z - 0.4), Vec3::new(X + 0.4, y0 + 0.35, -Z - 0.1), wood_dark);
    mb.box_(Vec3::new(-X - 0.4, y0 - 0.1, Z + 0.1), Vec3::new(X + 0.4, y0 + 0.35, Z + 0.4), wood_dark);
    mb.box_(Vec3::new(-X - 0.4, y0 - 0.1, -Z - 0.4), Vec3::new(-X - 0.1, y0 + 0.35, Z + 0.4), wood_dark);
    mb.box_(Vec3::new(X + 0.1, y0 - 0.1, -Z - 0.4), Vec3::new(X + 0.4, y0 + 0.35, Z + 0.4), wood_dark);

    // ---- sliding door on -Z wall (centered), with hazard stripe, clock, sign ----
    let door_w = 6.0;
    let door_h = 5.0;
    mb.box_(Vec3::new(-door_w / 2.0, 0.0, -Z - 0.14), Vec3::new(door_w / 2.0, door_h, -Z - 0.06), Vec3::new(0.23, 0.30, 0.24));
    // hazard stripes bottom
    for i in 0..12 {
        let x0 = -door_w / 2.0 + i as f32 * 0.5;
        let col = if i % 2 == 0 { Vec3::new(0.85, 0.70, 0.10) } else { Vec3::new(0.08, 0.08, 0.08) };
        mb.box_(Vec3::new(x0, 0.0, -Z - 0.16), Vec3::new(x0 + 0.5, 0.55, -Z - 0.14), col);
    }
    // door rail
    mb.box_(Vec3::new(-door_w / 2.0 - 0.6, door_h + 0.1, -Z - 0.22), Vec3::new(door_w / 2.0 + 0.6, door_h + 0.22, -Z - 0.14), steel);

    // LED clock above the door: dark box + red digits "23:41"
    mb.box_(Vec3::new(-1.1, door_h + 0.5, -Z - 0.20), Vec3::new(1.1, door_h + 1.35, -Z - 0.10), Vec3::new(0.05, 0.05, 0.06));
    mb.text3d("23:41", Vec3::new(-0.85, door_h + 0.72, -Z - 0.21), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.11, 0.015, Vec3::new(1.0, 0.08, 0.04));
    // two small lamps beside the door
    for sx in [-1.0f32, 1.0] {
        mb.box_(Vec3::new(sx * (door_w / 2.0 + 1.2) - 0.12, door_h + 0.9, -Z - 0.18), Vec3::new(sx * (door_w / 2.0 + 1.2) + 0.12, door_h + 1.14, -Z - 0.06), Vec3::new(0.95, 0.92, 0.80));
    }

    // "HAYWAY CO." sign above the clock
    mb.box_(Vec3::new(-2.6, door_h + 1.6, -Z - 0.22), Vec3::new(2.6, door_h + 2.5, -Z - 0.12), Vec3::new(0.88, 0.85, 0.78));
    mb.text3d("HAYWAY CO.", Vec3::new(-2.3, door_h + 1.78, -Z - 0.24), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.115, 0.02, Vec3::new(0.16, 0.12, 0.08));

    // ---- HAYWAY CO. tools shop (left of the door, like shot 1) ----
    shop_counter(&mut mb, Vec3::new(-14.0, 0.0, -13.0), std::f32::consts::FRAC_PI_2 * 0.5);
    // small intake belt next to the shop
    mb.box_(Vec3::new(-11.0, 0.55, -12.6), Vec3::new(-8.4, 0.65, -12.2), Vec3::new(0.30, 0.29, 0.28));
    for lx in [-10.8, -8.6] {
        mb.box_(Vec3::new(lx - 0.06, 0.0, -12.6), Vec3::new(lx + 0.06, 0.55, -12.2), steel);
    }

    // ---- SELL HAY stall (right side, +X, like shot 2; front faces the pile) ----
    sell_stall(&mut mb, Vec3::new(19.0, 0.0, 6.0), std::f32::consts::FRAC_PI_2);

    // ---- conveyor from pile to the sell stall hopper ----
    conveyor(&mut mb, Vec3::new(11.0, 0.35, 1.0), Vec3::new(20.0, 1.5, 4.0));

    (mb.verts, mb.idx)
}

/// One vertical wall band (quad + corrugation handled in shader by flag).
fn wall_band(mb: &mut MeshBuilder, origin: Vec3, along: Vec3, y0: f32, y1: f32, col: Vec3, flag: f32) {
    let up = Vec3::new(0.0, y1 - y0, 0.0);
    let o = origin + Vec3::new(0.0, y0, 0.0);
    // face inward: normal = -along cross up
    let n = along.cross(up).normalize_or_zero();
    let a = mb.vf(o, n, col, flag);
    let b = mb.vf(o + along, n, col, flag);
    let c = mb.vf(o + along + up, n, col, flag);
    let d = mb.vf(o + up, n, col, flag);
    mb.quad(a, b, c, d);
    // outer face too (visible when outside)
    let a = mb.vf(o, -n, col * 0.92, flag);
    let b = mb.vf(o + along, -n, col * 0.92, flag);
    let c = mb.vf(o + along + up, -n, col * 0.92, flag);
    let d = mb.vf(o + up, -n, col * 0.92, flag);
    mb.quad(a, d, c, b);
}

fn arc_y(x: f32, x_max: f32, y0: f32, rise: f32) -> f32 {
    let t = (x.abs() / x_max).clamp(0.0, 1.0);
    y0 + rise * (1.0 - t * t)
}

/// Curved roof truss: box segments following the parabolic arc at fixed z.
fn roof_arc(mb: &mut MeshBuilder, x_max: f32, y0: f32, rise: f32, th: f32, depth: f32, segs: usize, col: Vec3, z: f32) {
    let mut prev = Vec3::new(-x_max, y0, z);
    for i in 1..=segs {
        let x = -x_max + 2.0 * x_max * (i as f32 / segs as f32);
        let y = arc_y(x, x_max, y0, rise);
        let p = Vec3::new(x, y, z);
        let mid = (prev + p) * 0.5;
        let dx = p.x - prev.x;
        let dy = p.y - prev.y;
        let len = (dx * dx + dy * dy).sqrt();
        let ang = dy.atan2(dx);
        let rot = Mat4::from_rotation_z(ang);
        let c1 = rot.transform_point3(Vec3::new(-len * 0.5, -th * 0.5, -depth * 0.5));
        let c2 = rot.transform_point3(Vec3::new(len * 0.5, th * 0.5, depth * 0.5));
        let mn = (c1.min(c2) + mid).to_array();
        let mx = (c1.max(c2) + mid).to_array();
        mb.box_(Vec3::from_array(mn), Vec3::from_array(mx), col);
        prev = p;
    }
}

fn push_quad_4(mb: &mut MeshBuilder, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Vec3, flag: f32) {
    let n = (b - a).cross(d - a).normalize_or_zero();
    let a = mb.vf(a, n, col, flag);
    let b = mb.vf(b, n, col, flag);
    let c = mb.vf(c, n, col, flag);
    let d = mb.vf(d, n, col, flag);
    mb.quad(a, b, c, d);
}

/// Wooden tool shop counter with a small roof + sign (HAYWAY CO. shop).
fn shop_counter(mb: &mut MeshBuilder, at: Vec3, yaw: f32) {
    let wood = Vec3::new(0.42, 0.30, 0.18);
    let wood_d = Vec3::new(0.30, 0.21, 0.12);
    let m = Mat4::from_rotation_y(yaw) * Mat4::from_translation(at);
    let mut s = MeshBuilder::new();
    // counter body
    s.box_(Vec3::new(-1.6, 0.9, -0.7), Vec3::new(1.6, 1.05, 0.7), wood);
    s.box_(Vec3::new(-1.5, 0.0, -0.6), Vec3::new(1.5, 0.9, 0.6), wood_d);
    // backboard + roof
    s.box_(Vec3::new(-1.7, 1.05, -0.75), Vec3::new(1.7, 2.6, -0.6), wood);
    s.box_(Vec3::new(-1.9, 2.6, -0.95), Vec3::new(1.9, 2.75, 1.0), wood_d);
    // side posts
    for sx in [-1.7f32, 1.7] {
        s.box_(Vec3::new(sx - 0.08, 0.0, -0.7), Vec3::new(sx + 0.08, 2.6, -0.55), wood_d);
    }
    // sign on the roof board
    s.box_(Vec3::new(-1.6, 2.15, -0.78), Vec3::new(1.6, 2.55, -0.72), Vec3::new(0.88, 0.85, 0.78));
    s.text3d("TOOLS", Vec3::new(-0.85, 2.2, -0.80), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.12, 0.02, Vec3::new(0.16, 0.12, 0.08));
    // register
    s.box_(Vec3::new(0.7, 1.05, -0.2), Vec3::new(1.15, 1.45, 0.25), Vec3::new(0.15, 0.15, 0.16));
    s.box_(Vec3::new(0.78, 1.45, -0.1), Vec3::new(1.07, 1.52, 0.12), Vec3::new(0.85, 0.83, 0.75));
    append_mesh(mb, s, m);
}

/// SELL HAY stall: kiosk with cash register, big black sign, blackboard
/// "$0.022 PER STRAND", stack of bales, intake hopper + fan unit.
fn sell_stall(mb: &mut MeshBuilder, at: Vec3, yaw: f32) {
    let wood = Vec3::new(0.20, 0.15, 0.10);
    let wood_d = Vec3::new(0.14, 0.10, 0.07);
    let m = Mat4::from_rotation_y(yaw) * Mat4::from_translation(at);
    let mut s = MeshBuilder::new();
    // stall body (open front toward -X local => build facing +Z, rotate -90deg)
    s.box_(Vec3::new(-1.8, 0.0, -0.8), Vec3::new(1.8, 1.1, 0.8), wood);
    s.box_(Vec3::new(-1.9, 1.1, -0.9), Vec3::new(1.9, 1.25, 0.9), wood_d); // counter lip
    // back wall + roof
    s.box_(Vec3::new(-1.9, 1.25, 0.55), Vec3::new(1.9, 3.0, 0.75), wood);
    s.box_(Vec3::new(-2.0, 3.0, -1.0), Vec3::new(2.0, 3.15, 1.0), wood_d);
    for sx in [-1.9f32, 1.9] {
        s.box_(Vec3::new(sx - 0.09, 0.0, -0.9), Vec3::new(sx + 0.09, 3.0, -0.7), wood_d);
    }
    // big black sign "SELL HAY"
    s.box_(Vec3::new(-1.7, 2.3, -0.98), Vec3::new(1.7, 2.95, -0.90), Vec3::new(0.05, 0.05, 0.05));
    s.text3d("SELL HAY", Vec3::new(-1.42, 2.44, -1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.115, 0.02, Vec3::new(0.93, 0.90, 0.82));
    // cash register on the counter
    s.box_(Vec3::new(-1.2, 1.25, -0.35), Vec3::new(-0.6, 1.75, 0.25), Vec3::new(0.13, 0.13, 0.14));
    s.box_(Vec3::new(-1.05, 1.75, -0.2), Vec3::new(-0.75, 1.84, 0.1), Vec3::new(0.85, 0.83, 0.75));
    // round green stamp on the counter front
    s.cylinder(0.14, 1.05, 1.09, 10, Vec3::new(0.85, 0.15, 0.12));
    // price blackboard (A-frame) in front
    s.box_(Vec3::new(0.4, 0.0, -1.9), Vec3::new(1.7, 1.25, -1.82), Vec3::new(0.06, 0.06, 0.06));
    s.box_(Vec3::new(0.4, 0.0, -1.7), Vec3::new(1.7, 1.25, -1.62), Vec3::new(0.06, 0.06, 0.06));
    s.box_(Vec3::new(0.45, 0.15, -1.86), Vec3::new(1.65, 1.15, -1.66), Vec3::new(0.05, 0.05, 0.05));
    s.text3d("$0.022", Vec3::new(0.55, 0.62, -1.87), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.095, 0.02, Vec3::new(0.95, 0.92, 0.85));
    s.text3d("PER STRAND", Vec3::new(0.55, 0.35, -1.87), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 0.075, 0.02, Vec3::new(0.95, 0.92, 0.85));
    // bales stacked beside
    bale(&mut s, Vec3::new(-1.2, 0.0, -2.2), 0.0);
    bale(&mut s, Vec3::new(-1.2, 0.72, -2.2), 0.12);
    // intake hopper + fan unit at the back (belt feeds this)
    s.box_(Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.2, 0.9, 2.2), Vec3::new(0.55, 0.55, 0.58));
    s.funnel(0.55, 1.7, 0.16, 0.95, 10, Vec3::new(0.45, 0.46, 0.50));
    s.cylinder(0.12, 0.9, 1.7, 8, Vec3::new(0.35, 0.36, 0.40));
    append_mesh(mb, s, m);
}

/// A rectangular hay bale (tan box + darker strap lines).
fn bale(mb: &mut MeshBuilder, at: Vec3, yaw: f32) {
    let m = Mat4::from_rotation_y(yaw) * Mat4::from_translation(at);
    let mut s = MeshBuilder::new();
    let tan = Vec3::new(0.78, 0.62, 0.33);
    let strap = Vec3::new(0.45, 0.36, 0.20);
    s.box_(Vec3::new(-0.55, 0.0, -0.35), Vec3::new(0.55, 0.72, 0.35), tan);
    for x in [-0.3, 0.0, 0.3] {
        s.box_(Vec3::new(x - 0.035, 0.0, -0.36), Vec3::new(x + 0.035, 0.73, 0.36), strap);
    }
    append_mesh(mb, s, m);
}

/// Straight conveyor belt from `a` (low) to `b` (high) with legs + side rails.
fn conveyor(mb: &mut MeshBuilder, a: Vec3, b: Vec3) {
    let dir = (b - a).normalize();
    let len = (b - a).length();
    let right = dir.cross(Vec3::Y).normalize_or_zero();
    let steel = Vec3::new(0.42, 0.44, 0.48);
    // belt surface
    push_quad_4(mb, a - right * 0.35 + Vec3::Y * 0.0, a + right * 0.35, b + right * 0.35, b - right * 0.35, Vec3::new(0.22, 0.22, 0.23), FLAG_BELT);
    // side rails
    for s in [-1.0f32, 1.0] {
        let a2 = a + right * 0.42 * s;
        let b2 = b + right * 0.42 * s;
        let a3 = a2 + Vec3::new(0.0, 0.18, 0.0);
        let b3 = b2 + Vec3::new(0.0, 0.18, 0.0);
        push_quad_4(mb, a2, a3, b3, b2, steel, 0.0);
        // rail inner wall
        push_quad_4(mb, a2, b2, b2 - Vec3::new(0.0, 0.08, 0.0), a2 - Vec3::new(0.0, 0.08, 0.0), steel * 0.8, 0.0);
    }
    // legs every ~2.2m
    let n = (len / 2.2).ceil() as usize;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let p = a + (b - a) * t;
        let h = p.y;
        mb.box_(Vec3::new(p.x - 0.06, 0.0, p.z - 0.06), Vec3::new(p.x + 0.06, h - 0.05, p.z + 0.06), steel * 0.9);
        // foot
        mb.box_(Vec3::new(p.x - 0.14, 0.0, p.z - 0.14), Vec3::new(p.x + 0.14, 0.04, p.z + 0.14), steel * 0.7);
    }
}

/// Outdoor world: grass terrain ring, trees, scattered bales and small hay
/// piles, distant hills. All merged into ONE static mesh = one draw call.
pub fn outdoor_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    const X: f32 = 23.0;
    const Z: f32 = 17.0;

    // ---- grass around the sand apron: concentric square rings out to 700m ----
    let fx = X + 8.0;
    let fz = Z + 8.0;
    grass_rings(&mut mb, fx, fz, 700.0);

    // ---- trees (low poly: trunk + 2 cones) around the barn ----
    let mut rng = crate::rng::Rng::new(0xBEEF);
    for _ in 0..46 {
        let ang = rng.f32() * std::f32::consts::TAU;
        let dist = rng.f32b(42.0, 160.0);
        let x = ang.cos() * dist;
        let z = ang.sin() * dist;
        let s = rng.f32b(0.8, 1.5);
        tree(&mut mb, Vec3::new(x, 0.0, z), s, Vec3::new(0.24, 0.38, 0.16));
    }

    // ---- scattered hay piles + bales outside (like shot 1) ----
    for _ in 0..26 {
        let ang = rng.f32() * std::f32::consts::TAU;
        let dist = rng.f32b(36.0, 120.0);
        let x = ang.cos() * dist;
        let z = ang.sin() * dist;
        let s = rng.f32b(0.5, 1.4);
        let mut pile = MeshBuilder::new();
        pile.dome(2.2, 1.5, 10, 4, Vec3::new(0.62, 0.47, 0.22));
        append_mesh(&mut mb, pile, Mat4::from_scale(Vec3::new(s, s * 0.8, s)) * Mat4::from_translation(Vec3::new(x, 0.0, z)));
        if rng.bool_p(0.5) {
            bale(&mut mb, Vec3::new(x + 2.5, 0.0, z + 1.2), rng.f32() * 3.0);
        }
    }

    // ---- fence around the apron ----
    let fx = X + 8.0;
    let fz = Z + 8.0;
    let post = Vec3::new(0.45, 0.33, 0.20);
    let mut px = -fx;
    while px <= fx {
        mb.box_(Vec3::new(px - 0.07, 0.0, -fz - 0.07), Vec3::new(px + 0.07, 1.1, -fz + 0.07), post);
        mb.box_(Vec3::new(px - 0.07, 0.0, fz - 0.07), Vec3::new(px + 0.07, 1.1, fz + 0.07), post);
        px += 4.0;
    }
    let mut pz = -fz;
    while pz <= fz {
        mb.box_(Vec3::new(-fx - 0.07, 0.0, pz - 0.07), Vec3::new(-fx + 0.07, 1.1, pz + 0.07), post);
        mb.box_(Vec3::new(fx - 0.07, 0.0, pz - 0.07), Vec3::new(fx + 0.07, 1.1, pz + 0.07), post);
        pz += 4.0;
    }
    // rails
    for y in [0.45f32, 0.95] {
        mb.box_(Vec3::new(-fx, y, -fz + 0.0), Vec3::new(fx, y + 0.09, -fz + 0.09), post);
        mb.box_(Vec3::new(-fx, y, fz - 0.09), Vec3::new(fx, y + 0.09, fz), post);
        mb.box_(Vec3::new(-fx, y, -fz), Vec3::new(-fx + 0.09, y + 0.09, fz), post);
        mb.box_(Vec3::new(fx - 0.09, y, -fz), Vec3::new(fx, y + 0.09, fz), post);
    }

    (mb.verts, mb.idx)
}

/// Concentric square ground rings from the apron edge out to `outer`,
/// gently rising into hills. 8 quads per ring, vertex-color graded.
fn grass_rings(mb: &mut MeshBuilder, fx: f32, fz: f32, outer: f32) {
    const RINGS: usize = 14;
    const SUB: usize = 8; // quads per ring
    let mut r_prev = (fx, fz);
    for k in 0..RINGS {
        let t0 = k as f32 / RINGS as f32;
        let t1 = (k + 1) as f32 / RINGS as f32;
        let s0 = (fx + (outer - fx) * t0, fz + (outer - fz) * t0);
        let s1 = (fx + (outer - fx) * t1, fz + (outer - fz) * t1);
        let y0 = t0 * t0 * 26.0 + (t0 * 9.0).sin().abs() * 0.0;
        let y1 = t1 * t1 * 26.0;
        let col = Vec3::new(0.40, 0.52, 0.26) * (1.0 - 0.25 * t0) + Vec3::new(0.04, 0.03, 0.02) * t0;
        for i in 0..SUB {
            let a0 = i as f32 / SUB as f32;
            let a1 = (i + 1) as f32 / SUB as f32;
            // 8-point ring: corners + edge midpoints, unit square param
            let pt = |a: f32, sx: f32, sz: f32, y: f32| -> Vec3 {
                // map a in 0..1 around the square (8 segments: midpoints at .125 steps)
                let (u, v) = square_param(a);
                Vec3::new(u * sx, y, v * sz)
            };
            let p00 = pt(a0, r_prev.0, r_prev.1, y0);
            let p01 = pt(a1, r_prev.0, r_prev.1, y0);
            let p10 = pt(a0, s1.0, s1.1, y1);
            let p11 = pt(a1, s1.0, s1.1, y1);
            let n = Vec3::new(0.0, 1.0, 0.0);
            let va = mb.v(p00, n, col);
            let vb = mb.v(p01, n, col);
            let vc = mb.v(p11, n, col);
            let vd = mb.v(p10, n, col);
            mb.quad(va, vb, vc, vd);
        }
        r_prev = s1;
        let _ = s0;
    }
}

/// Map a in [0,1) to (u,v) walking the unit square perimeter through 8 points.
fn square_param(a: f32) -> (f32, f32) {
    // 8 anchor points: (-1,-1),(0,-1),(1,-1),(1,0),(1,1),(0,1),(-1,1),(-1,0)
    const P: [(f32, f32); 9] = [
        (-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0), (1.0, 1.0),
        (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0), (-1.0, -1.0),
    ];
    let t = a * 8.0;
    let i = (t as usize).min(7);
    let f = t - i as f32;
    let (u0, v0) = P[i];
    let (u1, v1) = P[i + 1];
    (u0 + (u1 - u0) * f, v0 + (v1 - v0) * f)
}



fn tree(mb: &mut MeshBuilder, at: Vec3, s: f32, leaf: Vec3) {
    let mut t = MeshBuilder::new();
    t.cylinder(0.22, 0.0, 2.2, 6, Vec3::new(0.35, 0.25, 0.15));
    t.cone(1.6, 1.4, 4.4, 7, leaf);
    t.cone(1.15, 2.6, 5.4, 7, leaf * 1.12);
    append_mesh(mb, t, Mat4::from_scale(Vec3::new(s, s, s)) * Mat4::from_translation(at));
}

/// First-person pitchfork: light wooden handle, 5 steel tines (like shot 4/5).
pub fn pitchfork_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let wood = Vec3::new(0.62, 0.48, 0.30);
    let wood_d = Vec3::new(0.50, 0.38, 0.22);
    let steel = Vec3::new(0.72, 0.74, 0.78);
    // handle along +Y, length ~1.15
    mb.cylinder(0.021, 0.0, 1.15, 8, wood);
    mb.cylinder(0.024, 0.0, 0.16, 8, wood_d); // grip
    // fork head at the top
    mb.box_(Vec3::new(-0.155, 1.10, -0.012), Vec3::new(0.155, 1.16, 0.012), steel);
    for i in 0..5 {
        let x = -0.14 + i as f32 * 0.07;
        let mut tine = MeshBuilder::new();
        tine.cylinder(0.007, 0.0, 0.34, 6, steel);
        append_mesh(&mut mb, tine, Mat4::from_translation(Vec3::new(x, 1.14, 0.0)));
    }
    (mb.verts, mb.idx)
}

/// Metal bucket with hay heap inside (shot 5).
pub fn bucket_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let metal = Vec3::new(0.70, 0.72, 0.74);
    // tapered bucket: rings
    for i in 0..8 {
        let t0 = i as f32 / 8.0;
        let t1 = (i + 1) as f32 / 8.0;
        let r0 = 0.16 - 0.03 * t0;
        let r1 = 0.16 - 0.03 * t1;
        let y0 = t0 * 0.30;
        let y1 = t1 * 0.30;
        let a0 = i as f32 / 8.0 * std::f32::consts::TAU;
        let _ = a0;
        // build ring segment
        let segs = 12;
        for k in 0..segs {
            let aa0 = k as f32 / segs as f32 * std::f32::consts::TAU;
            let aa1 = (k + 1) as f32 / segs as f32 * std::f32::consts::TAU;
            let p00 = Vec3::new(aa0.cos() * r0, y0, aa0.sin() * r0);
            let p01 = Vec3::new(aa1.cos() * r0, y0, aa1.sin() * r0);
            let p10 = Vec3::new(aa0.cos() * r1, y1, aa0.sin() * r1);
            let p11 = Vec3::new(aa1.cos() * r1, y1, aa1.sin() * r1);
            let n0 = Vec3::new(aa0.cos(), 0.25, aa0.sin()).normalize();
            let n1 = Vec3::new(aa1.cos(), 0.25, aa1.sin()).normalize();
            let a = mb.v(p00, n0, metal);
            let b = mb.v(p01, n1, metal);
            let c = mb.v(p11, n1, metal);
            let d = mb.v(p10, n0, metal);
            mb.quad(a, b, c, d);
        }
    }
    // bottom
    mb.ring_flat(0.0, 0.13, 0.005, 10, metal * 0.85, 0.0, true);
    // hay heap
    let mut heap = MeshBuilder::new();
    heap.dome(0.145, 0.10, 10, 3, Vec3::new(0.55, 0.42, 0.20));
    append_mesh(&mut mb, heap, Mat4::from_translation(Vec3::new(0.0, 0.20, 0.0)));
    (mb.verts, mb.idx)
}

/// Industrial robot arm (orange/black, KUKA-style, like shot 3).
pub fn robot_arm_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let orange = Vec3::new(0.85, 0.38, 0.08);
    let black = Vec3::new(0.09, 0.09, 0.10);
    // base
    mb.cylinder(0.35, 0.0, 0.22, 12, black);
    mb.cylinder(0.26, 0.22, 0.55, 12, orange);
    // shoulder
    mb.box_(Vec3::new(-0.22, 0.55, -0.22), Vec3::new(0.22, 0.95, 0.22), black);
    // upper arm (angled forward)
    let rot1 = Mat4::from_rotation_z(0.55);
    let mut up = MeshBuilder::new();
    up.box_(Vec3::new(-0.14, -0.12, -0.14), Vec3::new(0.14, 1.05, 0.14), orange);
    append_mesh(&mut mb, up, rot1 * Mat4::from_translation(Vec3::new(0.0, 0.85, 0.0)));
    // elbow
    let joint = rot1 * Mat4::from_translation(Vec3::new(0.0, 1.85, 0.0));
    let mut el = MeshBuilder::new();
    el.sphere(0.17, 10, 6, black);
    append_mesh(&mut mb, el, joint);
    // forearm angled down
    let rot2 = rot1 * Mat4::from_translation(Vec3::new(0.0, 1.85, 0.0)) * Mat4::from_rotation_z(-1.25);
    let mut fore = MeshBuilder::new();
    fore.box_(Vec3::new(-0.11, -0.10, -0.11), Vec3::new(0.11, 0.95, 0.11), orange);
    append_mesh(&mut mb, fore, rot2);
    // wrist + gripper
    let wrot = rot2 * Mat4::from_translation(Vec3::new(0.0, 0.95, 0.0));
    let mut wr = MeshBuilder::new();
    wr.cylinder(0.09, 0.0, 0.22, 8, black);
    wr.box_(Vec3::new(-0.05, 0.22, -0.05), Vec3::new(0.05, 0.42, 0.05), black);
    wr.box_(Vec3::new(-0.09, 0.30, -0.02), Vec3::new(-0.02, 0.44, 0.02), Vec3::new(0.2, 0.2, 0.22));
    wr.box_(Vec3::new(0.02, 0.30, -0.02), Vec3::new(0.09, 0.44, 0.02), Vec3::new(0.2, 0.2, 0.22));
    append_mesh(&mut mb, wr, wrot);
    (mb.verts, mb.idx)
}

/// Hay baler: gray machine with funnel intake + output chute (shot 3 style).
pub fn baler_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let gray = Vec3::new(0.62, 0.63, 0.66);
    let dark = Vec3::new(0.25, 0.26, 0.28);
    mb.box_(Vec3::new(-0.8, 0.25, -0.65), Vec3::new(0.8, 1.5, 0.65), gray);
    mb.box_(Vec3::new(-0.5, 1.5, -0.4), Vec3::new(0.5, 1.8, 0.4), dark);
    mb.funnel(0.5, 2.25, 0.18, 1.8, 10, dark);
    mb.cylinder(0.28, 0.0, 0.22, 10, dark);
    // output chute
    let mut ch = MeshBuilder::new();
    ch.tube(0.14, 0.0, 0.5, 8, gray);
    let m = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2 * 0.75) * Mat4::from_translation(Vec3::new(0.85, 1.1, 0.0));
    append_mesh(&mut mb, ch, m);
    // status LED
    mb.box_(Vec3::new(-0.1, 1.55, -0.42), Vec3::new(0.1, 1.63, -0.40), Vec3::new(0.1, 0.8, 0.15));
    (mb.verts, mb.idx)
}

/// Hay vacuum: big orange industrial unit with intake pipe.
pub fn vacuum_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let orange = Vec3::new(0.82, 0.40, 0.10);
    let dark = Vec3::new(0.22, 0.22, 0.24);
    mb.box_(Vec3::new(-1.0, 0.2, -0.7), Vec3::new(1.0, 1.9, 0.7), orange);
    mb.box_(Vec3::new(-0.35, 1.9, -0.35), Vec3::new(0.35, 2.3, 0.35), dark);
    mb.funnel(0.42, 2.75, 0.14, 2.3, 10, dark);
    mb.cylinder(0.10, 0.0, 2.6, 8, dark);
    (mb.verts, mb.idx)
}

/// Scanner arch frame (gray posts + top panel + green LED).
pub fn scanner_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let gray = Vec3::new(0.58, 0.60, 0.62);
    mb.box_(Vec3::new(-1.2, 0.0, -0.12), Vec3::new(-1.0, 2.4, 0.12), gray);
    mb.box_(Vec3::new(1.0, 0.0, -0.12), Vec3::new(1.2, 2.4, 0.12), gray);
    mb.box_(Vec3::new(-1.2, 2.4, -0.12), Vec3::new(1.2, 2.75, 0.12), gray);
    mb.box_(Vec3::new(-0.7, 2.05, -0.14), Vec3::new(0.7, 2.35, -0.12), Vec3::new(0.12, 0.12, 0.14));
    mb.box_(Vec3::new(-0.2, 2.10, -0.155), Vec3::new(0.2, 2.30, -0.14), Vec3::new(0.1, 0.85, 0.2));
    (mb.verts, mb.idx)
}

/// Handheld metal detector (black/yellow, like the reference photo).
pub fn detector_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let black = Vec3::new(0.07, 0.07, 0.07);
    let yellow = Vec3::new(0.92, 0.75, 0.10);
    mb.box_(Vec3::new(-0.025, 0.0, -0.018), Vec3::new(0.025, 0.14, 0.018), black); // grip
    mb.box_(Vec3::new(-0.022, 0.14, -0.016), Vec3::new(0.022, 0.30, 0.016), yellow); // body
    mb.box_(Vec3::new(-0.010, 0.30, -0.008), Vec3::new(0.010, 0.42, 0.008), black); // neck
    // flat round search head at the top, tilted
    let mut head = MeshBuilder::new();
    head.cylinder(0.075, -0.015, 0.015, 14, black);
    head.cylinder(0.052, -0.017, 0.017, 14, yellow);
    let m = Mat4::from_translation(Vec3::new(0.0, 0.50, 0.0)) * Mat4::from_rotation_x(1.35);
    append_mesh(&mut mb, head, m);
    (mb.verts, mb.idx)
}

/// Barn perimeter belt loop (x, z), closed. All points outside the pile dome
/// (r >= 16.2) and inside the walls (x 23 / z 17). Matches shot 3's network.
pub const BELT_RING: [(f32, f32); 17] = [
    (18.5, 3.0), (18.0, -2.0), (16.5, -7.0), (14.0, -12.0), (8.0, -16.0),
    (0.0, -16.4), (-8.0, -16.0), (-14.0, -12.5), (-18.0, -7.0), (-20.0, 0.0),
    (-18.5, 7.0), (-14.0, 12.5), (-8.0, 15.8), (0.0, 16.2), (8.0, 15.8),
    (14.0, 12.0), (17.5, 7.0),
];
/// Dead-end spur belts branching off the ring (open paths).
pub const BELT_SPURS: [&[(f32, f32)]; 3] = [
    &[(-18.0, -7.0), (-17.5, -10.5), (-16.0, -13.5)],
    &[(14.0, -12.0), (16.0, -14.2), (19.5, -15.3)],
    &[(-8.0, 15.8), (-11.0, 13.8), (-13.5, 10.5)],
];
pub const BELT_H: f32 = 0.52;
const BELT_W: f32 = 0.44; // half width

fn stamped(mb: &mut MeshBuilder, part: MeshBuilder, at: Vec3, yaw: f32) {
    let m = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw);
    append_mesh(mb, part, m);
}

/// One straight belt segment from a to b (floor plan), top at BELT_H:
/// dark rubber slab w/ animated cleats (flag 0.993), light side rails, legs.
fn belt_seg(mb: &mut MeshBuilder, a: (f32, f32), b: (f32, f32)) {
    let (ax, az) = a;
    let (bx, bz) = b;
    let dx = bx - ax;
    let dz = bz - az;
    let len = (dx * dx + dz * dz).sqrt();
    if len < 0.05 {
        return;
    }
    let yaw = -dz.atan2(dx);
    let mut part = MeshBuilder::new();
    let rubber = Vec3::new(0.15, 0.15, 0.16);
    let rail = Vec3::new(0.60, 0.61, 0.64);
    let legc = Vec3::new(0.22, 0.22, 0.24);
    part.boxf(
        Vec3::new(-0.06, BELT_H - 0.10, -BELT_W),
        Vec3::new(len + 0.06, BELT_H, BELT_W),
        rubber,
        0.993,
    );
    // light edge rails (shot 3: gray belts with pale borders)
    part.boxf(
        Vec3::new(-0.06, BELT_H - 0.15, -BELT_W - 0.07),
        Vec3::new(len + 0.06, BELT_H - 0.05, -BELT_W + 0.01),
        rail,
        0.0,
    );
    part.boxf(
        Vec3::new(-0.06, BELT_H - 0.15, BELT_W - 0.01),
        Vec3::new(len + 0.06, BELT_H - 0.05, BELT_W + 0.07),
        rail,
        0.0,
    );
    let n_legs = (len / 2.2) as usize + 1;
    for i in 0..=n_legs {
        let x = 0.35 + i as f32 * (len - 0.7) / n_legs as f32;
        part.boxf(
            Vec3::new(x - 0.045, 0.0, -0.26),
            Vec3::new(x + 0.045, BELT_H - 0.10, 0.26),
            legc,
            0.0,
        );
    }
    stamped(mb, part, Vec3::new(ax, 0.0, az), yaw);
}

/// Sagging wire between two anchor points (thin square-section catenary).
fn wire(mb: &mut MeshBuilder, a: Vec3, b: Vec3, sag: f32, col: Vec3) {
    let segs = 6;
    let mut prev_top: [Vec3; 4] = [Vec3::ZERO; 4];
    for i in 0..=segs {
        let t = i as f32 / segs as f32;
        let p = a.lerp(b, t) - Vec3::new(0.0, sag * 4.0 * t * (1.0 - t), 0.0);
        let r = 0.022;
        let corners = [
            Vec3::new(p.x - r, p.y, p.z - r),
            Vec3::new(p.x + r, p.y, p.z - r),
            Vec3::new(p.x + r, p.y, p.z + r),
            Vec3::new(p.x - r, p.y, p.z + r),
        ];
        if i > 0 {
            for k in 0..4 {
                let k2 = (k + 1) & 3;
                let va = mb.v(prev_top[k], Vec3::ZERO, col);
                let vb = mb.v(prev_top[k2], Vec3::ZERO, col);
                let vc = mb.v(corners[k2], Vec3::ZERO, col);
                let vd = mb.v(corners[k], Vec3::ZERO, col);
                mb.quad(va, vb, vc, vd);
            }
        }
        prev_top = corners;
    }
}

/// ALL the yard machines, baked into the static world mesh (still 1 draw call):
/// perimeter + spur conveyor network, power poles w/ wires, steam boiler,
/// tube launcher, water trough, sprinkler, market stand, wooden crates.
pub fn machines_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();

    // ---- conveyor network ----
    for i in 0..BELT_RING.len() {
        let a = BELT_RING[i];
        let b = BELT_RING[(i + 1) % BELT_RING.len()];
        belt_seg(&mut mb, a, b);
        // corner platform so turns look continuous
        let mut c = MeshBuilder::new();
        c.boxf(
            Vec3::new(-BELT_W, BELT_H - 0.10, -BELT_W),
            Vec3::new(BELT_W, BELT_H, BELT_W),
            Vec3::new(0.15, 0.15, 0.16),
            0.993,
        );
        stamped(&mut mb, c, Vec3::new(a.0, 0.0, a.1), 0.0);
    }
    for spur in BELT_SPURS {
        for w in spur.windows(2) {
            belt_seg(&mut mb, (w[0].0, w[0].1), (w[1].0, w[1].1));
        }
    }

    // ---- power poles + wires (like the lamp posts ringing shot 3) ----
    let poles: [(f32, f32); 10] = [
        (-21.0, -13.0), (-21.8, -5.0), (-21.5, 3.0), (-19.0, 8.0), (-20.5, 11.5),
        (5.5, 16.6), (16.0, 15.9), (21.3, 9.0), (21.9, 0.0), (20.5, -8.5),
    ];
    let wood = Vec3::new(0.30, 0.24, 0.17);
    let steel = Vec3::new(0.35, 0.36, 0.38);
    for &(px, pz) in poles.iter() {
        let mut p = MeshBuilder::new();
        p.boxf(Vec3::new(-0.09, 0.0, -0.09), Vec3::new(0.09, 3.4, 0.09), wood, 0.995);
        p.boxf(Vec3::new(-0.55, 3.05, -0.05), Vec3::new(0.55, 3.17, 0.05), wood, 0.995);
        // insulators + lamp head
        p.boxf(Vec3::new(-0.42, 3.17, -0.05), Vec3::new(-0.34, 3.30, 0.05), steel, 0.0);
        p.boxf(Vec3::new(0.34, 3.17, -0.05), Vec3::new(0.42, 3.30, 0.05), steel, 0.0);
        p.boxf(Vec3::new(-0.14, 3.28, -0.14), Vec3::new(0.14, 3.44, 0.14), steel, 0.0);
        p.boxf(Vec3::new(-0.10, 3.20, -0.10), Vec3::new(0.10, 3.28, 0.10), Vec3::new(0.95, 0.90, 0.70), 0.91);
        stamped(&mut mb, p, Vec3::new(px, 0.0, pz), 0.0);
    }
    for w in poles.windows(2) {
        let a = Vec3::new(w[0].0 + 0.40, 3.22, w[0].1);
        let b = Vec3::new(w[1].0 - 0.40, 3.22, w[1].1);
        wire(&mut mb, a, b, a.distance(b) * 0.06, Vec3::new(0.08, 0.08, 0.09));
        let a2 = Vec3::new(w[0].0 - 0.40, 3.22, w[0].1);
        let b2 = Vec3::new(w[1].0 + 0.40, 3.22, w[1].1);
        wire(&mut mb, a2, b2, a2.distance(b2) * 0.06, Vec3::new(0.08, 0.08, 0.09));
    }

    // ---- steam boiler (west wall): horizontal tank + chimney + glowing firebox
    {
        let steel = Vec3::new(0.45, 0.47, 0.50);
        let dark = Vec3::new(0.20, 0.20, 0.22);
        let mut b = MeshBuilder::new();
        // tank: vertical cylinder + sphere caps, laid along Z
        let mut tank = MeshBuilder::new();
        tank.cylinder(0.85, -1.2, 1.2, 12, steel);
        let mut cap_lo = MeshBuilder::new();
        cap_lo.sphere(0.85, 12, 5, steel);
        append_mesh(&mut tank, cap_lo, Mat4::from_translation(Vec3::new(0.0, -1.2, 0.0)));
        let mut cap_hi = MeshBuilder::new();
        cap_hi.sphere(0.85, 12, 5, steel);
        append_mesh(&mut tank, cap_hi, Mat4::from_translation(Vec3::new(0.0, 1.2, 0.0)));
        // lay it down: rotate about Z so the axis runs along X, then lift onto legs
        let laid = Mat4::from_translation(Vec3::new(0.0, 1.30, 0.0))
            * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        append_mesh(&mut b, tank, laid);
        // legs
        b.boxf(Vec3::new(-0.6, 0.0, -0.55), Vec3::new(-0.42, 0.55, 0.55), dark, 0.0);
        b.boxf(Vec3::new(0.42, 0.0, -0.55), Vec3::new(0.6, 0.55, 0.55), dark, 0.0);
        // firebox with warm glow (LED red flag => flicker)
        b.boxf(Vec3::new(-0.55, 0.0, -0.55), Vec3::new(0.55, 0.62, 0.55), dark, 0.0);
        b.boxf(Vec3::new(-0.28, 0.12, -0.62), Vec3::new(0.28, 0.55, -0.56), Vec3::new(1.0, 0.45, 0.10), 0.90);
        stamped(&mut mb, b, Vec3::new(-21.6, 0.0, -11.0), std::f32::consts::FRAC_PI_2);
        // chimney at the rear end
        let mut ch = MeshBuilder::new();
        ch.cylinder(0.32, 0.0, 0.3, 10, dark);
        ch.cylinder(0.26, 0.3, 4.3, 10, dark);
        stamped(&mut mb, ch, Vec3::new(-21.6, 2.1, -13.3), 0.0);
    }

    // ---- tube launcher (NW): big angled tube aimed at the pile ----
    {
        let mut t = MeshBuilder::new();
        let gray = Vec3::new(0.52, 0.54, 0.57);
        let dark = Vec3::new(0.18, 0.18, 0.20);
        t.tube(0.52, 0.0, 4.6, 12, gray);
        // muzzle ring + bands
        t.tube(0.60, 4.3, 4.6, 12, dark);
        t.tube(0.58, 1.4, 1.75, 12, dark);
        t.tube(0.58, 2.9, 3.25, 12, dark);
        // A-frame support
        t.boxf(Vec3::new(-0.9, 0.0, 0.0), Vec3::new(-0.75, 2.2, 0.12), dark, 0.0);
        t.boxf(Vec3::new(0.75, 0.0, 0.0), Vec3::new(0.9, 2.2, 0.12), dark, 0.0);
        // tilt the whole tube 32 deg toward +X, then place
        let mut tilted = MeshBuilder::new();
        append_mesh(
            &mut tilted,
            t,
            Mat4::from_rotation_z(0.56) * Mat4::from_translation(Vec3::new(0.0, 0.35, 0.0)),
        );
        stamped(&mut mb, tilted, Vec3::new(-19.8, 0.0, 14.8), 0.9);
    }

    // ---- hay wrapper (west, near belt A) is a dynamic prop; add its pad ----
    {
        let mut pad = MeshBuilder::new();
        pad.boxf(Vec3::new(-1.3, 0.0, -1.0), Vec3::new(1.3, 0.08, 1.0), Vec3::new(0.30, 0.30, 0.32), 0.0);
        stamped(&mut mb, pad, Vec3::new(-19.5, 0.0, -6.0), 0.0);
    }

    // ---- water trough (north wall) ----
    {
        let mut tr = MeshBuilder::new();
        let wood = Vec3::new(0.42, 0.33, 0.22);
        tr.boxf(Vec3::new(-3.0, 0.0, -0.4), Vec3::new(3.0, 0.55, 0.4), wood, 0.995);
        tr.boxf(Vec3::new(-2.85, 0.42, -0.30), Vec3::new(2.85, 0.48, 0.30), Vec3::new(0.30, 0.52, 0.62), 0.0);
        stamped(&mut mb, tr, Vec3::new(-11.5, 0.0, 16.3), 0.0);
    }

    // ---- sprinkler tripod (south-east) ----
    {
        let mut s = MeshBuilder::new();
        let dark = Vec3::new(0.25, 0.26, 0.28);
        for i in 0..3 {
            let a = i as f32 * std::f32::consts::TAU / 3.0;
            let mut leg = MeshBuilder::new();
            leg.boxf(
                Vec3::new(-0.03, 0.0, -0.03),
                Vec3::new(0.03, 0.75, 0.03),
                dark,
                0.0,
            );
            let m = Mat4::from_translation(Vec3::new(a.cos() * 0.16, 0.0, a.sin() * 0.16))
                * Mat4::from_rotation_z(a.cos() * 0.35)
                * Mat4::from_rotation_x(-a.sin() * 0.35);
            append_mesh(&mut s, leg, m);
        }
        s.cylinder(0.05, 0.70, 0.95, 8, dark);
        s.cylinder(0.10, 0.95, 1.02, 8, Vec3::new(0.65, 0.68, 0.70));
        stamped(&mut mb, s, Vec3::new(9.5, 0.0, -14.2), 0.0);
    }

    // ---- market stand (NW corner, faces the pile): posts + striped awning ----
    {
        let mut st = MeshBuilder::new();
        let wood = Vec3::new(0.46, 0.36, 0.24);
        for sx in [-1.15f32, 1.15] {
            for sz in [-0.75f32, 0.75] {
                st.boxf(
                    Vec3::new(sx - 0.05, 0.0, sz - 0.05),
                    Vec3::new(sx + 0.05, 2.15, sz + 0.05),
                    wood,
                    0.995,
                );
            }
        }
        // counter
        st.boxf(Vec3::new(-1.2, 0.85, -0.85), Vec3::new(1.2, 1.0, 0.85), wood, 0.995);
        // awning: alternating red/cream stripes
        for i in 0..6 {
            let x0 = -1.35 + i as f32 * 0.45;
            let col = if i % 2 == 0 {
                Vec3::new(0.72, 0.16, 0.13)
            } else {
                Vec3::new(0.90, 0.86, 0.76)
            };
            st.boxf(Vec3::new(x0, 2.15, -0.95), Vec3::new(x0 + 0.45, 2.23, 1.05), col, 0.0);
        }
        st.text3d(
            "FRESH HAY",
            Vec3::new(-0.85, 1.30, -0.86),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            0.16,
            0.02,
            Vec3::new(0.14, 0.10, 0.06),
        );
        stamped(&mut mb, st, Vec3::new(-16.0, 0.0, 14.0), 0.6);
    }

    // ---- wooden crates (scattered clusters, like shot 3) ----
    let crates: [(f32, f32, f32); 6] = [
        (-21.2, 14.8, 0.3),
        (-20.2, 14.2, 0.9),
        (16.8, -13.8, 0.5),
        (18.6, -12.4, 0.15),
        (19.8, -14.2, 0.7),
        (-20.8, 5.2, 0.2),
    ];
    for &(cx, cz, yaw) in crates.iter() {
        let mut c = MeshBuilder::new();
        c.boxf(Vec3::new(-0.55, 0.0, -0.55), Vec3::new(0.55, 1.0, 0.55), Vec3::new(0.48, 0.37, 0.23), 0.995);
        c.boxf(Vec3::new(-0.57, 0.42, -0.57), Vec3::new(0.57, 0.56, 0.57), Vec3::new(0.40, 0.30, 0.18), 0.995);
        stamped(&mut mb, c, Vec3::new(cx, 0.0, cz), yaw);
    }

    (mb.verts, mb.idx)
}

/// Hay wrapper: gray machine with a big horizontal rotating drum + rollers.
pub fn wrapper_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let gray = Vec3::new(0.58, 0.59, 0.62);
    let dark = Vec3::new(0.22, 0.22, 0.24);
    mb.box_(Vec3::new(-1.1, 0.25, -0.7), Vec3::new(1.1, 0.9, 0.7), gray);
    // big horizontal drum
    let mut drum = MeshBuilder::new();
    drum.cylinder(0.52, -0.75, 0.75, 12, dark);
    let m = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2) * Mat4::from_translation(Vec3::new(0.0, 1.35, 0.0));
    append_mesh(&mut mb, drum, m);
    // feed ramp + rollers
    mb.funnel(0.35, 1.15, 0.55, 0.62, 8, dark);
    mb.cylinder(0.12, 0.15, 0.35, 8, dark);
    mb.cylinder(0.10, 0.0, 0.25, 8, dark);
    // status LED
    mb.boxf(Vec3::new(-0.10, 0.95, -0.72), Vec3::new(0.10, 1.05, -0.70), Vec3::new(0.1, 0.85, 0.2), 0.91);
    (mb.verts, mb.idx)
}

/// Scout drone: quadcopter body + 4 rotor discs + camera dot.
pub fn drone_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let dark = Vec3::new(0.13, 0.13, 0.15);
    let gray = Vec3::new(0.50, 0.52, 0.55);
    mb.box_(Vec3::new(-0.22, -0.07, -0.22), Vec3::new(0.22, 0.10, 0.22), dark);
    mb.box_(Vec3::new(-0.10, 0.10, -0.10), Vec3::new(0.10, 0.16, 0.10), gray);
    // camera gimbal
    mb.sphere(0.05, 8, 4, Vec3::new(0.05, 0.05, 0.06));
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            mb.box_(
                Vec3::new(sx * 0.22 - 0.025 * sx, 0.02, sz * 0.22 - 0.025 * sz),
                Vec3::new(sx * 0.42 + 0.025 * sx, 0.05, sz * 0.42 + 0.025 * sz),
                dark,
            );
            // rotor disc (thin cylinder)
            let mut rotor = MeshBuilder::new();
            rotor.cylinder(0.24, -0.006, 0.006, 10, Vec3::new(0.30, 0.30, 0.33));
            append_mesh(
                &mut mb,
                rotor,
                Mat4::from_translation(Vec3::new(sx * 0.42, 0.06, sz * 0.42)),
            );
        }
    }
    (mb.verts, mb.idx)
}

/// All prop meshes, indexed:
/// 0 haystack core dome, 1 needle, 2 detector, 3..12 valuables,
/// 13 baler, 14 vacuum, 15 pitchfork, 16 bucket, 17 robot arm, 18 scanner,
/// 19 hay ball (belt), 20 loose hay clump (pile crumbs), 21 hay wrapper,
/// 22 scout drone.
pub fn build_all_meshes() -> Vec<(Vec<Vertex>, Vec<u16>)> {
    let mut out: Vec<(Vec<Vertex>, Vec<u16>)> = Vec::new();

    // 0: haystack dark core (semi-ellipsoid slightly inside the straws)
    let mut mb = MeshBuilder::new();
    mb.dome(0.97, 0.97, 40, 10, Vec3::new(0.34, 0.25, 0.12));
    out.push(mb.build());

    // 1: needle
    out.push(needle_mesh());

    // 2: handheld metal detector
    out.push(detector_mesh());

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
    mb.box_(Vec3::new(0.0, 0.0, -0.0012), Vec3::new(0.045, 0.0024, 0.0012), Vec3::new(0.60, 0.45, 0.25));
    mb.box_(Vec3::new(-0.008, 0.0, -0.006), Vec3::new(0.004, 0.0024, 0.008), Vec3::new(0.60, 0.45, 0.25));
    out.push(mb.build());
    // 7: spoon
    let mut mb = MeshBuilder::new();
    mb.box_(Vec3::new(0.0, 0.0, -0.0025), Vec3::new(0.075, 0.002, 0.0025), Vec3::new(0.78, 0.80, 0.83));
    mb.sphere(0.010, 8, 4, Vec3::new(0.78, 0.80, 0.83));
    out.push(mb.build());
    // 8: fork
    let mut mb = MeshBuilder::new();
    mb.box_(Vec3::new(0.0, 0.0, -0.0022), Vec3::new(0.070, 0.002, 0.0022), Vec3::new(0.78, 0.80, 0.83));
    for k in 0..4 {
        let z = -0.0066 + k as f32 * 0.0044;
        mb.box_(Vec3::new(0.070, 0.0, z - 0.0008), Vec3::new(0.012, 0.002, z + 0.0008), Vec3::new(0.78, 0.80, 0.83));
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

    // 13: baler
    out.push(baler_mesh());
    // 14: vacuum
    out.push(vacuum_mesh());
    // 15: pitchfork (viewmodel)
    out.push(pitchfork_mesh());
    // 16: bucket (viewmodel)
    out.push(bucket_mesh());
    // 17: robot arm
    out.push(robot_arm_mesh());
    // 18: scanner arch
    out.push(scanner_mesh());
    // 19: hay ball (belt cargo)
    let mut mb = MeshBuilder::new();
    mb.sphere(0.16, 10, 6, Vec3::new(0.85, 0.55, 0.24));
    out.push(mb.build());
    // 20: loose hay clump
    let mut mb = MeshBuilder::new();
    mb.dome(0.10, 0.07, 7, 3, Vec3::new(0.60, 0.46, 0.22));
    out.push(mb.build());
    // 21: hay wrapper
    out.push(wrapper_mesh());
    // 22: scout drone
    out.push(drone_mesh());
    // 23: mechanical sorter (rotating drum on a frame + control screen)
    out.push(sorter_mesh());
    // 24: vertical elevator tower (tall belt with cleats, like shot 3)
    out.push(elevator_mesh());
    // 25: sale truck (tractor + flat trailer waiting at the gate)
    out.push(truck_mesh());
    // 26: the silo (big cylinder + dome + ladder)
    out.push(silo_mesh());
    // 27: eco brick press (hopper + press head + brick chute)
    out.push(brick_press_mesh());

    out
}

/// Mechanical sorter: dark frame, horizontal drum with ribs, small control
/// screen with a green LED. Sits at belt junctions.
fn sorter_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let steel = Vec3::new(0.35, 0.38, 0.40);
    let dark = Vec3::new(0.22, 0.24, 0.26);
    // frame legs + deck
    for (x, z) in [(-0.5, -0.35), (0.5, -0.35), (-0.5, 0.35), (0.5, 0.35)] {
        mb.box_(
            Vec3::new(x - 0.05, 0.0, z - 0.05),
            Vec3::new(x + 0.05, 0.9, z + 0.05),
            dark,
        );
    }
    mb.box_(Vec3::new(-0.62, 0.9, -0.45), Vec3::new(0.62, 1.02, 0.45), steel);
    // ribbed drum (inline cylinder laid horizontally via stacked boxes)
    let drum_col = Vec3::new(0.55, 0.48, 0.34);
    for i in 0..7 {
        let t = i as f32 / 6.0;
        let x = -0.42 + t * 0.84;
        let rib = if i % 2 == 0 { 0.30 } else { 0.26 };
        mb.box_(
            Vec3::new(x - 0.05, 1.02 - rib * 0.5, -rib),
            Vec3::new(x + 0.05, 1.02 + rib * 0.5, rib),
            drum_col,
        );
    }
    // feed hopper on the back
    mb.box_(Vec3::new(-0.55, 1.02, -0.45), Vec3::new(0.10, 1.42, -0.05), steel);
    // control screen + green LED
    mb.box_(Vec3::new(0.30, 1.02, 0.45), Vec3::new(0.55, 1.34, 0.50), dark);
    mb.box_(Vec3::new(0.44, 1.22, 0.505), Vec3::new(0.52, 1.30, 0.51), Vec3::new(0.1, 0.9, 0.3));
    mb.build()
}

/// Vertical elevator tower: tall dark trunk with a light belt strip up the
/// front (FLAG_BELT so the cleats animate), platform at the top. ~7 m tall.
fn elevator_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let dark = Vec3::new(0.18, 0.19, 0.21);
    let steel = Vec3::new(0.38, 0.40, 0.43);
    // 4 legs splaying slightly
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        mb.box_(
            Vec3::new(sx * 0.34 - 0.06, 0.0, sz * 0.34 - 0.06),
            Vec3::new(sx * 0.22 + 0.06, 6.6, sz * 0.22 + 0.06),
            dark,
        );
    }
    // trunk
    mb.box_(Vec3::new(-0.42, 0.4, -0.42), Vec3::new(0.42, 6.6, 0.42), dark);
    // belt strip up the front face (animated cleats)
    mb.boxf(
        Vec3::new(-0.30, 0.5, 0.425),
        Vec3::new(0.30, 6.5, 0.44),
        Vec3::new(0.25, 0.25, 0.27),
        FLAG_BELT,
    );
    // cross braces
    for y in [1.6f32, 3.4, 5.2] {
        mb.box_(Vec3::new(-0.48, y, -0.48), Vec3::new(0.48, y + 0.06, 0.48), steel);
    }
    // top platform + chute
    mb.box_(Vec3::new(-0.55, 6.6, -0.55), Vec3::new(0.55, 6.78, 0.55), steel);
    mb.box_(Vec3::new(-0.10, 5.4, 0.44), Vec3::new(0.34, 5.6, 1.05), steel);
    mb.build()
}

/// Sale truck parked by the gate: brown cab + flat trailer with side boards.
fn truck_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let body = Vec3::new(0.42, 0.30, 0.18);
    let dark = Vec3::new(0.20, 0.16, 0.12);
    let tyre = Vec3::new(0.08, 0.08, 0.09);
    // chassis
    mb.box_(Vec3::new(-1.6, 0.45, -0.42), Vec3::new(1.6, 0.62, 0.42), dark);
    // cab (front = +x)
    mb.box_(Vec3::new(0.75, 0.62, -0.40), Vec3::new(1.55, 1.35, 0.40), body);
    mb.box_(Vec3::new(1.30, 0.95, -0.33), Vec3::new(1.52, 1.22, 0.33), Vec3::new(0.55, 0.65, 0.70));
    // flat trailer with boards
    mb.box_(Vec3::new(-1.55, 0.62, -0.45), Vec3::new(0.70, 0.78, 0.45), body);
    for z in [-0.45f32, 0.37] {
        mb.box_(Vec3::new(-1.55, 0.78, z), Vec3::new(0.70, 1.10, z + 0.08), dark);
    }
    // wheels (box hubs - cheap and readable at gameplay distance)
    for (x, z) in [(1.15f32, -0.45f32), (1.15, 0.45), (-0.9, -0.45), (-0.9, 0.45), (-1.3, -0.45), (-1.3, 0.45)] {
        mb.box_(
            Vec3::new(x - 0.20, 0.06, z - 0.10),
            Vec3::new(x + 0.20, 0.46, z + 0.10),
            tyre,
        );
    }
    mb.build()
}

/// The Silo: big pale cylinder with a dome top, ladder rail, band rings.
fn silo_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let wall = Vec3::new(0.72, 0.70, 0.64);
    let band = Vec3::new(0.45, 0.42, 0.38);
    mb.cylinder(1.5, 0.0, 5.2, 16, wall);
    // band rings (slightly wider short cylinders)
    for y in [0.9f32, 2.4, 3.9] {
        mb.cylinder(1.54, y, y + 0.10, 16, band);
    }
    mb.dome(1.5, 0.8, 16, 5, band);
    // legs
    for k in 0..4 {
        let a = k as f32 * std::f32::consts::FRAC_PI_2;
        let (x, z) = (a.cos() * 1.1, a.sin() * 1.1);
        mb.box_(Vec3::new(x - 0.08, 0.0, z - 0.08), Vec3::new(x + 0.08, 1.0, z + 0.08), band);
    }
    // ladder
    mb.box_(Vec3::new(1.42, 0.0, -0.05), Vec3::new(1.48, 4.8, 0.05), band);
    mb.box_(Vec3::new(1.42, 0.0, 0.30), Vec3::new(1.48, 4.8, 0.40), band);
    for k in 0..8 {
        let y = 0.5 + k as f32 * 0.55;
        mb.box_(Vec3::new(1.42, y, -0.05), Vec3::new(1.48, y + 0.05, 0.40), band);
    }
    mb.build()
}

/// Eco brick press: hopper on top, press box, brick chute with green LED.
fn brick_press_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let mut mb = MeshBuilder::new();
    let steel = Vec3::new(0.40, 0.36, 0.30);
    let dark = Vec3::new(0.24, 0.22, 0.19);
    // base + press box
    mb.box_(Vec3::new(-0.55, 0.0, -0.40), Vec3::new(0.55, 0.75, 0.40), steel);
    mb.box_(Vec3::new(-0.40, 0.75, -0.30), Vec3::new(0.40, 1.15, 0.30), dark);
    // hopper (inverted pyramid faked with two boxes)
    mb.box_(Vec3::new(-0.45, 1.15, -0.35), Vec3::new(0.45, 1.55, 0.35), steel);
    // chute out the front
    mb.box_(Vec3::new(0.40, 0.30, -0.16), Vec3::new(0.95, 0.48, 0.16), steel);
    // bricks on the chute
    mb.box_(Vec3::new(0.55, 0.48, -0.12), Vec3::new(0.78, 0.60, 0.12), Vec3::new(0.70, 0.52, 0.26));
    // green LED
    mb.box_(Vec3::new(-0.50, 0.80, 0.405), Vec3::new(-0.42, 0.88, 0.415), Vec3::new(0.1, 0.9, 0.3));
    mb.build()
}


