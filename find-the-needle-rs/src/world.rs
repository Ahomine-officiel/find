// World generation: the haystack, the needle, the valuables, and the dig logic.
use crate::mesh::ValuableKind;
use crate::rng::Rng;
use bytemuck::Pod;
use bytemuck::Zeroable;
use glam::Vec3;

pub const PILE_R: f32 = 16.0;
pub const PILE_H: f32 = 11.0;
pub const PILE_CENTER: Vec3 = Vec3::new(0.0, 0.0, 0.0);

/// GPU straw instance: 32 bytes.
/// pos = (x, y, z, shade). data = (packed_rot_len, color_var, state, kind).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct StrawInst {
    pub pos: [f32; 4],
    pub data: [u32; 4],
}

pub const STRAW_KIND_PILE: u32 = 0;
pub const STRAW_KIND_TOSS: u32 = 1;
pub const STRAW_KIND_FLY: u32 = 2;

pub struct Valuable {
    pub pos: Vec3,
    pub yaw: f32,
    pub kind: ValuableKind,
    pub taken: bool,
}

pub struct Needle {
    pub pos: Vec3,
    pub yaw: f32,
    pub found: bool,
}

struct Grid {
    cell: f32,
    dim_x: usize,
    dim_y: usize,
    dim_z: usize,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new() -> Self {
        let cell = 1.5f32;
        let dim_x = ((PILE_R * 2.0 + 4.0) / cell) as usize + 1;
        let dim_y = ((PILE_H + 2.0) / cell) as usize + 1;
        let dim_z = dim_x;
        let n = dim_x * dim_y * dim_z;
        Grid { cell, dim_x, dim_y, dim_z, cells: vec![Vec::new(); n] }
    }

    #[inline]
    fn idx(&self, cx: usize, cy: usize, cz: usize) -> usize {
        cx + self.dim_x * (cy + self.dim_y * cz)
    }

    fn insert(&mut self, p: Vec3, i: u32) {
        let x = ((p.x + PILE_R + 2.0) / self.cell) as usize;
        let y = ((p.y + 1.0) / self.cell) as usize;
        let z = ((p.z + PILE_R + 2.0) / self.cell) as usize;
        let x = x.min(self.dim_x - 1);
        let y = y.min(self.dim_y - 1);
        let z = z.min(self.dim_z - 1);
        let ci = self.idx(x, y, z);
        self.cells[ci].push(i);
    }
}

pub struct World {
    pub seed: u64,
    pub straw_count: u32,
    pub straws: Vec<StrawInst>,
    pub removed: Vec<u64>,
    pub removed_count: u32,
    pub needle: Needle,
    pub valuables: Vec<Valuable>,
    grid: Grid,
    pending_removals: Vec<u32>,
}

fn pack_straw(yaw: f32, pitch: f32, len: f32) -> u32 {
    let yb = (((yaw.rem_euclid(std::f32::consts::TAU)) / std::f32::consts::TAU) * 4095.0) as u32 & 0xFFF;
    let pn = (pitch / std::f32::consts::FRAC_PI_2 * 0.5 + 0.5).clamp(0.0, 1.0); // -90..90 -> 0..1
    let pb = ((pn * 1023.0) as u32 & 0x3FF) << 12;
    let ln = ((len - 0.28) / 0.32).clamp(0.0, 1.0);
    let lb = (((ln * 63.0) as u32) & 0x3F) << 22;
    yb | pb | lb
}

