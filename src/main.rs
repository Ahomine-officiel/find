// Find the Needle - Rust + wgpu remake. Window, input, game loop, co-op.
mod cam;
mod game;
mod gpu;
mod hud;
mod mesh;
mod model;
mod net;
mod render;
mod rng;
mod settings;
mod shaders;
mod ui;
mod world;

use cam::Camera;
use game::{Game, GameState};
use glam::{Mat4, Vec3, Vec4};
use hud::Hud;
use net::{Net, Role};
use render::{PropDraw, Renderer};
use rng::hash_seed;
use settings::Settings;
use std::collections::HashSet;
use std::sync::Arc;
use world::{StrawInst, World, PILE_H, PILE_R};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

// ---------------- tossed straw physics ----------------
struct TossItem {
    alive: bool,
    settled: bool,
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    pitch: f32,
    spin: f32,
    len: f32,
}

struct TossSys {
    items: Vec<TossItem>,
    cap: usize,
    cursor: usize,
}

impl TossSys {
    fn new(cap: usize) -> Self {
        TossSys {
            items: (0..cap)
                .map(|_| TossItem {
                    alive: false,
                    settled: false,
                    pos: Vec3::ZERO,
                    vel: Vec3::ZERO,
                    yaw: 0.0,
                    pitch: 0.0,
                    spin: 0.0,
                    len: 0.3,
                })
                .collect(),
            cap,
            cursor: 0,
        }
    }

    fn spawn_burst(&mut self, origin: Vec3, n: usize, rng: &mut rng::Rng) {
        for _ in 0..n {
            let it = &mut self.items[self.cursor];
            self.cursor = (self.cursor + 1) % self.cap;
            let dir_yaw = rng.f32() * std::f32::consts::TAU;
            let dir = Vec3::new(dir_yaw.sin(), 0.0, dir_yaw.cos());
            it.alive = true;
            it.settled = false;
            it.pos = origin + dir * rng.f32b(0.0, 0.4) + Vec3::Y * rng.f32b(0.05, 0.3);
            it.vel = dir * rng.f32b(1.2, 3.2) + Vec3::Y * rng.f32b(1.8, 3.6);
            it.yaw = rng.f32() * std::f32::consts::TAU;
            it.pitch = rng.f32b(-1.2, 1.2);
            it.spin = rng.f32b(-9.0, 9.0);
            it.len = rng.f32b(0.14, 0.42);
        }
    }

    fn update(&mut self, dt: f32) {
        for it in &mut self.items {
            if !it.alive || it.settled {
                continue;
            }
            it.vel.y -= 9.8 * dt;
            it.pos += it.vel * dt;
            it.pitch += it.spin * dt;
            if it.pos.y <= 0.015 {
                it.pos.y = 0.015;
                if it.vel.y.abs() < 0.6 {
                    it.settled = true;
                    it.pitch = it.pitch.clamp(1.35, 1.57); // lie flat
                } else {
                    it.vel.y *= -0.35;
                    it.vel.x *= 0.6;
                    it.vel.z *= 0.6;
                }
            }
        }
    }

    fn instances(&self) -> Vec<StrawInst> {
        let mut out = Vec::new();
        for it in &self.items {
            if !it.alive {
                continue;
            }
            out.push(StrawInst {
                pos: [it.pos.x, it.pos.y, it.pos.z, 1.12],
                data: [world::pack_straw(it.yaw, it.pitch, it.len), 210, 0, world::STRAW_KIND_TOSS],
            });
        }
        out
    }
}

// ---------------- app state ----------------
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    SeedInput,
    JoinInput,
    HowTo,
}

#[derive(Clone, Copy, PartialEq)]
enum Click {
    Start,
    TogglePure,
    Seed,
    Quality,
    Host,
    Join,
    HowTo,
    Quit,
    Resume,
    ToMenu,
    Buy(usize),
    SellAll,
    Back,
    SaveLoad,
}

pub struct App {
    pub settings: Settings,
    pub window: Option<Arc<Window>>,
    pub surface: Option<wgpu::Surface<'static>>,
    pub renderer: Option<Renderer>,
    pub adapter_info: String,
    pub size: (u32, u32),

