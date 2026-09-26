// Game state: tools, economy, shop, win/lose flow, HUD text, persistence.

use crate::mesh::ValuableKind;
use crate::rng::Rng;
use crate::world::World;
use glam::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    Menu,
    Playing,
    Shop,
    Paused,
    Won,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Hands,
    Gloves,
    Pitchfork,
    Shovel,
    Detector,
    DetectorII,
    Magnet,
    Baler,
    Vacuum,
}

#[derive(Clone, Copy)]
pub struct ToolDef {
    pub tool: Tool,
    pub name: &'static str,
    pub price: i32,
    pub desc: &'static str,
    pub automation: Option<(u32, f32)>, // (straws per action, actions per second)
}

pub const TOOLS: [ToolDef; 9] = [
    ToolDef { tool: Tool::Hands, name: "BARE HANDS", price: 0, desc: "1 straw at a time. The true experience.", automation: None },
    ToolDef { tool: Tool::Gloves, name: "GLOVES", price: 60, desc: "Grab 3 straws per pick.", automation: None },
    ToolDef { tool: Tool::Pitchfork, name: "PITCHFORK", price: 320, desc: "Scoop ~12 straws at once.", automation: None },
    ToolDef { tool: Tool::Shovel, name: "SHOVEL", price: 900, desc: "Scoop ~45 straws at once.", automation: None },
    ToolDef { tool: Tool::Detector, name: "METAL DETECTOR", price: 600, desc: "Pings within a 2m radius.", automation: None },
    ToolDef { tool: Tool::DetectorII, name: "DETECTOR MK-II", price: 4000, desc: "Pings within a 6m radius.", automation: None },
    ToolDef { tool: Tool::Magnet, name: "GIANT MAGNET", price: 800, desc: "Pulls nearby valuables to you.", automation: None },
    ToolDef { tool: Tool::Baler, name: "HAY BALER", price: 2500, desc: "Eats 25 straws/sec, forever.", automation: Some((25, 1.0)) },
    ToolDef { tool: Tool::Vacuum, name: "HAY VACUUM MK-II", price: 12000, desc: "Eats 120 straws/sec, forever.", automation: Some((120, 1.0)) },
];

pub struct Toast {
    pub text: String,
    pub life: f32,
}

pub struct Game {
    pub state: GameState,
    pub seed: u64,
    pub pure_mode: bool,
    pub money: i32,
    pub owned: [bool; 9],
    pub selected: usize,
    pub time: f64,
    pub dig_cd: f32,
    pub removed_count: u32,
    pub valuables_bag: Vec<(ValuableKind, i32)>, // kind, value
    pub bag_value: i32,
    pub toasts: Vec<Toast>,
    pub best_time: Option<f64>,
    pub menu_seed_input: bool,
    pub seed_input: String,
    pub automation_acc: f32,
    pub won_time: f64,
    pub detector_pulse: f32,
    pub detector_hot: bool,
    pub bg_fade: f32,
    pub time_accum: f64,
    pub last_valuable_idx: Option<u32>,
}