impl World {
    pub fn new(seed: u64, straw_count: u32) -> Self {
        let mut rng = Rng::new(seed);
        let mut straws: Vec<StrawInst> = Vec::with_capacity(straw_count as usize);
        let mut grid = Grid::new();

        for i in 0..straw_count {
            let shell = rng.bool_p(0.62);
            let p = loop {
                let x = rng.f32b(-PILE_R, PILE_R);
                let z = rng.f32b(-PILE_R, PILE_R);
                let y = rng.f32b(0.05, PILE_H);
                let u = (x / PILE_R).powi(2) + (y / PILE_H).powi(2) + (z / PILE_R).powi(2);
                if u > 1.0 {
                    continue;
                }
                let u = if shell {
                    // concentrate near the surface (shell band ~2m deep)
                    let k = rng.f32() * 0.35;
                    1.0 - (1.0 - u).max(0.38) * k
                } else {
                    u
                };
                let u = u.clamp(0.05, 1.0);
                break (x * (u / u).sqrt(), y, z, u); // (x,y,z) kept, u carries depth
            };
            let (x, y, z, u) = p;
            let mut y = y;
            if y < 0.02 {
                y = 0.02;
            }
            let mut px = x;
            let mut pz = z;
            // surface straws poke outward past the ellipsoid -> fuzzy jagged rim
            if shell && u > 0.8 {
                let radial = (x * x + z * z).sqrt().max(1e-4);
                let out = rng.f32b(0.0, 0.45) * (1.0 - (1.0 - u) * 2.2).clamp(0.0, 1.0);
                px += x / radial * out;
                pz += z / radial * out;
                if rng.bool_p(0.35) {
                    y += rng.f32b(0.0, 0.25);
                }
            }
            let yaw = rng.f32() * std::f32::consts::TAU;
            let pitch = rng.f32b(-0.95, 0.95).asin();
            let len = rng.f32b(0.30, 0.58);
            let shade = (0.55 + 0.45 * u) * rng.f32b(0.82, 1.0);
            let tint = rng.f32();
            straws.push(StrawInst {
                pos: [px, y, pz, shade],
                data: [
                    pack_straw(yaw, pitch, len),
                    (tint * 255.0) as u32,
                    0, // state: alive
                    STRAW_KIND_PILE,
                ],
            });
            grid.insert(Vec3::new(px, y, pz), i);
        }

        // The needle: buried somewhere inside, biased towards reachable depth.
        let needle = loop {
            let (x, y, z, u) = Self::sample(&mut rng, 0.55, 0.92);
            if u > 0.5 {
                let y = y.max(0.004);
                break Needle {
                    pos: Vec3::new(x, y, z),
                    yaw: rng.f32() * std::f32::consts::TAU,
                    found: false,
                };
            }
        };

        // Valuables scattered in the outer shell.
        let mut valuables = Vec::new();
        let kinds: [(ValuableKind, f32); 10] = [
            (ValuableKind::Coin, 0.26),
            (ValuableKind::Button, 0.11),
            (ValuableKind::Thimble, 0.08),
            (ValuableKind::Key, 0.08),
            (ValuableKind::Spoon, 0.08),
            (ValuableKind::Fork, 0.08),
            (ValuableKind::Horseshoe, 0.07),
            (ValuableKind::Ring, 0.09),
            (ValuableKind::Locket, 0.07),
            (ValuableKind::Watch, 0.08),
        ];
        for _ in 0..40 {
            let (x, y, z, _u) = Self::sample(&mut rng, 0.68, 0.96);
            let mut r = rng.f32();
            let mut kind = kinds[0].0;
            for (k, w) in kinds {
                if r < w {
                    kind = k;
                    break;
                }
                r -= w;
            }
            valuables.push(Valuable {
                pos: Vec3::new(x, y.max(0.004), z),
                yaw: rng.f32() * std::f32::consts::TAU,
                kind,
                taken: false,
            });
        }

        World {
            seed,
            straw_count,
            straws,
            removed: vec![0u64; (straw_count as usize + 63) / 64],
            removed_count: 0,
            needle,
            valuables,
            grid,
            pending_removals: Vec::new(),
        }
    }

    fn sample(rng: &mut Rng, u_min: f32, u_max: f32) -> (f32, f32, f32, f32) {
        loop {
            let x = rng.f32b(-PILE_R, PILE_R);
            let z = rng.f32b(-PILE_R, PILE_R);
            let y = rng.f32b(0.05, PILE_H);
            let u = (x / PILE_R).powi(2) + (y / PILE_H).powi(2) + (z / PILE_R).powi(2);
            if u > 1.0 || u < u_min || u > u_max {
                continue;
            }
            break (x, y.max(0.02), z, u);
        }
    }

    #[inline]
    pub fn is_removed(&self, i: u32) -> bool {
        (self.removed[(i >> 6) as usize] >> (i & 63)) & 1 == 1
    }

    #[inline]
    fn mark_removed(&mut self, i: u32) -> bool {
        let w = (i >> 6) as usize;
        let b = i & 63;
        if self.removed[w] & (1u64 << b) != 0 {
            return false;
        }
        self.removed[w] |= 1u64 << b;
        self.straws[i as usize].data[2] = 1;
        self.pending_removals.push(i);
        self.removed_count += 1;
        true
    }

    /// Random alive straw (for baler/vacuum automation). Returns None if done.
    pub fn pop_random_alive(&mut self, rng: &mut Rng) -> Option<u32> {
        if self.removed_count >= self.straw_count {
            return None;
        }
        for _ in 0..64 {
            let i = (rng.next_u64() % self.straw_count as u64) as u32;
            if self.mark_removed(i) {
                return Some(i);
            }
        }
        // fallback: linear scan
        for i in 0..self.straw_count {
            if self.mark_removed(i) {
                return Some(i);
            }
        }
        None
    }