    pub world: Option<World>,
    pub game: Option<Game>,
    pub cam: Camera,
    pub hud: Hud,
    pub toss: TossSys,
    pub net: Net,

    pub screen: Screen,
    pub menu_sel: usize,
    pub shop_sel: usize,
    pub pure_mode: bool,
    pub buf: String,
    pub connecting: bool,
    pub connect_t: f32,
    pub hello_t: f32,

    pub keys: HashSet<KeyCode>,
    pub mouse_l: bool,
    pub mouse_r: bool,
    pub grabbed: bool,
    pub walk_t: f32,
    pub fps: f32,
    pub fps_t: f32,
    pub autosave_t: f32,
    pub time: f64,
    pub last: Option<std::time::Instant>,
    pub click: Vec<(f32, f32, f32, f32, Click)>,
    pub cursor: (f32, f32),
    pub toasts: Vec<(String, f32)>,
}

impl App {
    fn new() -> Self {
        let settings = Settings::load();
        App {
            settings,
            window: None,
            surface: None,
            renderer: None,
            adapter_info: String::new(),
            size: (1280, 720),
            world: None,
            game: None,
            cam: Camera::new(settings.fov),
            hud: Hud::new(),
            toss: TossSys::new(3072),
            net: Net::offline(),
            screen: Screen::Main,
            menu_sel: 0,
            shop_sel: 0,
            pure_mode: false,
            buf: String::new(),
            connecting: false,
            connect_t: 0.0,
            hello_t: 0.0,
            keys: HashSet::new(),
            mouse_l: false,
            mouse_r: false,
            grabbed: false,
            walk_t: 0.0,
            fps: 0.0,
            fps_t: 0.0,
            autosave_t: 0.0,
            time: 0.0,
            last: None,
            click: Vec::new(),
            cursor: (640.0, 360.0),
            toasts: Vec::new(),
        }
    }

    pub fn window(&self) -> &Arc<Window> {
        self.window.as_ref().unwrap()
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push((text.into(), 3.2));
        if self.toasts.len() > 7 {
            self.toasts.remove(0);
        }
    }

    fn start_game(&mut self, seed: u64, count: u32, pure: bool) {
        let world = World::new(seed, count);
        self.world = Some(world);
        let mut g = Game::new(seed, pure);
        g.state = GameState::Playing;
        self.game = Some(g);
        self.toss = TossSys::new(3072);
        let r = self.renderer.as_mut().unwrap();
        r.set_straw_instances(self.world.as_ref().unwrap());
        r.apply_quality(&self.settings);
        self.net.set_world_info(seed, count);
        self.set_grab(true);
        self.cam.pos = Vec3::new(0.0, 1.62, PILE_R + 30.0);
        self.cam.yaw = 0.0;
        self.cam.pitch = -0.06;
        self.cam.zoom = 1.0;
        self.toast(format!("GENERATED {} PIECES - GOOD LUCK", count));
    }

    fn to_menu(&mut self) {
        if let (Some(g), Some(w)) = (&self.game, &self.world) {
            let _ = game::save_game(g, w, "save.bin");
        }
        self.set_grab(false);
        if let Some(g) = &mut self.game {
            g.state = GameState::Menu;
        }
        self.game = None;
        self.screen = Screen::Main;
        self.net = Net::offline();
        self.connecting = false;
    }

    fn set_grab(&mut self, on: bool) {
        self.grabbed = on;
        let Some(w) = &self.window else { return };
        if on {
            let _ = w.set_cursor_grab(winit::window::CursorGrabMode::Locked);
            w.set_cursor_visible(false);
        } else {
            let _ = w.set_cursor_grab(winit::window::CursorGrabMode::None);
            w.set_cursor_visible(true);
        }
    }

    fn in_menu(&self) -> bool {
        self.game.is_none() && !self.connecting
    }