impl Game {
    pub fn new(seed: u64, pure_mode: bool) -> Self {
        Game {
            state: GameState::Menu,
            seed,
            pure_mode,
            money: 0,
            owned: [false; 9],
            selected: 0,
            time: 0.0,
            dig_cd: 0.0,
            removed_count: 0,
            valuables_bag: Vec::new(),
            bag_value: 0,
            toasts: Vec::new(),
            best_time: load_best_time(),
            menu_seed_input: false,
            seed_input: String::new(),
            automation_acc: 0.0,
            won_time: 0.0,
            detector_pulse: 0.0,
            detector_hot: false,
            bg_fade: 0.0,
            time_accum: 0.0,
            last_valuable_idx: None,
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), life: 3.2 });
        if self.toasts.len() > 6 {
            self.toasts.remove(0);
        }
    }

    pub fn current_tool(&self) -> Tool {
        TOOLS[self.selected].tool
    }

    pub fn pick_n(&self) -> u32 {
        if self.pure_mode {
            return 1;
        }
        match self.current_tool() {
            Tool::Hands => 1,
            Tool::Gloves => 3,
            _ => 1,
        }
    }

    pub fn scoop_params(&self) -> Option<(f32, u32)> {
        if self.pure_mode {
            return None;
        }
        match self.current_tool() {
            Tool::Pitchfork => Some((0.26, 14)),
            Tool::Shovel => Some((0.45, 50)),
            _ => None,
        }
    }

    pub fn cooldown(&self) -> f32 {
        match self.current_tool() {
            Tool::Hands => 0.30,
            Tool::Gloves => 0.28,
            Tool::Pitchfork => 0.42,
            Tool::Shovel => 0.55,
            _ => 0.30,
        }
    }

    pub fn detector_radius(&self) -> Option<f32> {
        if self.pure_mode {
            return None;
        }
        match self.current_tool() {
            Tool::Detector => Some(2.0),
            Tool::DetectorII => Some(6.0),
            _ => None,
        }
    }

    pub fn buy(&mut self, slot: usize) {
        if self.pure_mode {
            self.toast("PURE MODE: NO TOOLS. JUST YOU AND THE PILE.");
            return;
        }
        let def = &TOOLS[slot];
        if self.owned[slot] {
            self.selected = slot;
            self.toast(format!("EQUIPPED: {}", def.name));
            return;
        }
        if self.money >= def.price {
            self.money -= def.price;
            self.owned[slot] = true;
            self.selected = slot;
            self.toast(format!("BOUGHT: {} (-${})", def.name, def.price));
        } else {
            self.toast(format!("NOT ENOUGH MONEY (${} / ${})", self.money, def.price));
        }
    }

    pub fn sell_all(&mut self) {
        if self.bag_value <= 0 {
            self.toast("NOTHING TO SELL. GO DIG.");
            return;
        }
        let total = self.bag_value;
        self.money += total;
        self.valuables_bag.clear();
        self.bag_value = 0;
        self.toast(format!("SOLD EVERYTHING: +${}", total));
    }

    pub fn collect_valuable(&mut self, kind: ValuableKind) {
        let v = kind.value();
        self.valuables_bag.push((kind, v));
        self.bag_value += v;
        self.toast(format!("FOUND: {} (${})", kind.name(), v));
    }

    pub fn update(&mut self, dt: f32, world: &mut World, cam_pos: Vec3, cam_fwd: Vec3) {
        self.bg_fade = (self.bg_fade + dt * 3.0).min(1.0);
        self.time_accum += dt as f64;
        for t in self.toasts.iter_mut() {
            t.life -= dt;
        }
        self.toasts.retain(|t| t.life > 0.0);
        self.dig_cd -= dt;

        if self.state != GameState::Playing {
            return;
        }
        self.time += dt as f64;

        // automation (baler / vacuum)
        if !self.pure_mode {
            let mut rate = 0.0f32;
            if self.owned[7] {
                rate += 25.0;
            }
            if self.owned[8] {
                rate += 120.0;
            }
            if rate > 0.0 {
                self.automation_acc += dt * rate;
                let n = self.automation_acc as u32;
                if n > 0 {
                    self.automation_acc -= n as f32;
                    let mut rng = Rng::new((self.time_accum * 1000.0) as u64 | 1);
                    for _ in 0..n.min(400) {
                        if world.pop_random_alive(&mut rng).is_some() {
                            self.removed_count += 1;
                        }
                    }
                }
            }
        }

        // detector
        if let Some(r) = self.detector_radius() {
            let dist = (world.needle.pos - cam_pos).length();
            self.detector_hot = dist < r && !world.needle.found;
            let speed = if self.detector_hot {
                (2.2 - 1.6 * (dist / r).clamp(0.0, 1.0))
            } else {
                0.0
            };
            self.detector_pulse += dt * speed;
        }

        // magnet
        if !self.pure_mode && self.owned[6] {
            for vi in 0..world.valuables.len() {
                let v = &mut world.valuables[vi];
                if !v.taken && (v.pos - cam_pos).length() < 2.5 {
                    v.taken = true;
                    self.collect_valuable(v.kind);
                    self.last_valuable_idx = Some(vi as u32);
                }
            }
        }

        let _ = (cam_fwd,);
    }

    /// LMB action. Returns (hit_pos, burst_positions, tossed_origin).
    pub fn dig(&mut self, world: &mut World, o: Vec3, d: Vec3) -> Option<(Vec3, Vec<Vec3>)> {
        if self.dig_cd > 0.0 || self.state != GameState::Playing {
            return None;
        }
        self.dig_cd = self.cooldown();

        // picking the needle itself?
        if Self::aim_at_needle(o, d, world.needle.pos, world.needle.found) {
            world.needle.found = true;
            self.won_time = self.time;
            self.state = GameState::Won;
            if self.best_time.map_or(true, |b| self.won_time < b) {
                self.best_time = Some(self.won_time);
                save_best_time(self.won_time);
                self.toast("NEW BEST TIME!");
            }
            return None;
        }

        // picking a valuable?
        for vi in 0..world.valuables.len() {
            let v = &mut world.valuables[vi];
            if !v.taken && Self::aim_at_item(o, d, v.pos, 0.16) {
                v.taken = true;
                self.collect_valuable(v.kind);
                self.last_valuable_idx = Some(vi as u32);
                self.dig_cd = 0.15;
                return None;
            }
        }

        let mut removed = Vec::new();
        let hit;
        if let Some((r, cap)) = self.scoop_params() {
            let (h, rm) = world.scoop(o, d, r, cap);
            hit = h;
            removed = rm;
        } else {
            let (h, rm) = world.pick(o, d, self.pick_n());
            hit = h;
            removed = rm;
        }
        self.removed_count += removed.len() as u32;
        if removed.is_empty() {
            return None;
        }
        Some((hit?, removed))
    }

    pub fn aim_at_needle(o: Vec3, d: Vec3, needle: Vec3, found: bool) -> bool {
        if found {
            return false;
        }
        let to = needle - o;
        let dist = to.length();
        if dist > 2.2 {
            return false;
        }
        let cosang = to.normalize().dot(d);
        let tol = (0.035 / dist.max(0.05)).atan() + 0.05;
        cosang > tol.cos()
    }

    pub fn aim_at_item(o: Vec3, d: Vec3, pos: Vec3, radius: f32) -> bool {
        let to = pos - o;
        let dist = to.length();
        if dist > 2.4 {
            return false;
        }
        let cosang = to.normalize().dot(d);
        let tol = (radius / dist.max(0.05)).atan() + 0.06;
        cosang > tol.cos()
    }
}

