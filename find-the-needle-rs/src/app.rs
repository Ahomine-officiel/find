// Application state: input, movement, interactions, per-frame simulation,
// HUD building (in-game + menu + shop + YARD RESEARCH + win screen).

use crate::cam::Camera;
use crate::game::{
    format_money, format_time, price_for_level, total_levels, Game, GameState, RESEARCH, TOOLS,
};
use crate::hud::{Hud, HudQuad};
use crate::mesh::ValuableKind;
use crate::render::{PropInst, Renderer};
use crate::rng::Rng;
use crate::settings::{Quality, Settings};
use crate::world::{StrawInst, World, PILE_H, PILE_R};
use glam::{Mat4, Vec3};
use std::collections::HashSet;
use winit::keyboard::{KeyCode, PhysicalKey};

// Barn layout constants (must match mesh.rs)
const BARN_X: f32 = 23.0;
const BARN_Z: f32 = 17.0;
const SHOP_POS: Vec3 = Vec3::new(-14.0, 0.0, -13.0);
const STALL_FRONT: Vec3 = Vec3::new(STALL_X - 1.8, 0.0, 6.0);
const STALL_X: f32 = 19.0;
const BELT_P0: Vec3 = Vec3::new(11.0, 0.52, 1.0);
const BELT_P1: Vec3 = Vec3::new(20.0, 1.66, 4.0);

pub struct Tossed {
    pos: Vec3,
    vel: Vec3,
    spin: f32,
    life: f32,
}

pub struct Ball {
    t: f32,
    speed: f32,
    offset: f32,
}

pub struct App {
    pub window: Option<std::sync::Arc<winit::window::Window>>,
    pub renderer: Renderer,
    pub settings: Settings,
    pub world: World,
    pub game: Game,
    pub cam: Camera,

    keys: HashSet<KeyCode>,
    mouse_dx: f32,
    mouse_dy: f32,
    lmb: bool,
    rmb_held: bool,
    pointer_locked: bool,
    vy: f32,
    pending_hud: Vec<HudQuad>,

    pub time: f64,
    last_time: Option<std::time::Instant>,
    fps_frames: u32,
    fps_accum: f32,
    fps_value: u32,
    pub show_fps: bool,

    tossed: Vec<Tossed>,
    balls: Vec<Ball>,
    loop_balls: Vec<Ball>,
    clumps: Vec<Vec3>,
    dig_anim: f32,
    bob_phase: f32,

    pub quit: bool,
    headless_size: (u32, u32),
}