    // ---------------- per-frame update ----------------
    fn update(&mut self, dt: f32) {
        self.time += dt as f64;
        self.fps_t += dt;
        if self.fps_t > 0.25 {
            self.fps = 1.0 / dt.max(1e-4);
            self.fps_t = 0.0;
        }
        for t in self.toasts.iter_mut() {
            t.1 -= dt;
        }
        self.toasts.retain(|t| t.1 > 0.0);

        // menu orbit camera
        if self.in_menu() {
            let a = (self.time * 0.05) as f32;
            self.cam.pos = Vec3::new(a.sin() * 62.0, 16.0, a.cos() * 62.0);
            self.cam.yaw = a;
            self.cam.pitch = -0.22;
        }

        // co-op connecting
        if self.connecting {
            self.connect_t += dt;
            self.hello_t -= dt;
            if self.hello_t <= 0.0 {
                self.net.send_hello();
                self.hello_t = 0.5;
            }
            self.net.poll(dt);
            if let Some((seed, count)) = self.net_connect_ready() {
                self.connecting = false;
                self.start_game(seed, count, false);
                self.toast("CONNECTED! FIND IT TOGETHER.");
            } else if self.connect_t > 10.0 {
                self.connecting = false;
                self.net = Net::offline();
                self.screen = Screen::Main;
                self.toast("CONNECTION FAILED: NO HOST ANSWERED");
            }
            return;
        }

        self.net.poll(dt);

        // remote co-op events (before local logic)
        let mut pending_toasts: Vec<String> = Vec::new();
        if self.net.role != Role::Offline && self.world.is_some() {
            let mut idxs = std::mem::take(&mut self.net.remote.pending_digs);
            if !idxs.is_empty() {
                let mut applied = Vec::new();
                {
                    let w = self.world.as_mut().unwrap();
                    for i in idxs.drain(..) {
                        if w.mark_removed(i) {
                            applied.push(i);
                        }
                    }
                }
                if !applied.is_empty() {
                    self.renderer.as_mut().unwrap().update_straw_states(&applied);
                    let mut rng = rng::Rng::new((self.time * 777.0) as u64 | 1);
                    let w = self.world.as_ref().unwrap();
                    for &i in &applied {
                        let p = w.straws[i as usize].pos;
                        self.toss.spawn_burst(Vec3::new(p[0], p[1], p[2]), 2, &mut rng);
                    }
                }
            }
            {
                let w = self.world.as_mut().unwrap();
                for vi in std::mem::take(&mut self.net.remote.pending_valuables) {
                    if let Some(v) = w.valuables.get_mut(vi as usize) {
                        if !v.taken {
                            v.taken = true;
                            pending_toasts.push(format!("PLAYER FOUND: {}", v.kind.name()));
                        }
                    }
                }
            }
            if let Some(pid) = self.net.remote.found_by.take() {
                if let Some(g) = &mut self.game {
                    if g.state == GameState::Playing {
                        g.state = GameState::Won;
                        g.won_time = g.time;
                        pending_toasts.push(format!("PLAYER #{} FOUND THE NEEDLE!", pid));
                    }
                }
            }
            // broadcast our position
            self.net.send_pos(
                [self.cam.pos.x, self.cam.pos.y, self.cam.pos.z],
                self.cam.yaw,
                self.cam.pitch,
                dt,
            );
        }
        for t in pending_toasts {
            self.toast(t);
        }

        let mut playing = false;
        let mut won = false;
        {
        let Some(g) = &mut self.game else { return };
        let Some(w) = &mut self.world else { return };
        playing = g.state == GameState::Playing;
        won = g.state == GameState::Won;

        // movement + dig
        if playing {
            let mut mv = Vec3::ZERO;
            let (f, r) = (self.cam.fwd(), self.cam.right());
            if self.keys.contains(&KeyCode::KeyW) { mv += f; }
            if self.keys.contains(&KeyCode::KeyS) { mv -= f; }
            if self.keys.contains(&KeyCode::KeyA) { mv -= r; }
            if self.keys.contains(&KeyCode::KeyD) { mv += r; }
            let sprint = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
            let speed = if sprint { 7.5 } else { 4.2 };
            if mv.length_squared() > 0.0 {
                mv = mv.normalize() * speed * dt;
                self.walk_t += dt * (if sprint { 11.0 } else { 7.0 });
                self.cam.pos.x += mv.x;
                self.cam.pos.z += mv.z;
            }
            // keep the player out of the pile (inflated ellipsoid push-out)
            let pr = 0.45;
            let ex = self.cam.pos.x / (PILE_R + pr);
            let ez = self.cam.pos.z / (PILE_R + pr);
            let d2 = ex * ex + ez * ez;
            if d2 < 1.0 {
                let dir = Vec3::new(ex, 0.0, ez).normalize_or_zero();
                let np = dir * (PILE_R + pr);
                self.cam.pos.x = np.x;
                self.cam.pos.z = np.z;
            }
            self.cam.pos.x = self.cam.pos.x.clamp(-260.0, 260.0);
            self.cam.pos.z = self.cam.pos.z.clamp(-260.0, 260.0);
            self.cam.pos.y = 1.62;

            // dig with LMB (game enforces cooldown)
            if self.mouse_l {
                let o = self.cam.pos;
                let d = self.cam.fwd();
                let hit = g.dig(w, o, d);
                let idxs = w.take_pending_removals();
                if !idxs.is_empty() {
                    let hit_pos = hit.map(|(h, _)| h).unwrap_or(o + d * 2.0);
                    let mut rng = rng::Rng::new((self.time * 1000.0) as u64 | 1);
                    self.toss.spawn_burst(hit_pos, idxs.len().min(24), &mut rng);
                    self.renderer.as_mut().unwrap().update_straw_states(&idxs);
                    self.net.send_digs(&idxs);
                }
                if let Some(vi) = g.last_valuable_idx.take() {
                    self.net.send_valuable(vi);
                }
            }
            // RMB zoom (inspect)
            self.cam.zoom = if self.mouse_r || self.keys.contains(&KeyCode::KeyC) {
                (self.cam.zoom - dt * 3.0).max(0.12)
            } else {
                (self.cam.zoom + dt * 3.0).min(1.0)
            };
        }

        let cam_pos = self.cam.pos;
        let cam_fwd = self.cam.fwd();
        g.update(dt, w, cam_pos, cam_fwd);
        self.toss.update(dt);

        // autosave
        self.autosave_t += dt;
        if self.autosave_t > 60.0 && g.state == GameState::Playing {
            self.autosave_t = 0.0;
            let _ = game::save_game(g, w, "save.bin");
        }
        }

        if won && self.grabbed {
            self.set_grab(false);
        }
        if !playing {
            return;
        }
    }