pub fn format_time(t: f64) -> String {
    let m = (t / 60.0) as u64;
    let s = (t % 60.0) as u64;
    let cs = ((t * 100.0) % 100.0) as u64;
    format!("{:02}:{:02}.{:02}", m, s, cs)
}

// ----- tiny save persistence -----

const BEST_FILE: &str = "best_time.txt";

fn load_best_time() -> Option<f64> {
    std::fs::read_to_string(BEST_FILE).ok()?.trim().parse().ok()
}

fn save_best_time(t: f64) {
    let _ = std::fs::write(BEST_FILE, format!("{}", t));
}

// ----- full save game -----

const SAVE_MAGIC: &[u8; 4] = b"FNDL";
const SAVE_VERSION: u32 = 1;

pub fn save_game(game: &Game, world: &World, path: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = Vec::new();
    f.extend_from_slice(SAVE_MAGIC);
    f.extend_from_slice(&SAVE_VERSION.to_le_bytes());
    f.extend_from_slice(&game.seed.to_le_bytes());
    f.extend_from_slice(&world.straw_count.to_le_bytes());
    f.extend_from_slice(&game.money.to_le_bytes());
    for o in &game.owned {
        f.push(*o as u8);
    }
    f.extend_from_slice(&(game.selected as u32).to_le_bytes());
    f.push(game.pure_mode as u8);
    f.extend_from_slice(&game.time.to_le_bytes());
    f.extend_from_slice(&game.removed_count.to_le_bytes());
    f.extend_from_slice(&world.removed_bytes());
    f.extend_from_slice(&game.won_time.to_le_bytes());
    let mut file = std::fs::File::create(path)?;
    file.write_all(&f)?;
    Ok(())
}

