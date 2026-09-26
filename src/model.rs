// .bmesh model loader (Blender-generated assets) + smooth normal computation.
// Format: 'BMH1' | u32 nverts | [6 x f32] * nverts (pos3 + col3) | u32 ntris | [3 x u32] * ntris.
// Normals are computed once at load (angle-weighted) - keeps assets tiny.

use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub col: [f32; 3],
}

pub struct Model {
    pub verts: Vec<ModelVertex>,
    pub idx: Vec<u32>,
}

const MODELS: &[(&str, &[u8])] = &[
    ("needle", include_bytes!("../assets/models/needle.bmesh")),
    ("coin", include_bytes!("../assets/models/coin.bmesh")),
    ("button", include_bytes!("../assets/models/button.bmesh")),
    ("thimble", include_bytes!("../assets/models/thimble.bmesh")),
    ("key", include_bytes!("../assets/models/key.bmesh")),
    ("spoon", include_bytes!("../assets/models/spoon.bmesh")),
    ("fork", include_bytes!("../assets/models/fork.bmesh")),
    ("horseshoe", include_bytes!("../assets/models/horseshoe.bmesh")),
    ("ring", include_bytes!("../assets/models/ring.bmesh")),
    ("locket", include_bytes!("../assets/models/locket.bmesh")),
    ("watch", include_bytes!("../assets/models/watch.bmesh")),
    ("detector", include_bytes!("../assets/models/detector.bmesh")),
    ("pitchfork", include_bytes!("../assets/models/pitchfork.bmesh")),
    ("shovel", include_bytes!("../assets/models/shovel.bmesh")),
    ("baler", include_bytes!("../assets/models/baler.bmesh")),
    ("vacuum", include_bytes!("../assets/models/vacuum.bmesh")),
    ("farmer", include_bytes!("../assets/models/farmer.bmesh")),
];

pub const M_NEEDLE: usize = 0;
pub const M_COIN: usize = 1;
pub const M_BUTTON: usize = 2;
pub const M_THIMBLE: usize = 3;
pub const M_KEY: usize = 4;
pub const M_SPOON: usize = 5;
pub const M_FORK: usize = 6;
pub const M_HORSESHOE: usize = 7;
pub const M_RING: usize = 8;
pub const M_LOCKET: usize = 9;
pub const M_WATCH: usize = 10;
pub const M_DETECTOR: usize = 11;
pub const M_PITCHFORK: usize = 12;
pub const M_SHOVEL: usize = 13;
pub const M_BALER: usize = 14;
pub const M_VACUUM: usize = 15;
pub const M_FARMER: usize = 16;

pub fn load_all() -> Vec<Model> {
    MODELS
        .iter()
        .map(|(name, data)| {
            let m = parse(data).unwrap_or_else(|e| {
                eprintln!("model {name}: {e}");
                Model { verts: Vec::new(), idx: Vec::new() }
            });
            m
        })
        .collect()
}

fn parse(data: &[u8]) -> Result<Model, String> {
    if data.len() < 12 || &data[0..4] != b"BMH1" {
        return Err("bad magic".into());
    }
    let rd_u32 = |o: usize| -> u32 {
        u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
    };
    let nv = rd_u32(4) as usize;
    let mut off = 8usize;
    let f32at = |o: usize| -> f32 {
        f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
    };
    let mut pos = Vec::with_capacity(nv);
    let mut col = Vec::with_capacity(nv);
    for _ in 0..nv {
        pos.push(Vec3::new(f32at(off), f32at(off + 4), f32at(off + 8)));
        col.push(Vec3::new(f32at(off + 12), f32at(off + 16), f32at(off + 20)));
        off += 24;
    }
    let nt = rd_u32(off) as usize;
    off += 4;
    let mut tris = Vec::with_capacity(nt * 3);
    for _ in 0..nt {
        let a = rd_u32(off) as usize;
        let b = rd_u32(off + 4) as usize;
        let c = rd_u32(off + 8) as usize;
        off += 12;
        tris.push([a, b, c]);
    }

    // Angle-weighted smooth normals, one pass.
    let mut nrm = vec![Vec3::ZERO; nv];
    for t in &tris {
        let (a, b, c) = (t[0], t[1], t[2]);
        let e1 = pos[b] - pos[a];
        let e2 = pos[c] - pos[a];
        let fn_ = e1.cross(e2);
        // skip degenerate
        if fn_.length_squared() < 1e-18 {
            continue;
        }
        nrm[a] += fn_;
        nrm[b] += fn_;
        nrm[c] += fn_;
    }
    let verts: Vec<ModelVertex> = (0..nv)
        .map(|i| {
            let n = nrm[i].normalize_or_zero();
            ModelVertex {
                pos: pos[i].to_array(),
                nrm: n.to_array(),
                col: col[i].to_array(),
            }
        })
        .collect();
    let idx: Vec<u32> = tris.iter().flat_map(|t| t.iter().copied().map(|i| i as u32)).collect();
    Ok(Model { verts, idx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_models_parse() {
        let models = load_all();
        assert_eq!(models.len(), MODELS.len());
        for (i, m) in models.iter().enumerate() {
            assert!(m.verts.len() > 8, "model {} has no verts", MODELS[i].0);
            assert!(m.idx.len() >= 12, "model {} has no tris", MODELS[i].0);
            assert_eq!(m.idx.len() % 3, 0);
        }
        // needle is tiny (7cm) but present
        let n = &models[M_NEEDLE];
        let mut minx = f32::MAX;
        let mut maxx = f32::MIN;
        for v in &n.verts {
            minx = minx.min(v.pos[0]);
            maxx = maxx.max(v.pos[0]);
        }
        assert!((maxx - minx) > 0.06 && (maxx - minx) < 0.08, "needle length {}mm", (maxx - minx) * 1000.0);
    }
}