    fn net_connect_ready(&self) -> Option<(u64, u32)> {
        if self.net.role == Role::Client && self.net.remote.id != 255 {
            if let (Some(s), Some(c)) = (self.net.welcome_seed, self.net.welcome_count) {
                return Some((s, c));
            }
        }
        None
    }

    // ---------------- props ----------------
    fn build_props(&self) -> Vec<PropDraw> {
        let mut props: Vec<PropDraw> = Vec::new();
        let dome_idx = model::M_FARMER + 1;
        let ground_idx = dome_idx + 1;

        props.push(PropDraw {
            model: ground_idx,
            mat: Mat4::IDENTITY,
            col: [0.62, 0.63, 0.65],
            flag: 0.998,
        });
        props.push(PropDraw {
            model: dome_idx,
            mat: Mat4::IDENTITY,
            col: [1.0, 1.0, 1.0],
            flag: 0.0,
        });

        if let (Some(g), Some(w)) = (&self.game, &self.world) {
            if g.state != GameState::Menu {
                props.push(PropDraw {
                    model: model::M_NEEDLE,
                    mat: Mat4::from_rotation_y(w.needle.yaw) * Mat4::from_translation(w.needle.pos),
                    col: [1.0, 1.0, 1.0],
                    flag: 0.5,
                });
                for v in &w.valuables {
                    if v.taken {
                        continue;
                    }
                    props.push(PropDraw {
                        model: valuable_model(v.kind),
                        mat: Mat4::from_rotation_y(v.yaw)
                            * Mat4::from_translation(v.pos)
                            * Mat4::from_scale(Vec3::splat(1.6)),
                        col: [1.0, 1.0, 1.0],
                        flag: 0.55,
                    });
                }
                if !g.pure_mode {
                    if g.owned[7] {
                        props.push(PropDraw {
                            model: model::M_BALER,
                            mat: Mat4::from_rotation_y(-0.5) * Mat4::from_translation(Vec3::new(-26.0, 0.0, 12.0)),
                            col: [1.0, 1.0, 1.0],
                            flag: 0.6,
                        });
                    }
                    if g.owned[8] {
                        props.push(PropDraw {
                            model: model::M_VACUUM,
                            mat: Mat4::from_rotation_y(0.5) * Mat4::from_translation(Vec3::new(26.0, 0.0, 12.0)),
                            col: [1.0, 1.0, 1.0],
                            flag: 0.6,
                        });
                    }
                }
            }
        }
        // co-op farmers
        for (i, p) in self.net.remote.players.iter().enumerate() {
            if p.active && i != self.net.remote.id as usize {
                props.push(PropDraw {
                    model: model::M_FARMER,
                    mat: Mat4::from_rotation_y(p.yaw) * Mat4::from_translation(Vec3::new(p.pos[0], 0.0, p.pos[2])),
                    col: [1.0, 1.0, 1.0],
                    flag: 0.0,
                });
            }
        }
        props.sort_by_key(|p| p.model);
        props
    }