pub struct LoadResult {
    pub game: Game,
}

pub fn load_game(path: &str) -> Option<(Game, u32, Vec<u8>, u32)> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 44 || &data[0..4] != SAVE_MAGIC {
        return None;
    }
    let ver = u32::from_le_bytes(data[4..8].try_into().ok()?);
    if ver != SAVE_VERSION {
        return None;
    }
    let seed = u64::from_le_bytes(data[8..16].try_into().ok()?);
    let straw_count = u32::from_le_bytes(data[16..20].try_into().ok()?);
    let money = i32::from_le_bytes(data[20..24].try_into().ok()?);
    let mut owned = [false; 9];
    for i in 0..9 {
        owned[i] = data[24 + i] != 0;
    }
    let selected = u32::from_le_bytes(data[33..37].try_into().ok()?) as usize;
    let pure = data[37] != 0;
    let time = f64::from_le_bytes(data[38..46].try_into().ok()?);
    let removed_count = u32::from_le_bytes(data[46..50].try_into().ok()?);
    // the save ends with an 8-byte won_time after the bitset
    if data.len() < 58 {
        return None;
    }
    let bitset_end = data.len() - 8;
    let won_time = f64::from_le_bytes(data[bitset_end..].try_into().ok()?);
    let bitset = data[50..bitset_end].to_vec();

    let mut game = Game::new(seed, pure);
    game.money = money;
    game.owned = owned;
    game.selected = selected.min(8);
    game.time = time;
    game.won_time = won_time;
    Some((game, straw_count, bitset, removed_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn world_dig_and_save_roundtrip() {
        let mut w = World::new(42, 50_000);
        let mut g = Game::new(42, false);
        g.state = GameState::Playing;
        // dig toward the pile center from above the surface
        let o = glam::Vec3::new(0.0, 2.0, 30.0);
        let d = glam::Vec3::new(0.0, 0.0, -1.0);
        let mut removed_total = 0u32;
        for _ in 0..200 {
            let _ = g.dig(&mut w, o, d);
            let idxs = w.take_pending_removals();
            removed_total += idxs.len() as u32;
        }
        assert!(removed_total > 0, "digging must remove straws");
        assert_eq!(w.removed_count, removed_total);
        // bitset roundtrip
        let bytes = w.removed_bytes();
        let w2_bytes = bytes.clone();
        assert!(w.restore_removed(&w2_bytes, removed_total));
        // save/load
        save_game(&g, &w, "test_save.bin").unwrap();
        let (g2, count, bitset, rc) = load_game("test_save.bin").unwrap();
        assert_eq!(count, 50_000);
        assert_eq!(rc, removed_total);
        assert_eq!(bitset.len(), bytes.len());
        assert_eq!(g2.seed, 42);
        let _ = std::fs::remove_file("test_save.bin");
    }

    #[test]
    fn needle_is_findable() {
        let w = World::new(7, 20_000);
        assert!(!w.needle.found);
        assert!(w.needle.pos.y >= 0.0);
        assert!(w.straw_count == 20_000);
    }

    #[test]
    fn tools_prices_ordered() {
        for t in TOOLS.iter() {
            assert!(t.price >= 0);
        }
        assert_eq!(TOOLS[0].tool, Tool::Hands);
    }
}