    /// Raycast against the haystack ellipsoid (unit space quadratic).
    /// Returns entry parameter t (>= 0) or None if the ray misses the pile.
    pub fn pile_entry(o: Vec3, d: Vec3) -> Option<f32> {
        let os = Vec3::new(o.x / PILE_R, o.y / PILE_H, o.z / PILE_R);
        let ds = Vec3::new(d.x / PILE_R, d.y / PILE_H, d.z / PILE_R);
        let a = ds.dot(ds);
        let b = 2.0 * os.dot(ds);
        let c = os.dot(os) - 1.0;
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        let t0 = (-b - sq) / (2.0 * a);
        let t1 = (-b + sq) / (2.0 * a);
        if t1 < 0.0 {
            return None;
        }
        Some(t0.max(0.0))
    }

    fn cells_around(&self, p: Vec3, r: f32) -> Vec<u32> {
        let c = self.grid.cell;
        let mut out = Vec::with_capacity(256);
        let x0 = (((p.x - r + PILE_R + 2.0) / c) as isize).max(0) as usize;
        let x1 = (((p.x + r + PILE_R + 2.0) / c) as usize).min(self.grid.dim_x - 1);
        let y0 = (((p.y - r + 1.0) / c) as isize).max(0) as usize;
        let y1 = (((p.y + r + 1.0) / c) as usize).min(self.grid.dim_y - 1);
        let z0 = (((p.z - r + PILE_R + 2.0) / c) as isize).max(0) as usize;
        let z1 = (((p.z + r + PILE_R + 2.0) / c) as usize).min(self.grid.dim_z - 1);
        for z in z0..=z1 {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    out.extend_from_slice(&self.grid.cells[self.grid.idx(x, y, z)]);
                }
            }
        }
        out
    }

    /// Dig with a spherical scoop at the ray/pile hit point.
    /// Returns (hit point, removed straw positions).
    pub fn scoop(&mut self, o: Vec3, d: Vec3, radius: f32, cap: u32) -> (Option<Vec3>, Vec<Vec3>) {
        let mut removed = Vec::new();
        let Some(t) = Self::pile_entry(o, d) else {
            return (None, removed);
        };
        let hit = o + d * t;
        let mut count = 0u32;
        for i in self.cells_around(hit, radius + 0.6) {
            if count >= cap {
                break;
            }
            let s = &self.straws[i as usize];
            let sp = Vec3::new(s.pos[0], s.pos[1], s.pos[2]);
            if sp.distance(hit) <= radius && self.mark_removed(i) {
                removed.push(sp);
                count += 1;
            }
        }
        (Some(hit), removed)
    }

    /// Hand pick: remove the straw(s) closest to the aim ray near the pile surface.
    pub fn pick(&mut self, o: Vec3, d: Vec3, max_n: u32) -> (Option<Vec3>, Vec<Vec3>) {
        let mut removed = Vec::new();
        let Some(t) = Self::pile_entry(o, d) else {
            return (None, removed);
        };
        let hit = o + d * t;
        // search along the ray around the entry point
        let mut best: Vec<(f32, u32, Vec3)> = Vec::new(); // (score, idx, pos)
        for dt in [-0.4f32, 0.0, 0.4] {
            let p = o + d * (t + dt);
            for i in self.cells_around(p, 0.45) {
                if self.is_removed(i) {
                    continue;
                }
                let s = &self.straws[i as usize];
                let sp = Vec3::new(s.pos[0], s.pos[1], s.pos[2]);
                let to_s = sp - o;
                let proj = to_s.dot(d);
                if proj < 0.0 {
                    continue;
                }
                let perp = to_s.distance(d * proj);
                if perp < 0.30 {
                    best.push((perp + (t - proj).abs() * 0.3, i, sp));
                }
            }
        }
        best.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        best.dedup_by(|a, b| a.1 == b.1);
        for (_, i, sp) in best.iter().take(max_n as usize) {
            if self.mark_removed(*i) {
                removed.push(*sp);
            }
        }
        if removed.is_empty() {
            // graceful fallback: small scoop at the surface
            return self.scoop(o, d, 0.22, 1);
        }
        (Some(hit), removed)
    }

    pub fn take_pending_removals(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.pending_removals)
    }

    // ----- persistence -----

    pub fn removed_bytes(&self) -> Vec<u8> {
        self.removed.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    pub fn restore_removed(&mut self, bytes: &[u8], count: u32) -> bool {
        if bytes.len() != self.removed.len() * 8 {
            return false;
        }
        for (i, w) in self.removed.iter_mut().enumerate() {
            let b: [u8; 8] = bytes[i * 8..i * 8 + 8].try_into().unwrap();
            *w = u64::from_le_bytes(b);
        }
        for (i, s) in self.straws.iter_mut().enumerate() {
            let w = self.removed[i >> 6];
            s.data[2] = if (w >> (i & 63)) & 1 != 0 { 1 } else { 0 };
        }
        self.removed_count = count;
        true
    }
}