    fn build_viewmodel(&self) -> Option<PropDraw> {
        let g = self.game.as_ref()?;
        if g.state != GameState::Playing {
            return None;
        }
        let right = self.cam.right();
        let up = self.cam.up();
        let fwd = self.cam.fwd();
        let cam_m = Mat4::from_cols(
            Vec4::new(right.x, right.y, right.z, 0.0),
            Vec4::new(up.x, up.y, up.z, 0.0),
            Vec4::new(-fwd.x, -fwd.y, -fwd.z, 0.0),
            Vec4::new(self.cam.pos.x, self.cam.pos.y, self.cam.pos.z, 1.0),
        );
        let bob_y = self.walk_t.sin() * 0.014;
        let bob_x = (self.walk_t * 0.5).cos() * 0.010;
        let (model, local) = match g.current_tool() {
            game::Tool::Detector | game::Tool::DetectorII => (
                model::M_DETECTOR,
                Mat4::from_translation(Vec3::new(0.34 - bob_x, -0.34 + bob_y, -0.55))
                    * Mat4::from_rotation_x(-1.32)
                    * Mat4::from_rotation_y(-0.30),
            ),
            game::Tool::Pitchfork => (
                model::M_PITCHFORK,
                Mat4::from_translation(Vec3::new(0.42 - bob_x, -0.62 + bob_y, -0.85))
                    * Mat4::from_rotation_x(-0.9)
                    * Mat4::from_rotation_z(0.18),
            ),
            game::Tool::Shovel => (
                model::M_SHOVEL,
                Mat4::from_translation(Vec3::new(0.44 - bob_x, -0.60 + bob_y, -0.80))
                    * Mat4::from_rotation_x(-0.95)
                    * Mat4::from_rotation_z(0.20),
            ),
            _ => return None,
        };
        Some(PropDraw { model, mat: cam_m * local, col: [1.0, 1.0, 1.0], flag: 0.0 })
    }

    fn render(&mut self) {
        let props = self.build_props();
        let vm = self.build_viewmodel();
        let toss = self.toss.instances();
        self.hud.clear();
        self.click.clear();
        ui::draw(self);
        let frame = render::Frame {
            cam: &self.cam,
            hud: &self.hud,
            props: &props,
            vm: vm.as_ref(),
            toss: &toss,
            time: self.time as f32,
            ui_scale: 1.0,
        };
        if let Some(r) = &mut self.renderer {
            let _ = r.render(&frame);
        }
    }
}