impl App {
    fn device_desc() -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            label: Some("find-the-needle"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        }
    }

    pub fn new(window: std::sync::Arc<winit::window::Window>) -> Self {
        let settings = Settings::load();
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window.clone())
            .expect("surface creation failed");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no suitable GPU adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Self::device_desc(), None))
                .expect("device creation failed");
        Self::build(
            (size.width, size.height),
            Some(surface),
            &adapter,
            device,
            queue,
            Some(window),
            settings,
        )
    }

    /// Headless app (no window): for `--shot` UI captures on any adapter.
    pub fn new_headless(size: (u32, u32)) -> Self {
        let settings = Settings::load();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("headless: no GPU adapter found");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Self::device_desc(), None))
                .expect("headless: device creation failed");
        Self::build(size, None, &adapter, device, queue, None, settings)
    }

    fn build(
        size: (u32, u32),
        surface: Option<wgpu::Surface<'static>>,
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        window: Option<std::sync::Arc<winit::window::Window>>,
        settings: Settings,
    ) -> Self {
        let seed = Rng::new(0xF1EED).next_u64()
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(1);
        let world = World::new(seed, settings.quality.straw_count());
        let game = Game::new(seed, false);

        let mut renderer = Renderer::new(device, queue, surface, adapter, size, &world, &settings);
        renderer.set_present_mode(settings.vsync);

        let mut balls = Vec::new();
        for i in 0..10 {
            balls.push(Ball {
                t: (i as f32) / 10.0,
                speed: 0.14,
                offset: (i as f32 * 1.7).sin() * 0.05,
            });
        }

        App {
            window,
            headless_size: size,
            renderer,
            settings,
            world,
            game,
            cam: Camera::new(75.0),
            keys: HashSet::new(),
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            lmb: false,
            rmb_held: false,
            pointer_locked: false,
            vy: 0.0,
            pending_hud: Vec::with_capacity(1024),
            time: 0.0,
            last_time: None,
            fps_frames: 0,
            fps_accum: 0.0,
            fps_value: 0,
            show_fps: false,
            tossed: Vec::with_capacity(2048),
            balls,
            loop_balls: Vec::new(),
            clumps: Vec::new(),
            dig_anim: 0.0,
            bob_phase: 0.0,
            quit: false,
        }
    }

    fn win_size(&self) -> (f32, f32) {
        match &self.window {
            Some(w) => {
                let s = w.inner_size();
                (s.width.max(1) as f32, s.height.max(1) as f32)
            }
            None => (self.headless_size.0.max(1) as f32, self.headless_size.1.max(1) as f32),
        }
    }

    /// Headless screenshot: builds the HUD for the current state, renders one
    /// frame offscreen and writes a .bmp for inspection.
    pub fn headless_shot(&mut self, path: &str) {
        self.build_hud();
        let hud_snapshot = Hud {
            quads: std::mem::take(&mut self.pending_hud),
        };
        let (straw_dyn, prop_insts, batches) = self.collect_instances();
        self.renderer.run_selftest(
            &self.cam,
            &hud_snapshot,
            &straw_dyn,
            &prop_insts,
            &batches,
            Some(path),
        );
        eprintln!("shot written: {path}");
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h, self.settings.render_scale);
    }

    pub fn key(&mut self, pk: PhysicalKey, pressed: bool) {
        let code = match pk {
            PhysicalKey::Code(c) => c,
            _ => return,
        };
        if pressed {
            if self.keys.insert(code) {
                self.key_just_pressed(code);
            }
        } else {
            self.keys.remove(&code);
        }
    }

    fn key_just_pressed(&mut self, code: KeyCode) {
        match self.game.state {
            GameState::Menu => self.menu_key(code),
            GameState::Playing => match code {
                KeyCode::Escape => {
                    self.unlock_pointer();
                    self.game.state = GameState::Paused;
                }
                KeyCode::Tab => {
                    self.unlock_pointer();
                    self.game.state = GameState::Research;
                    self.game.ui_index = 0;
                }
                KeyCode::KeyE => self.try_interact(),
                KeyCode::F5 => {
                    let r = crate::game::save_game(&self.game, &self.world, "save.bin");
                    self.game.toast(if r.is_ok() { "GAME SAVED" } else { "SAVE FAILED" });
                }
                KeyCode::F9 => self.load_save(),
                KeyCode::F3 => self.show_fps = !self.show_fps,
                _ => {}
            },
            GameState::Shop => match code {
                KeyCode::Escape | KeyCode::KeyE => {
                    self.game.state = GameState::Playing;
                    self.lock_pointer();
                }
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.game.shop_index = self
                        .game
                        .shop_index
                        .saturating_sub(1)
                        .min(TOOLS.len() - 1);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.game.shop_index = (self.game.shop_index + 1).min(TOOLS.len() - 1);
                }
                KeyCode::Enter | KeyCode::Space => {
                    let idx = self.game.shop_index;
                    self.game.buy_tool(idx);
                }
                _ => {}
            },
            GameState::Research => match code {
                KeyCode::Escape | KeyCode::Tab => {
                    self.game.state = GameState::Playing;
                    self.lock_pointer();
                }
                KeyCode::ArrowLeft | KeyCode::KeyA => {
                    self.game.ui_index = self.game.ui_index.saturating_sub(1);
                }
                KeyCode::ArrowRight | KeyCode::KeyD => {
                    self.game.ui_index += 1;
                }
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.game.research_cat = (self.game.research_cat
                        + crate::game::RCategory::ALL.len()
                        - 1)
                        % crate::game::RCategory::ALL.len();
                    self.game.ui_index = 0;
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.game.research_cat =
                        (self.game.research_cat + 1) % crate::game::RCategory::ALL.len();
                    self.game.ui_index = 0;
                }
                KeyCode::Enter | KeyCode::Space => {
                    if let Some(i) = self.card_at_cursor() {
                        self.game.buy_research(i as u32);
                    }
                }
                _ => {}
            },
            GameState::Paused => match code {
                KeyCode::Escape | KeyCode::KeyP => {
                    self.game.state = GameState::Playing;
                    self.lock_pointer();
                }
                KeyCode::KeyQ => {
                    self.game.state = GameState::Menu;
                }
                KeyCode::F3 => self.show_fps = !self.show_fps,
                _ => {}
            },
            GameState::Won => {
                if code == KeyCode::Enter || code == KeyCode::Escape {
                    self.game.state = GameState::Menu;
                }
            }
        }
    }

    fn menu_key(&mut self, code: KeyCode) {
        use KeyCode::*;
        if self.game.menu_seed_input {
            match code {
                Backspace => {
                    self.game.seed_input.pop();
                }
                Enter => {
                    self.game.menu_seed_input = false;
                    if !self.game.seed_input.is_empty() {
                        self.new_game(Some(self.game.seed_input.clone()));
                    }
                }
                Escape => self.game.menu_seed_input = false,
                _ => {}
            }
            return;
        }
        let items = 7usize;
        match code {
            ArrowUp | KeyW => {
                self.game.menu_index = (self.game.menu_index + items - 1) % items;
            }
            ArrowDown | KeyS => {
                self.game.menu_index = (self.game.menu_index + 1) % items;
            }
            Enter | Space => match self.game.menu_index {
                0 => self.new_game(None),
                1 => self.load_save(),
                2 => {
                    self.game.pure_mode = !self.game.pure_mode;
                    self.game.toast(if self.game.pure_mode {
                        "PURE MODE ON: NO TOOLS. JUST YOU AND THE PILE."
                    } else {
                        "PURE MODE OFF"
                    });
                }
                3 => {
                    self.game.menu_seed_input = true;
                    self.game.seed_input.clear();
                }
                4 => {
                    let next = match self.settings.quality {
                        Quality::Low => Quality::Medium,
                        Quality::Medium => Quality::High,
                        Quality::High => Quality::Low,
                    };
                    self.settings.quality = next;
                    self.apply_quality();
                }
                5 => {
                    self.settings.vsync = !self.settings.vsync;
                    self.renderer.set_present_mode(self.settings.vsync);
                    self.settings.save();
                }
                _ => self.quit = true,
            },
            Escape => self.quit = true,
            _ => {}
        }
    }

    fn apply_quality(&mut self) {
        let count = self.settings.quality.straw_count();
        self.world = World::new(self.game.seed, count);
        self.renderer.replace_straw_static(&self.world);
        self.settings.save();
    }

    fn new_game(&mut self, seed_text: Option<String>) {
        let seed = match seed_text {
            Some(s) => crate::rng::hash_seed(&s),
            None => Rng::new(0xF1EED).next_u64()
                ^ std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(1),
        };
        self.game.seed = seed;
        if self.world.seed != seed {
            self.world = World::new(seed, self.settings.quality.straw_count());
            self.renderer.replace_straw_static(&self.world);
        }
        let pure = self.game.pure_mode;
        self.game = Game::new(seed, pure);
        self.game.state = GameState::Playing;
        self.clumps.clear();
        self.tossed.clear();
        self.cam = Camera::new(self.settings.fov);
        self.cam.pos = Vec3::new(-19.5, 1.62, -6.0);
        self.cam.yaw = std::f32::consts::FRAC_PI_2; // face +X (towards the pile)
        self.cam.pitch = -0.02;
        self.vy = 0.0;
        if pure {
            self.game.toast("PURE MODE: 1 STRAND AT A TIME. GOOD LUCK.");
        } else {
            self.game.toast("WELCOME TO THE YARD. FIND THE NEEDLE.");
            self.game.toast("WASD MOVE - LMB DIG - TAB RESEARCH - E INTERACT - F5 SAVE");
        }
        self.lock_pointer();
    }

    fn load_save(&mut self) {
        if let Some((game, straw_count, bitset, removed)) = crate::game::load_game("save.bin") {
            let count = if straw_count > 0 {
                straw_count
            } else {
                self.settings.quality.straw_count()
            };
            if count != self.world.straw_count || self.world.seed != game.seed {
                self.world = World::new(game.seed, count);
                self.renderer.replace_straw_static(&self.world);
            }
            let seed = game.seed;
            let pure = game.pure_mode;
            self.game = game;
            self.game.seed = seed;
            self.game.pure_mode = pure;
            if !self.world.restore_removed(&bitset, removed) {
                self.game.toast("SAVE CORRUPTED (BITSET MISMATCH)");
            }
            self.game.state = GameState::Playing;
            self.cam = Camera::new(self.settings.fov);
            self.cam.pos = Vec3::new(-19.5, 1.62, -6.0);
            self.cam.yaw = std::f32::consts::FRAC_PI_2;
            self.vy = 0.0;
            self.lock_pointer();
            self.game.toast("GAME LOADED");
        } else {
            self.game.toast("NO SAVE FOUND");
        }
    }

    fn try_interact(&mut self) {
        let p = Vec3::new(self.cam.pos.x, 0.0, self.cam.pos.z);
        let stall_d = (p - STALL_FRONT).length();
        let shop_d = (p - SHOP_POS).length();
        if stall_d < 3.4 {
            let earned = self.game.sell_all();
            if earned > 0 {
                self.game.toast(format!("SOLD: +{}", format_money(earned)));
            } else {
                self.game.toast("NOTHING TO SELL. GO DIG.");
            }
            return;
        }
        if shop_d < 4.2 {
            self.unlock_pointer();
            self.game.state = GameState::Shop;
        }
    }

    pub fn mouse_button(
        &mut self,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        let pressed = state == winit::event::ElementState::Pressed;
        match button {
            winit::event::MouseButton::Left => {
                self.lmb = pressed;
                if pressed && !self.pointer_locked && self.game.state == GameState::Playing {
                    self.lock_pointer();
                }
            }
            winit::event::MouseButton::Right => self.rmb_held = pressed,
            _ => {}
        }
    }

    pub fn mouse_motion(&mut self, dx: f64, dy: f64) {
        if self.pointer_locked && self.game.state == GameState::Playing {
            self.mouse_dx += dx as f32;
            self.mouse_dy += dy as f32;
        }
    }

    fn lock_pointer(&mut self) {
        if self.pointer_locked {
            return;
        }
        use winit::window::CursorGrabMode;
        if let Some(w) = &self.window {
            let _ = w.set_cursor_grab(CursorGrabMode::Locked);
            let _ = w.set_cursor_visible(false);
        }
        self.pointer_locked = true;
    }

    fn unlock_pointer(&mut self) {
        use winit::window::CursorGrabMode;
        if let Some(w) = &self.window {
            let _ = w.set_cursor_grab(CursorGrabMode::None);
            let _ = w.set_cursor_visible(true);
        }
        self.pointer_locked = false;
    }

    pub fn redraw(&mut self) {
        let now = std::time::Instant::now();
        let dt = self
            .last_time
            .map(|t| now.duration_since(t).as_secs_f32().min(0.05))
            .unwrap_or(0.016);
        self.last_time = Some(now);
        self.time += dt as f64;

        // fps counter
        self.fps_frames += 1;
        self.fps_accum += dt;
        if self.fps_accum >= 0.5 {
            self.fps_value = (self.fps_frames as f32 / self.fps_accum) as u32;
            self.fps_frames = 0;
            self.fps_accum = 0.0;
        }

        self.update(dt);
        self.build_hud();

        let size = self.window.as_ref().map(|w| w.inner_size()).unwrap_or(winit::dpi::PhysicalSize::new(
            self.headless_size.0.max(1),
            self.headless_size.1.max(1),
        ));
        let aspect = (size.width.max(1) as f32, size.height.max(1) as f32);
        let (straw_dyn, prop_insts, batches) = self.collect_instances();
        let hud_snapshot = Hud {
            quads: std::mem::take(&mut self.pending_hud),
        };
        self.renderer.draw_frame(
            &self.cam,
            aspect,
            self.time as f32,
            if self.settings.clouds { 1.0 } else { 0.0 },
            &straw_dyn,
            &prop_insts,
            &batches,
            &hud_snapshot,
        );
    }

    // ------------------------------------------------------------ update ----

    fn update(&mut self, dt: f32) {
        // camera look
        if self.game.state == GameState::Playing {
            let sens = self.settings.sensitivity * 0.0022;
            self.cam.yaw = (self.cam.yaw - self.mouse_dx * sens)
                .rem_euclid(std::f32::consts::TAU);
            self.cam.pitch = (self.cam.pitch - self.mouse_dy * sens).clamp(-1.5, 1.5);
        }
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;

        // zoom
        let zoom_target = if self.rmb_held { 0.12 } else { 1.0 };
        self.cam.zoom += (zoom_target - self.cam.zoom) * dt.min(0.1) * 8.0;

        let playing = self.game.state == GameState::Playing;
        let mut moving = false;

        if playing {
            // movement
            let fwd = Vec3::new(self.cam.yaw.sin(), 0.0, -self.cam.yaw.cos());
            let right = Vec3::new(self.cam.yaw.cos(), 0.0, self.cam.yaw.sin());
            let mut wish = Vec3::ZERO;
            if self.keys.contains(&KeyCode::KeyW) {
                wish += fwd;
            }
            if self.keys.contains(&KeyCode::KeyS) {
                wish -= fwd;
            }
            if self.keys.contains(&KeyCode::KeyD) {
                wish += right;
            }
            if self.keys.contains(&KeyCode::KeyA) {
                wish -= right;
            }
            if wish.length_squared() > 0.0 {
                wish = wish.normalize();
                moving = true;
            }
            let sprint = self.keys.contains(&KeyCode::ShiftLeft)
                || self.keys.contains(&KeyCode::ShiftRight);
            let speed = self.game.walk_speed() * if sprint { 1.7 } else { 1.0 };

            let mut new_pos = self.cam.pos + wish * speed * dt;
            // gravity + jump (cam.pos.y is EYE height)
            let ground_eye = self.ground_height(new_pos) + 1.62;
            if self.keys.contains(&KeyCode::Space)
                && self.vy == 0.0
                && self.cam.pos.y <= ground_eye + 0.06
            {
                self.vy = 4.8;
            }
            self.vy -= 14.0 * dt;
            new_pos.y += self.vy * dt;
            if new_pos.y <= ground_eye {
                new_pos.y = ground_eye;
                self.vy = 0.0;
            }
            self.cam.pos = self.constrain(new_pos, self.cam.pos);

            // bobbing
            if moving && self.vy == 0.0 {
                self.bob_phase += dt * (speed * 1.9);
            }

            // dig (LMB)
            if self.lmb {
                let d = self.cam.fwd();
                if let Some((hit, removed)) = self.game.dig(&mut self.world, self.cam.pos, d) {
                    self.dig_anim = 1.0;
                    let mut rng = Rng::new((self.time * 1e6) as u64 | 1);
                    let to_player = (self.cam.pos - hit).normalize_or_zero();
                    for p in removed.iter().take(48) {
                        if self.tossed.len() >= 3000 {
                            break;
                        }
                        let jitter = Vec3::new(
                            rng.f32b(-1.0, 1.0),
                            rng.f32b(0.8, 2.6),
                            rng.f32b(-1.0, 1.0),
                        ) * 2.4;
                        self.tossed.push(Tossed {
                            pos: *p,
                            vel: jitter + to_player * rng.f32b(0.5, 2.2),
                            spin: rng.f32() * 10.0,
                            life: rng.f32b(0.9, 1.5),
                        });
                    }
                    if self.clumps.len() > 24 {
                        self.clumps.remove(0);
                    }
                    self.clumps.push(hit);
                }
            }
            self.dig_anim = (self.dig_anim - dt * 4.5).max(0.0);
        } else {
            self.lmb = false;
        }

        // game logic (automation, detector, toasts)
        self.game.update(dt, &mut self.world, self.cam.pos, self.cam.fwd());

        // tossed straws physics
        for t in self.tossed.iter_mut() {
            t.vel.y -= 9.0 * dt;
            t.pos += t.vel * dt;
            if t.pos.y < 0.06 {
                t.pos.y = 0.06;
                t.vel.y = -t.vel.y * 0.25;
                t.vel.x *= 0.55;
                t.vel.z *= 0.55;
            }
            t.life -= dt;
            t.spin += dt * 8.0;
        }
        self.tossed.retain(|t| t.life > 0.0);

        // belt balls
        let belt_len = (BELT_P1 - BELT_P0).length();
        let ball_count = 10
            + if self.game.owned[7] { 8 } else { 0 }
            + if self.game.owned[8] { 12 } else { 0 }
            + if self.game.has(19) { 10 } else { 0 };
        while self.balls.len() < ball_count.min(40) {
            self.balls.push(Ball { t: 0.0, speed: 0.14, offset: 0.0 });
        }
        self.balls.truncate(40);
        let belt_active = self.game.has(0); // Conveyor Plans
        for b in self.balls.iter_mut() {
            let spd = if self.game.has(4) { b.speed * 1.6 } else { b.speed };
            if belt_active || self.game.auto_rate() > 0.0 {
                b.t += spd * dt / belt_len.max(1.0);
            }
            if b.t >= 1.0 {
                b.t = 0.0;
            }
        }
        // perimeter ring belt balls (closed loop around the yard)
        let ring_active = belt_active || self.game.auto_rate() > 0.0;
        while self.loop_balls.len() < 12 {
            let i = self.loop_balls.len() as f32;
            self.loop_balls.push(Ball {
                t: i / 12.0,
                speed: 0.11,
                offset: (i * 2.3).sin() * 0.04,
            });
        }
        for b in self.loop_balls.iter_mut() {
            if ring_active {
                let spd = if self.game.has(4) { b.speed * 1.6 } else { b.speed };
                b.t += spd * dt;
            }
            if b.t >= 1.0 {
                b.t -= 1.0;
            }
        }
        let _ = moving;
    }

    /// Position on the perimeter belt loop (closed polyline), t in [0,1).
    fn ring_point(t: f32) -> Vec3 {
        const RING: [(f32, f32); 17] = crate::mesh::BELT_RING;
        let mut total = 0.0;
        let mut lens = [0.0f32; 17];
        for i in 0..RING.len() {
            let a = RING[i];
            let b = RING[(i + 1) % RING.len()];
            let d = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            lens[i] = d;
            total += d;
        }
        let mut dist = t.fract() * total;
        for i in 0..RING.len() {
            if dist <= lens[i] {
                let a = RING[i];
                let b = RING[(i + 1) % RING.len()];
                let f = if lens[i] > 0.0 { dist / lens[i] } else { 0.0 };
                return Vec3::new(
                    a.0 + (b.0 - a.0) * f,
                    crate::mesh::BELT_H + 0.10,
                    a.1 + (b.1 - a.1) * f,
                );
            }
            dist -= lens[i];
        }
        Vec3::new(RING[0].0, crate::mesh::BELT_H + 0.10, RING[0].1)
    }

    fn ground_height(&self, p: Vec3) -> f32 {
        let r = (p.x * p.x + p.z * p.z).sqrt();
        if r < PILE_R {
            let t = (1.0 - (r / PILE_R).powi(2)).max(0.0).sqrt();
            t * PILE_H
        } else {
            0.0
        }
    }

    fn constrain(&self, new_pos: Vec3, old: Vec3) -> Vec3 {
        let mut p = new_pos;
        if !Self::pos_valid(p) {
            let try_x = Vec3::new(new_pos.x, old.y, old.z);
            let try_z = Vec3::new(old.x, old.y, new_pos.z);
            if Self::pos_valid(try_x) {
                p = try_x;
            } else if Self::pos_valid(try_z) {
                p = try_z;
            } else {
                p = old;
            }
        }
        // solid props
        const BOXES: [(Vec3, Vec3); 13] = [
            // sell stall body
            (Vec3::new(17.9, 0.0, 3.9), Vec3::new(20.1, 3.0, 8.1)),
            // blackboard
            (Vec3::new(16.9, 0.0, 4.1), Vec3::new(17.5, 1.3, 5.9)),
            // tools shop
            (Vec3::new(-16.2, 0.0, -15.2), Vec3::new(-11.8, 3.0, -10.8)),
            // door-side shop belt
            (Vec3::new(-11.2, 0.0, -12.8), Vec3::new(-8.2, 1.2, -12.0)),
            // steam boiler
            (Vec3::new(-22.6, 0.0, -12.2), Vec3::new(-20.6, 2.5, -9.8)),
            // tube launcher
            (Vec3::new(-21.0, 0.0, 13.6), Vec3::new(-18.6, 3.5, 16.0)),
            // hay wrapper
            (Vec3::new(-20.7, 0.0, -7.1), Vec3::new(-18.3, 1.9, -4.9)),
            // vacuum
            (Vec3::new(-19.7, 0.0, 11.3), Vec3::new(-17.3, 2.6, 13.7)),
            // baler row
            (Vec3::new(15.5, 0.0, -2.6), Vec3::new(17.7, 2.3, -0.4)),
            (Vec3::new(15.3, 0.0, 1.9), Vec3::new(17.5, 2.3, 4.1)),
            (Vec3::new(15.9, 0.0, 7.4), Vec3::new(18.1, 2.3, 9.6)),
            // water trough
            (Vec3::new(-14.6, 0.0, 15.8), Vec3::new(-8.4, 0.7, 16.8)),
            // market stand
            (Vec3::new(-17.3, 0.0, 12.7), Vec3::new(-14.7, 2.4, 15.3)),
        ];
        for (mn, mx) in BOXES {
            if p.x > mn.x - 0.35
                && p.x < mx.x + 0.35
                && p.z > mn.z - 0.35
                && p.z < mx.z + 0.35
                && p.y < mx.y
            {
                let dxp = (mx.x + 0.35) - p.x;
                let dxn = p.x - (mn.x - 0.35);
                let dzp = (mx.z + 0.35) - p.z;
                let dzn = p.z - (mn.z - 0.35);
                let m = dxp.min(dxn).min(dzp).min(dzn);
                if m == dxp {
                    p.x = mx.x + 0.35;
                } else if m == dxn {
                    p.x = mn.x - 0.35;
                } else if m == dzp {
                    p.z = mx.z + 0.35;
                } else {
                    p.z = mn.z - 0.35;
                }
            }
        }
        p
    }

    fn pos_valid(p: Vec3) -> bool {
        // inside barn
        if p.x.abs() <= BARN_X - 0.8 && p.z.abs() <= BARN_Z - 0.8 {
            return true;
        }
        // door slot
        if p.x.abs() <= 2.5 && p.z.abs() <= BARN_Z + 1.5 {
            return true;
        }
        // outside apron
        if p.x.abs() <= BARN_X + 7.5 && p.z.abs() <= BARN_Z + 7.5 {
            if p.x.abs() >= BARN_X + 0.8 || p.z.abs() >= BARN_Z + 0.8 {
                return true;
            }
        }
        false
    }

    // ----------------------------------------------------- instance build ----

    fn collect_instances(
        &mut self,
    ) -> (Vec<StrawInst>, Vec<PropInst>, Vec<(usize, u32, u32)>) {
        let mut prop_insts: Vec<PropInst> = Vec::with_capacity(256);
        let mut batches: Vec<(usize, u32, u32)> = Vec::new();

        fn push_batch(
            prop_insts: &Vec<PropInst>,
            batches: &mut Vec<(usize, u32, u32)>,
            mesh_id: usize,
            count: usize,
        ) {
            if count > 0 {
                batches.push((mesh_id, (prop_insts.len() - count) as u32, count as u32));
            }
        }

        // haystack dark core (fills gaps between straws so the pile reads solid)
        {
            let m = Mat4::from_scale(Vec3::new(crate::world::PILE_R, crate::world::PILE_H, crate::world::PILE_R));
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            push_batch(&prop_insts, &mut batches, 0, 1);
        }
        // needle
        {
            let n = &self.world.needle;
            let m = Mat4::from_rotation_y(n.yaw) * Mat4::from_translation(n.pos);
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            push_batch(&prop_insts, &mut batches, 1, 1);
        }
        // valuables grouped per kind
        {
            let mut by_kind: Vec<(ValuableKind, Vec<usize>)> = Vec::new();
            for (i, v) in self.world.valuables.iter().enumerate() {
                if v.taken {
                    continue;
                }
                match by_kind.iter_mut().find(|(k, _)| *k == v.kind) {
                    Some((_, list)) => list.push(i),
                    None => by_kind.push((v.kind, vec![i])),
                }
            }
            for (k, idxs) in by_kind {
                let first = prop_insts.len();
                for i in idxs {
                    let v = &self.world.valuables[i];
                    let m = Mat4::from_rotation_y(v.yaw) * Mat4::from_translation(v.pos);
                    prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.55]));
                }
                push_batch(&prop_insts, &mut batches, k.mesh_index(), prop_insts.len() - first);
            }
        }
        // machines: the full yard fleet, ALWAYS visible (like the reference
        // screenshots); unlocked machines pulse via the working flag (0.6),
        // idle ones render with the standard material (0.0).
        {
            const ON: f32 = 0.6;
            const OFF: f32 = 0.0;
            // 3 hay balers on the east row (shot 3: baler cluster by the belts)
            const BALERS: [(Vec3, f32); 3] = [
                (Vec3::new(16.6, 0.0, -1.5), -1.35),
                (Vec3::new(16.4, 0.0, 3.0), -1.20),
                (Vec3::new(17.0, 0.0, 8.5), -0.75),
            ];
            for (p, yaw) in BALERS {
                let m = Mat4::from_translation(p) * Mat4::from_rotation_y(yaw);
                let f = if self.game.owned[7] { ON } else { OFF };
                prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            }
            push_batch(&prop_insts, &mut batches, 13, 3);
            // vacuum line unit (west)
            let m = Mat4::from_translation(Vec3::new(-18.5, 0.0, 12.5))
                * Mat4::from_rotation_y(0.8);
            let f = if self.game.owned[8] { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 14, 1);
            // hay wrapper on its west pad
            let m = Mat4::from_translation(Vec3::new(-19.5, 0.0, -6.0))
                * Mat4::from_rotation_y(1.35);
            let f = if self.game.has(18) { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 21, 1);
            // robot arm fleet ringing the pile along the belt network
            // (Extra Arm Batch adds 4 arms per level between the base ones)
            const ARMS: [(f32, f32); 14] = [
                (5.0, 19.5), (35.0, 17.2), (55.0, 19.0), (125.0, 19.6), (135.0, 17.5),
                (155.0, 18.2), (175.0, 18.0), (195.0, 18.0), (215.0, 17.6), (235.0, 17.0),
                (248.0, 16.5), (305.0, 19.4), (325.0, 19.3), (345.0, 19.3),
            ];
            let f = if self.game.has(19) { ON } else { OFF };
            let first = prop_insts.len();
            let extra = if self.game.has(19) { 4 * self.game.lv(60) } else { 0 };
            let total_arms = ARMS.len() + extra as usize;
            for k in 0..total_arms {
                let (deg, r) = if k < ARMS.len() {
                    ARMS[k]
                } else {
                    // extra arms interleave on the ring
                    let j = k - ARMS.len();
                    let a0 = ARMS[j % ARMS.len()].0;
                    let a1 = ARMS[(j + 1) % ARMS.len()].0;
                    let mid = if a1 > a0 { (a0 + a1) * 0.5 } else { (a0 + a1 + 360.0) * 0.5 };
                    (mid % 360.0, 18.2)
                };
                let a = deg.to_radians();
                let p = Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                let m = Mat4::from_translation(p)
                    * Mat4::from_rotation_y(-a + std::f32::consts::FRAC_PI_2);
                prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            }
            push_batch(&prop_insts, &mut batches, 17, prop_insts.len() - first);
            // quality scanner arch straddling the east belt run
            let m = Mat4::from_translation(Vec3::new(18.25, 0.0, 0.5))
                * Mat4::from_rotation_y(0.0995);
            let f = if self.game.has(17) { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 18, 1);
            // scout drone(s) circling the pile (Drone Fleet adds 2 per level)
            let drone_count = if self.game.has(21) { 1 + 2 * self.game.lv(62) } else { 0 };
            for di in 0..drone_count {
                let t = self.time as f32 + di as f32 * 2.1;
                let p = Vec3::new(
                    (t * 0.25).cos() * (7.0 + di as f32 * 0.9),
                    7.6 + (t * 1.1).sin() * 0.5,
                    (t * 0.25).sin() * (7.0 + di as f32 * 0.9),
                );
                let m = Mat4::from_translation(p) * Mat4::from_rotation_y(t * 1.7);
                prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            }
            if drone_count > 0 {
                push_batch(&prop_insts, &mut batches, 22, drone_count as usize);
            }
            // ---- v3 machines (mirroring the real late-game yard, shot 3) ----
            // mechanical sorter at the south belt junction
            let m = Mat4::from_translation(Vec3::new(6.5, 0.0, 15.0))
                * Mat4::from_rotation_y(-0.5);
            let f = if self.game.has(47) { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 23, 1);
            // vertical elevator tower east of the pile (black tower in shot 3)
            let m = Mat4::from_translation(Vec3::new(13.5, 0.0, 8.0))
                * Mat4::from_rotation_y(2.35);
            let f = if self.game.has(8) { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 24, 1);
            // sale truck parked at the SW gate corner
            let m = Mat4::from_translation(Vec3::new(-16.5, 0.0, -12.0))
                * Mat4::from_rotation_y(0.75);
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            push_batch(&prop_insts, &mut batches, 25, 1);
            // the silo on the far east pad
            let m = Mat4::from_translation(Vec3::new(19.5, 0.0, 12.5));
            let f = if self.game.has(57) { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 26, 1);
            // eco brick press next to the baler row
            let m = Mat4::from_translation(Vec3::new(15.8, 0.0, -6.5))
                * Mat4::from_rotation_y(-1.2);
            let f = if self.game.has(54) { ON } else { OFF };
            prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, f]));
            push_batch(&prop_insts, &mut batches, 27, 1);
        }
        // belt balls
        if self.game.has(0) || self.game.auto_rate() > 0.0 {
            let first = prop_insts.len();
            for b in &self.balls {
                let p = BELT_P0.lerp(BELT_P1, b.t) + Vec3::new(0.0, 0.10 + b.offset, 0.0);
                let m = Mat4::from_translation(p)
                    * Mat4::from_rotation_y(self.time as f32 * 2.0);
                prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            }
            // perimeter ring belt balls
            for b in &self.loop_balls {
                let p = Self::ring_point(b.t) + Vec3::new(0.0, b.offset, 0.0);
                let m = Mat4::from_translation(p)
                    * Mat4::from_rotation_y(self.time as f32 * 2.0);
                prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            }
            push_batch(&prop_insts, &mut batches, 19, prop_insts.len() - first);
        }
        // dig clumps (hay crumbs)
        if !self.clumps.is_empty() {
            let first = prop_insts.len();
            for (i, c) in self.clumps.iter().enumerate() {
                let m = Mat4::from_translation(
                    *c + Vec3::new(0.0, 0.03 + (i % 3) as f32 * 0.01, 0.0),
                ) * Mat4::from_scale(Vec3::splat(1.4 + (i % 4) as f32 * 0.2));
                prop_insts.push(PropInst::from_mat(m, [1.0, 1.0, 1.0, 0.0]));
            }
            push_batch(&prop_insts, &mut batches, 20, prop_insts.len() - first);
        }
        // viewmodel
        self.viewmodel(&mut prop_insts, &mut batches);

        // dynamic straws (tossed)
        let mut straw_dyn: Vec<StrawInst> = Vec::with_capacity(self.tossed.len());
        for t in &self.tossed {
            let fade = (t.life / 0.4).min(1.0);
            let yaw = t.spin.rem_euclid(std::f32::consts::TAU);
            let yb = (yaw / std::f32::consts::TAU * 4095.0) as u32 & 0xFFF;
            let pb = 512u32 << 12; // mid pitch
            let lb = 32u32 << 22; // mid length
            straw_dyn.push(StrawInst {
                pos: [t.pos.x, t.pos.y, t.pos.z, 0.9 * fade],
                data: [yb | pb | lb, 128, 0, 1], // kind TOSS
            });
        }
        (straw_dyn, prop_insts, batches)
    }

    fn viewmodel(&mut self, prop_insts: &mut Vec<PropInst>, batches: &mut Vec<(usize, u32, u32)>) {
        if self.game.state != GameState::Playing {
            return;
        }
        let cam_m = Mat4::from_translation(self.cam.pos)
            * Mat4::from_rotation_y(self.cam.yaw)
            * Mat4::from_rotation_x(self.cam.pitch);
        let bob = self.bob_phase.sin() * 0.012;
        let dig = (self.dig_anim * std::f32::consts::PI).sin() * 0.24;
        let t = self.time as f32;

        match self.game.current_tool() {
            crate::game::Tool::Hands | crate::game::Tool::Gloves => {
                if self.game.carried > 0 {
                    let local = Mat4::from_translation(Vec3::new(-0.34, -0.36 + bob, -0.58))
                        * Mat4::from_rotation_x(0.25)
                        * Mat4::from_scale(Vec3::splat(0.9));
                    let first = prop_insts.len();
                    prop_insts.push(PropInst::from_mat(cam_m * local, [1.0, 1.0, 1.0, 0.0]));
                    batches.push((16, first as u32, 1));
                }
            }
            crate::game::Tool::Pitchfork | crate::game::Tool::Shovel => {
                let sway = t.sin() * 0.02;
                let local = Mat4::from_translation(Vec3::new(0.26, -0.40 + bob, -0.62 + dig))
                    * Mat4::from_rotation_x(-0.55 + dig * 0.8)
                    * Mat4::from_rotation_z(sway)
                    * Mat4::from_scale(Vec3::splat(0.62));
                let first = prop_insts.len();
                prop_insts.push(PropInst::from_mat(cam_m * local, [1.0, 1.0, 1.0, 0.0]));
                batches.push((15, first as u32, 1));
                if self.game.carried > 0 {
                    let local = Mat4::from_translation(Vec3::new(-0.34, -0.36 + bob * 0.8, -0.58))
                        * Mat4::from_rotation_x(0.25)
                        * Mat4::from_scale(Vec3::splat(0.9));
                    let first = prop_insts.len();
                    prop_insts.push(PropInst::from_mat(cam_m * local, [1.0, 1.0, 1.0, 0.0]));
                    batches.push((16, first as u32, 1));
                }
            }
            crate::game::Tool::Detector | crate::game::Tool::DetectorII => {
                let sway = (t * 2.4).sin() * 0.16;
                let pulse = if self.game.detector_hot {
                    (self.game.detector_pulse * 10.0).sin().abs()
                } else {
                    0.0
                };
                let local = Mat4::from_translation(Vec3::new(0.30, -0.34 + bob, -0.55))
                    * Mat4::from_rotation_x(-0.35)
                    * Mat4::from_rotation_y(sway)
                    * Mat4::from_rotation_z(-0.1 + pulse * 0.12)
                    * Mat4::from_scale(Vec3::splat(0.85));
                let first = prop_insts.len();
                prop_insts.push(PropInst::from_mat(cam_m * local, [1.0, 1.0, 1.0, 0.0]));
                batches.push((2, first as u32, 1));
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------- HUD ------

    fn build_hud(&mut self) {
        let mut hud = Hud::new();
        let (w, h) = self.win_size();
        let g = &self.game;

        match g.state {
            GameState::Menu => self.hud_menu(&mut hud, w, h),
            GameState::Playing => self.hud_game(&mut hud, w, h),
            GameState::Shop => self.hud_shop(&mut hud, w, h),
            GameState::Research => self.hud_research(&mut hud, w, h),
            GameState::Paused => self.hud_pause(&mut hud, w, h),
            GameState::Won => self.hud_won(&mut hud, w, h),
        }

        if self.show_fps {
            hud.text(
                10.0,
                h - 22.0,
                2.0,
                [0.2, 1.0, 0.35, 0.9],
                &format!(
                    "FPS:{} {} STRAWS SCALE:{}%",
                    self.fps_value,
                    self.world.straw_count,
                    (self.settings.render_scale * 100.0) as u32
                ),
            );
        }

        self.pending_hud = hud.quads;
    }

    const GOLD: [f32; 4] = [0.96, 0.78, 0.25, 1.0];
    const WHITE: [f32; 4] = [0.94, 0.94, 0.92, 1.0];
    const DIM: [f32; 4] = [0.62, 0.60, 0.55, 1.0];
    const GREEN: [f32; 4] = [0.45, 0.85, 0.40, 1.0];

    fn hud_game(&self, hud: &mut Hud, w: f32, h: f32) {
        let g = &self.game;
        // money top-right (gold, like $13,976 in the real tech tree screen)
        let money_s = format_money(g.money);
        let mw = Hud::text_w(&money_s, 3.2);
        hud.text(w - mw - 18.0, 14.0, 3.2, Self::GOLD, &money_s);
        if g.carried > 0 {
            let s = format!("{} STRANDS", g.carried);
            hud.text(w - Hud::text_w(&s, 1.8) - 20.0, 44.0, 1.8, Self::DIM, &s);
        }

        // tool top-left
        let tool = TOOLS[g.selected].name;
        hud.text(14.0, 14.0, 2.0, Self::WHITE, tool);

        // crosshair
        hud.rect(w / 2.0 - 1.5, h / 2.0 - 1.5, 3.0, 3.0, [1.0, 1.0, 1.0, 0.75]);

        // detector radius text (top-center) + hot pulse ring
        if let Some(r) = g.detector_radius() {
            let s = format!("RADIUS - {}M", r as u32);
            hud.text_center(w / 2.0, 16.0, 2.0, Self::DIM, &s);
            if g.detector_hot {
                let pulse = (g.detector_pulse * 12.0).rem_euclid(1.0);
                hud.ring(w / 2.0, h / 2.0, 12.0 + pulse * 16.0, 2.5, [1.0, 0.85, 0.2]);
            }
        }

        // carried counter above the bucket (like "485 / 600")
        if g.carried > 0 {
            let s = format!("{} / {}", g.carried, g.capacity());
            let sc = 2.6;
            let cx = w / 2.0;
            let y = h * 0.60;
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                hud.text_center(cx + dx, y + dy, sc, [0.0, 0.0, 0.0, 0.85], &s);
            }
            hud.text_center(cx, y, sc, Self::WHITE, &s);
        }

        // interact prompts
        let p = Vec3::new(self.cam.pos.x, 0.0, self.cam.pos.z);
        let near_stall = (p - STALL_FRONT).length() < 3.4;
        let near_shop = (p - SHOP_POS).length() < 4.2;
        if near_stall {
            hud.text_center(w / 2.0, h * 0.72, 2.2, Self::GOLD, "E - SELL HAY ($0.022 PER STRAND)");
        } else if near_shop {
            hud.text_center(w / 2.0, h * 0.72, 2.2, Self::GOLD, "E - BROWSE TOOLS");
        }

        // toasts bottom-left
        for (i, t) in g.toasts.iter().enumerate() {
            let a = t.life.min(1.0);
            hud.text(14.0, h - 60.0 - i as f32 * 20.0, 2.0, [1.0, 0.9, 0.5, a], &t.text);
        }

        // hints (first 12 seconds)
        if g.time < 12.0 {
            hud.text_center(
                w / 2.0,
                h - 30.0,
                1.8,
                Self::DIM,
                "WASD MOVE - SHIFT RUN - SPACE JUMP - LMB DIG - RMB ZOOM - TAB RESEARCH - E INTERACT",
            );
        }
    }

    fn hud_menu(&self, hud: &mut Hud, w: f32, h: f32) {
        let g = &self.game;
        hud.rect(0.0, 0.0, w, h, [0.05, 0.04, 0.03, 0.72]);
        let title = "FIND THE NEEDLE";
        let sc = (w / Hud::text_w(title, 6.0)).min(7.0);
        hud.text_center(w / 2.0, h * 0.14, sc, Self::GOLD, title);
        hud.text_center(
            w / 2.0,
            h * 0.14 + sc * 8.0 + 6.0,
            2.0,
            Self::DIM,
            "SOMEWHERE IN THIS PILE IS A SINGLE SEWING NEEDLE. FIND IT.",
        );

        let base_y = h * 0.38;
        for i in 0..7 {
            let sel = g.menu_index == i;
            let label = match i {
                1 => {
                    if std::path::Path::new("save.bin").exists() {
                        "CONTINUE".to_string()
                    } else {
                        "CONTINUE (NO SAVE)".to_string()
                    }
                }
                2 => {
                    if g.pure_mode {
                        "PURE MODE: ON".to_string()
                    } else {
                        "PURE MODE: OFF".to_string()
                    }
                }
                3 => {
                    if g.menu_seed_input {
                        format!("SEED: {}_", g.seed_input)
                    } else {
                        "SET SEED".to_string()
                    }
                }
                4 => format!("QUALITY: {}", self.settings.quality.name()),
                5 => format!("VSYNC: {}", if self.settings.vsync { "ON" } else { "OFF" }),
                _ => ["PLAY", "CONTINUE", "", "", "", "", "QUIT"][i].to_string(),
            };
            let color = if sel { Self::GOLD } else { Self::WHITE };
            if sel {
                hud.text_center(
                    w / 2.0,
                    base_y + i as f32 * 34.0,
                    3.0,
                    [0.9, 0.7, 0.2, 0.9],
                    &format!("> {} <", label),
                );
            } else {
                hud.text_center(w / 2.0, base_y + i as f32 * 34.0, 3.0, color, &label);
            }
        }
        hud.text_center(
            w / 2.0,
            h - 46.0,
            1.8,
            Self::DIM,
            "ARROWS TO MOVE - ENTER SELECT",
        );

        for (i, t) in g.toasts.iter().enumerate() {
            let a = t.life.min(1.0);
            hud.text_center(w / 2.0, h - 92.0 - i as f32 * 18.0, 1.8, [1.0, 0.9, 0.5, a], &t.text);
        }
    }

    fn hud_shop(&self, hud: &mut Hud, w: f32, h: f32) {
        let g = &self.game;
        hud.rect(0.0, 0.0, w, h, [0.06, 0.05, 0.04, 0.78]);
        hud.text_center(w / 2.0, h * 0.10, 4.0, Self::GOLD, "HAYWAY CO. - TOOLS");
        let money_s = format_money(g.money);
        let mw = Hud::text_w(&money_s, 2.6);
        hud.text(w - mw - 18.0, 14.0, 2.6, Self::GOLD, &money_s);

        let y0 = h * 0.24;
        for (i, def) in TOOLS.iter().enumerate() {
            let sel = g.shop_index == i;
            let y = y0 + i as f32 * 40.0;
            let owned = def.price == 0 || g.owned[i];
            let equipped = g.selected == i;
            let name_col = if sel { Self::GOLD } else { Self::WHITE };
            hud.text(w * 0.18, y, 2.2, name_col, def.name);
            hud.text(w * 0.45, y + 2.0, 1.6, Self::DIM, def.desc);
            let right = if equipped {
                "EQUIPPED".to_string()
            } else if owned {
                "OWNED".to_string()
            } else {
                format_money(def.price)
            };
            let col = if equipped {
                Self::DIM
            } else if owned {
                Self::WHITE
            } else {
                Self::GREEN
            };
            hud.text(w * 0.78, y, 2.2, col, &right);
            if sel {
                hud.rect(w * 0.17, y - 6.0, w * 0.66, 34.0, [1.0, 0.8, 0.25, 0.10]);
            }
        }
        hud.text_center(w / 2.0, h - 34.0, 1.8, Self::DIM, "UP/DOWN - ENTER BUY/EQUIP - E CLOSE");
    }

    fn card_at_cursor(&self) -> Option<usize> {
        let cat = crate::game::RCategory::ALL[self.game.research_cat];
        let cards: Vec<usize> = RESEARCH
            .iter()
            .enumerate()
            .filter(|(_, d)| d.cat == cat)
            .map(|(i, _)| i)
            .collect();
        cards.get(self.game.ui_index).copied()
    }

    fn hud_research(&self, hud: &mut Hud, w: f32, h: f32) {
        let g = &self.game;
        hud.rect(0.0, 0.0, w, h, [0.03, 0.03, 0.035, 0.88]);
        hud.text(16.0, 12.0, 3.0, Self::GOLD, "YARD RESEARCH");
        hud.rect(w * 0.28, 12.0, w * 0.22, 26.0, [0.12, 0.12, 0.13, 0.9]);
        hud.text(w * 0.28 + 8.0, 18.0, 1.6, [0.4, 0.4, 0.42, 1.0], "SEARCH THE TREE");
        let money_s = format_money(g.money);
        let mw = Hud::text_w(&money_s, 3.0);
        hud.text(w - mw - 18.0, 12.0, 3.0, Self::GOLD, &money_s);

        // left category column
        let y0 = h * 0.14;
        for (i, cat) in crate::game::RCategory::ALL.iter().enumerate() {
            let sel = g.research_cat == i;
            let y = y0 + i as f32 * 52.0;
            if y > h - 40.0 {
                break;
            }
            if sel {
                hud.rect(w * 0.012, y - 8.0, w * 0.20, 50.0, [0.96, 0.78, 0.25, 0.10]);
            }
            let col = if sel { Self::GOLD } else { Self::WHITE };
            hud.text(w * 0.018, y, 2.0, col, cat.name());
            for (li, line) in cat.hint().lines().enumerate() {
                hud.text(w * 0.018, y + 16.0 + li as f32 * 12.0, 1.4, Self::DIM, line);
            }
        }

        // cards grid for the selected category (tier columns like the real tree)
        let cat = crate::game::RCategory::ALL[g.research_cat];
        let cards: Vec<usize> = RESEARCH
            .iter()
            .enumerate()
            .filter(|(_, d)| d.cat == cat)
            .map(|(i, _)| i)
            .collect();
        let max_tier = cards.iter().map(|&i| RESEARCH[i].tier).max().unwrap_or(0);
        let gx0 = w * 0.26;
        let gy0 = h * 0.18;
        let col_w = w * 0.115;
        for t in 0..=max_tier {
            let label = if t == 0 {
                "START".to_string()
            } else {
                format!("{} STEP{}", t, if t > 1 { "S" } else { "" })
            };
            hud.text(gx0 + t as f32 * col_w, gy0 - 20.0, 1.5, Self::DIM, &label);
        }
        let mut tier_counts = [0usize; 8];
        let sel_idx = g.ui_index % cards.len().max(1);
        for (ci, &ri) in cards.iter().enumerate() {
            let d = &RESEARCH[ri];
            let col = d.tier as usize;
            let row = tier_counts[col];
            tier_counts[col] += 1;
            let x = gx0 + col as f32 * col_w;
            let y = gy0 + row as f32 * 96.0;
            if y > h - 70.0 {
                continue;
            }
            let sel = ci == sel_idx;
            let cur = g.lv(ri as u32);
            let maxed = cur >= d.levels;
            hud.rect(x, y, col_w - 14.0, 82.0, [0.10, 0.085, 0.06, 0.96]);
            hud.rect(x, y, col_w - 14.0, 16.0, [0.85, 0.68, 0.22, 0.35]);
            hud.text(x + 6.0, y + 2.0, 1.2, Self::GOLD, cat.name());
            hud.text(x + 6.0, y + 22.0, 1.6, Self::WHITE, d.name);
            let locked = d.requires != u32::MAX && !g.has(d.requires);
            // level pips like the real tree ("0/6")
            let lv_label = format!("{}/{}", cur, d.levels);
            hud.text(
                x + col_w - 40.0,
                y + 2.0,
                1.4,
                if maxed { Self::GOLD } else { Self::DIM },
                &lv_label,
            );
            if maxed {
                hud.text(x + 6.0, y + 44.0, 1.6, Self::GOLD, "MAXED");
                hud.text(x + 6.0, y + 60.0, 1.4, Self::DIM, "DONE");
            } else if locked {
                hud.text(x + 6.0, y + 44.0, 1.5, [0.7, 0.3, 0.25, 1.0], "LOCKED");
            } else {
                let price = price_for_level(d, cur);
                hud.text(x + 6.0, y + 44.0, 1.6, Self::GREEN, &format_money(price));
                hud.rect(x + 6.0, y + 60.0, 40.0, 14.0, [0.9, 0.75, 0.3, 0.25]);
                hud.text(x + 10.0, y + 62.0, 1.3, Self::GOLD, "SHOW");
            }
            if sel {
                hud.rect(x - 3.0, y - 3.0, col_w - 8.0, 2.0, Self::GOLD);
                hud.rect(x - 3.0, y + 83.0, col_w - 8.0, 2.0, Self::GOLD);
                hud.rect(x - 3.0, y - 3.0, 2.0, 88.0, Self::GOLD);
                hud.rect(x + col_w - 17.0, y - 3.0, 2.0, 88.0, Self::GOLD);
            }
        }

        hud.text_center(
            w / 2.0,
            h - 26.0,
            1.7,
            Self::DIM,
            &format!(
                "{} OF {} LEVELS BOUGHT - ARROWS TO MOVE - ENTER TO BUY - TAB TO CLOSE",
                g.research_count(),
                total_levels()
            ),
        );
    }

    fn hud_pause(&self, hud: &mut Hud, w: f32, h: f32) {
        hud.rect(0.0, 0.0, w, h, [0.03, 0.03, 0.03, 0.55]);
        hud.text_center(w / 2.0, h * 0.42, 6.0, Self::WHITE, "PAUSED");
        hud.text_center(w / 2.0, h * 0.42 + 64.0, 2.2, Self::DIM, "ESC - RESUME   Q - QUIT TO MENU");
        hud.text_center(w / 2.0, h * 0.42 + 92.0, 2.2, Self::DIM, "F5 - SAVE   F9 - LOAD   F3 - FPS");
    }

    fn hud_won(&self, hud: &mut Hud, w: f32, h: f32) {
        hud.rect(0.0, 0.0, w, h, [0.02, 0.02, 0.02, 0.72]);
        hud.text_center(w / 2.0, h * 0.34, 5.0, Self::GOLD, "YOU FOUND THE NEEDLE!");
        hud.text_center(
            w / 2.0,
            h * 0.34 + 54.0,
            2.6,
            Self::WHITE,
            &format!("TIME: {}", format_time(self.game.won_time)),
        );
        hud.text_center(
            w / 2.0,
            h * 0.34 + 84.0,
            2.0,
            Self::DIM,
            &format!(
                "STRANDS DUG: {}   EARNED: {}",
                self.game.removed_count,
                format_money(self.game.earned_total)
            ),
        );
        if let Some(b) = self.game.best_time {
            hud.text_center(w / 2.0, h * 0.34 + 110.0, 2.0, Self::DIM, &format!("BEST: {}", format_time(b)));
        }
        hud.text_center(w / 2.0, h * 0.34 + 150.0, 2.2, Self::GOLD, "ENTER - BACK TO MENU");
    }
}