fn valuable_model(k: mesh::ValuableKind) -> usize {
    match k {
        mesh::ValuableKind::Coin => model::M_COIN,
        mesh::ValuableKind::Button => model::M_BUTTON,
        mesh::ValuableKind::Thimble => model::M_THIMBLE,
        mesh::ValuableKind::Key => model::M_KEY,
        mesh::ValuableKind::Spoon => model::M_SPOON,
        mesh::ValuableKind::Fork => model::M_FORK,
        mesh::ValuableKind::Horseshoe => model::M_HORSESHOE,
        mesh::ValuableKind::Ring => model::M_RING,
        mesh::ValuableKind::Locket => model::M_LOCKET,
        mesh::ValuableKind::Watch => model::M_WATCH,
    }
}

// ---------------- winit app handler ----------------
struct AppHandler(App);

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.0.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("FIND THE NEEDLE - rust+wgpu")
            .with_inner_size(LogicalSize::new(1280, 720));
        let window = Arc::new(el.create_window(attrs).expect("window"));
        self.0.size = (window.inner_size().width, window.inner_size().height);
        self.0.window = Some(window.clone());

        let instance = gpu::create_instance();
        let surface = instance.create_surface(window.clone()).expect("surface");
        let (_adapter, gp) = pollster::block_on(gpu::create_device(&instance)).expect("no GPU");
        let caps = surface.get_capabilities(&_adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: self.0.size.0.max(1),
            height: self.0.size.1.max(1),
            present_mode: if self.0.settings.vsync { wgpu::PresentMode::Fifo } else { wgpu::PresentMode::Immediate },
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gp.device, &config);
        let mut models = model::load_all();
        let mut mb = mesh::MeshBuilder::new();
        mb.dome(PILE_R * 0.90, PILE_H * 0.90, 64, 14, Vec3::new(0.135, 0.088, 0.042));
        let (dv, di) = mb.build();
        models.push(to_model(dv, di));
        let mut mb = mesh::MeshBuilder::new();
        mb.cube(Vec3::new(-600.0, -0.5, -600.0), Vec3::new(600.0, 0.0, 600.0), Vec3::new(0.62, 0.63, 0.65));
        let (gv, gi) = mb.build();
        models.push(to_model(gv, gi));

        self.0.adapter_info = gp.adapter_info.clone();
        self.0.renderer = Some(Renderer::new(gp.device, gp.queue, Some(surface), config, &self.0.settings, models));

        // menu preview world
        self.0.world = Some(World::new(20260926, 350_000));
        let r = self.0.renderer.as_mut().unwrap();
        r.set_straw_instances(self.0.world.as_ref().unwrap());

        window.request_redraw();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: winit::window::WindowId, event: WindowEvent) {
        let app = &mut self.0;
        match event {
            WindowEvent::CloseRequested => {
                if let (Some(g), Some(w)) = (&app.game, &app.world) {
                    let _ = game::save_game(g, w, "save.bin");
                }
                el.exit();
            }
            WindowEvent::Resized(size) => {
                app.size = (size.width, size.height);
                if let Some(r) = &mut app.renderer {
                    r.resize(size.width.max(1), size.height.max(1));
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    if pressed {
                        app.keys.insert(code);
                        ui::on_key(app, code, el);
                    } else {
                        app.keys.remove(&code);
                    }
                }
                if pressed {
                    if let Some(txt) = event.text.as_deref() {
                        ui::on_text(app, txt);
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        app.mouse_l = pressed;
                        if pressed {
                            ui::on_click(app);
                        }
                    }
                    MouseButton::Right => app.mouse_r = pressed,
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if app.grabbed {
                    let cx = app.size.0 as f64 / 2.0;
                    let cy = app.size.1 as f64 / 2.0;
                    let dx = position.x - cx;
                    let dy = position.y - cy;
                    let s = app.settings.sensitivity * 0.0022;
                    app.cam.yaw += dx as f32 * s;
                    app.cam.pitch = (app.cam.pitch - dy as f32 * s).clamp(-1.45, 1.45);
                    let _ = app.window().set_cursor_position(winit::dpi::Position::Physical(
                        winit::dpi::PhysicalPosition::new(cx as i32, cy as i32),
                    ));
                    app.cursor = (cx as f32, cy as f32);
                } else {
                    app.cursor = (position.x as f32, position.y as f32);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 0.12,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.01,
                };
                app.cam.zoom = (app.cam.zoom - d).clamp(0.12, 1.0);
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let dt = app.last.map(|t| (now - t).as_secs_f32()).unwrap_or(0.016).min(0.05);
                app.last = Some(now);
                app.update(dt);
                app.render();
                app.window().request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if self.0.window.is_some() {
            self.0.window().request_redraw();
        }
    }
}

fn to_model(verts: Vec<mesh::Vertex>, idx: Vec<u16>) -> model::Model {
    let verts = verts
        .into_iter()
        .map(|v| model::ModelVertex { pos: v.pos, nrm: v.nrm, col: v.col })
        .collect();
    model::Model { verts, idx: idx.into_iter().map(|i| i as u32).collect() }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--screenshot") {
        run_screenshot(&args);
        return;
    }
    let ev = EventLoop::new().unwrap();
    ev.set_control_flow(ControlFlow::Poll);
    let mut handler = AppHandler(App::new());
    ev.run_app(&mut handler).unwrap();
}

// Headless validation + README shots: renders the real scene without a window.
fn run_screenshot(args: &[String]) {
    let get = |k: &str, d: &str| -> String {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| d.to_string())
    };
    let out = get("--out", "screenshot.png");
    let seed: u64 = get("--seed", "12345").parse().unwrap_or(12345);
    let count: u32 = get("--count", "400000").parse().unwrap_or(400000);
    let px: u32 = get("--w", "1600").parse().unwrap_or(1600);
    let py: u32 = get("--h", "900").parse().unwrap_or(900);
    let yaw: f32 = get("--yaw", "0.0").parse().unwrap_or(0.0);
    let dist: f32 = get("--dist", "24.0").parse().unwrap_or(24.0);
    let eye: f32 = get("--eye", "3.2").parse().unwrap_or(3.2);

    let instance = gpu::create_instance();
    let (_adapter, gp) = pollster::block_on(gpu::create_device(&instance)).expect("no GPU");
    let settings = Settings::default();
    let mut models = model::load_all();
    let mut mb = mesh::MeshBuilder::new();
    mb.dome(PILE_R * 0.90, PILE_H * 0.90, 64, 14, Vec3::new(0.135, 0.088, 0.042));
    let (dv, di) = mb.build();
    models.push(to_model(dv, di));
    let mut mb = mesh::MeshBuilder::new();
    mb.cube(Vec3::new(-600.0, -0.5, -600.0), Vec3::new(600.0, 0.0, 600.0), Vec3::new(0.62, 0.63, 0.65));
    let (gv, gi) = mb.build();
    models.push(to_model(gv, gi));

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: px,
        height: py,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    let mut renderer = Renderer::new(gp.device, gp.queue, None, config, &settings, models);
    renderer.set_headless_size(px, py);

    let world = World::new(seed, count);
    renderer.set_straw_instances(&world);
    let mut g = game::Game::new(seed, false);
    g.state = GameState::Playing;
    let mut app = App::new();
    app.size = (px, py);
    app.world = Some(world);
    app.game = Some(g);
    app.cam.pos = Vec3::new(dist * yaw.sin(), eye, dist * yaw.cos());
    app.cam.yaw = yaw;
    app.cam.pitch = -0.08;

    let props = app.build_props();
    let vm = app.build_viewmodel();
    let toss = app.toss.instances();
    app.hud.clear();
    app.click.clear();
    ui::draw(&mut app);
    let frame = render::Frame {
        cam: &app.cam,
        hud: &app.hud,
        props: &props,
        vm: vm.as_ref(),
        toss: &toss,
        time: 1.0,
        ui_scale: 1.0,
    };
    renderer.request_screenshot(&out);
    renderer.render(&frame).unwrap();
    renderer.finish_screenshot();
    println!("headless render done: {} ({} straws, {}x{})", out, count, px, py);
}
