// UI: menu, shop, in-game HUD (objective, waypoint, radius tooltip, crosshair),
// pause, win screen, toasts. Pixel-space quads + 5x7 bitmap font only.
use crate::game::{GameState, TOOLS};
use crate::hud::Hud;
use crate::net::Role;
use crate::world::PILE_H;
use crate::rng::hash_seed;
use crate::{App, Click, Screen};
use glam::{Vec3, Vec4Swizzles};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

const AMBER: [f32; 4] = [1.0, 0.78, 0.10, 1.0];
const WHITE: [f32; 4] = [0.96, 0.97, 0.98, 1.0];
const SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 0.65];
const PANEL: [f32; 4] = [0.05, 0.05, 0.06, 0.88];
const GREEN: [f32; 4] = [0.35, 0.85, 0.40, 1.0];
const RED: [f32; 4] = [0.95, 0.35, 0.30, 1.0];

fn shadow_text(app: &mut App, x: f32, y: f32, s: f32, col: [f32; 4], text: &str) -> f32 {
    app.hud.text(x + s * 0.8, y + s * 0.8, s, SHADOW, text);
    app.hud.text(x, y, s, col, text)
}

fn shadow_text_center(app: &mut App, cx: f32, y: f32, s: f32, col: [f32; 4], text: &str) {
    let w = Hud::text_w(text, s);
    shadow_text(app, cx - w / 2.0, y, s, col, text);
}

fn project(app: &App, p: Vec3) -> Option<(f32, f32)> {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    if w < 1.0 || h < 1.0 {
        return None;
    }
    let aspect = w / h;
    let vp = app.cam.view_proj(aspect, app.cam.near(), 600.0);
    let clip = vp * p.extend(1.0);
    if clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.xy() / clip.w;
    Some(((ndc.x * 0.5 + 0.5) * w, (1.0 - (ndc.y * 0.5 + 0.5)) * h))
}

// ---------------- main draw dispatch ----------------
pub fn draw(app: &mut App) {
    if app.in_menu() {
        match app.screen {
            Screen::Main => draw_menu(app),
            Screen::SeedInput => draw_input(app, "ENTER SEED (TEXT OR NUMBER)"),
            Screen::JoinInput => draw_input(app, "ENTER HOST IP ADDRESS"),
            Screen::HowTo => draw_howto(app),
        }
        draw_toasts(app);
        return;
    }
    if app.connecting {
        let (w, h) = (app.size.0 as f32, app.size.1 as f32);
        app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.5]);
        shadow_text_center(app, w / 2.0, h / 2.0 - 12.0, 4.0, WHITE, "CONNECTING...");
        shadow_text_center(app, w / 2.0, h / 2.0 + 24.0, 2.0, AMBER, &format!("[{}] {}", app.connect_t as u32, app.net.status_line()));
        draw_toasts(app);
        return;
    }

    let state = app.game.as_ref().map(|g| g.state).unwrap_or(GameState::Playing);
    match state {
        GameState::Playing => draw_playing(app),
        GameState::Shop => {
            draw_playing(app);
            draw_shop(app);
        }
        GameState::Paused => {
            draw_playing(app);
            draw_pause(app);
        }
        GameState::Won => {
            draw_playing(app);
            draw_won(app);
        }
        GameState::Menu => draw_menu(app),
    }
    draw_toasts(app);
}

// ---------------- menu ----------------
fn menu_items(app: &App) -> [String; 7] {
    let pure = if app.pure_mode { "ON (NO TOOLS)" } else { "OFF" };
    let seed = if app.buf.is_empty() { "RANDOM".to_string() } else { app.buf.to_uppercase() };
    [
        format!("START HAYSTACK: {} PIECES", app.settings.quality.straw_count()),
        format!("PURE MODE (NO UPGRADES): {}", pure),
        format!("SEED: {}", seed),
        format!("QUALITY: {}", app.settings.quality.name()),
        "CO-OP: HOST GAME".to_string(),
        "CO-OP: JOIN (IP)".to_string(),
        "QUIT".to_string(),
    ]
}

fn draw_menu(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    // subtle dark vignette
    app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.02, 0.25]);
    app.hud.rect(0.0, 0.0, w, 120.0, [0.0, 0.0, 0.0, 0.35]);
    app.hud.rect(0.0, h - 150.0, w, 150.0, [0.0, 0.0, 0.0, 0.35]);

    shadow_text_center(app, w / 2.0, 26.0, 7.0, AMBER, "FIND THE NEEDLE");
    shadow_text_center(app, w / 2.0, 86.0, 2.2, WHITE, "THE RUST + WGPU REMAKE - 5,000,000 PIECES OF HAY - 1 NEEDLE");

    let items = menu_items(app);
    let y0 = h / 2.0 - 130.0;
    for (i, it) in items.iter().enumerate() {
        let y = y0 + i as f32 * 34.0;
        let sel = i == app.menu_sel;
        if sel {
            app.hud.rect(w / 2.0 - 220.0, y - 4.0, 440.0, 28.0, [1.0, 0.78, 0.10, 0.18]);
            app.hud.rect(w / 2.0 - 220.0, y - 4.0, 3.0, 28.0, AMBER);
            app.hud.rect(w / 2.0 + 217.0, y - 4.0, 3.0, 28.0, AMBER);
        }
        let col = if sel { AMBER } else { WHITE };
        shadow_text_center(app, w / 2.0, y, 2.0, col, it);
        app.click.push((w / 2.0 - 220.0, y - 4.0, 440.0, 28.0, menu_click(i)));
    }

    // footer
    let lines = [
        "W/S SELECT - ENTER CONFIRM - MOUSE CLICK WORKS TOO".to_string(),
        "IN GAME: WASD MOVE - MOUSE LOOK - LMB DIG - RMB ZOOM - TAB SHOP - E SELL - ESC PAUSE".to_string(),
        format!("{} - {} FPS", app.adapter_info, app.fps as u32),
    ];
    for (i, l) in lines.iter().enumerate() {
        shadow_text_center(app, w / 2.0, h - 140.0 + i as f32 * 22.0, 1.7, [0.75, 0.78, 0.82, 1.0], l);
    }
}

fn menu_click(i: usize) -> Click {
    match i {
        0 => Click::Start,
        1 => Click::TogglePure,
        2 => Click::Seed,
        3 => Click::Quality,
        4 => Click::Host,
        5 => Click::Join,
        _ => Click::Quit,
    }
}

fn draw_input(app: &mut App, title: &str) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.02, 0.45]);
    shadow_text_center(app, w / 2.0, h / 2.0 - 70.0, 2.4, AMBER, title);
    app.hud.rect(w / 2.0 - 240.0, h / 2.0 - 24.0, 480.0, 44.0, PANEL);
    app.hud.rect(w / 2.0 - 240.0, h / 2.0 - 24.0, 480.0, 2.0, AMBER);
    app.hud.rect(w / 2.0 - 240.0, h / 2.0 + 18.0, 480.0, 2.0, AMBER);
    let shown = format!("{}_", app.buf.to_uppercase());
    shadow_text_center(app, w / 2.0, h / 2.0 - 9.0, 2.4, WHITE, &shown);
    shadow_text_center(app, w / 2.0, h / 2.0 + 50.0, 1.8, [0.8, 0.8, 0.85, 1.0], "ENTER CONFIRM - ESC CANCEL");
}

fn draw_howto(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.02, 0.55]);
    shadow_text_center(app, w / 2.0, 50.0, 4.0, AMBER, "HOW TO PLAY");
    let lines = [
        "",
        "SOMEWHERE IN THE PILE IS ONE SEWING NEEDLE. DIG IT OUT.",
        "",
        "1. PICK HAY PIECE BY PIECE (OR SCOOP WITH TOOLS)",
        "2. VALUABLES HIDE IN THE HAY - SELL THEM FOR MONEY",
        "3. BUY TOOLS: FORKS SCOOP, DETECTORS PING THE NEEDLE,",
        "   THE BALER AND VACUUM EAT HAY FOREVER",
        "4. METAL DETECTOR: FASTER PING = YOU ARE CLOSE",
        "5. ZOOM (RMB) TO INSPECT. THE NEEDLE GLINTS IN THE SUN",
        "",
        "PURE MODE = NO TOOLS, NO AUTOMATION. JUST YOU AND 5M PIECES.",
        "CO-OP: ONE PLAYER HOSTS, OTHERS JOIN THE LAN IP.",
        "",
        "PRESS ESC TO GO BACK",
    ];
    for (i, l) in lines.iter().enumerate() {
        shadow_text_center(app, w / 2.0, 110.0 + i as f32 * 26.0, 2.0, WHITE, l);
    }
}

// ---------------- playing HUD ----------------
fn draw_playing(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    let (time, money, bag_value, selected, det_radius, det_hot, det_pulse, removed, total) = {
        let Some(g) = app.game.as_ref() else { return };
        let Some(world) = app.world.as_ref() else { return };
        (
            g.time,
            g.money,
            g.bag_value,
            g.selected,
            g.detector_radius(),
            g.detector_hot,
            g.detector_pulse,
            world.removed_count,
            world.straw_count,
        )
    };

    // OBJECTIVE panel (like the real game)
    shadow_text(app, 24.0, 20.0, 2.0, AMBER, "OBJECTIVE:");
    shadow_text(app, 24.0, 42.0, 2.6, WHITE, "FIND THE NEEDLE");
    // piece counter
    let removed_pct = (removed as f64 / total.max(1) as f64 * 100.0).min(100.0);
    shadow_text(
        app,
        24.0,
        78.0,
        1.8,
        [0.85, 0.87, 0.9, 1.0],
        &format!("HAY DUG: {} / {} ({:.4}%)", removed, total, removed_pct),
    );

    // timer top center
    let t = crate::game::format_time(time);
    shadow_text_center(app, w / 2.0, 16.0, 2.6, WHITE, &t);

    // fps + net top right
    let fps_txt = format!("{:.0} FPS", app.fps);
    let fw = Hud::text_w(&fps_txt, 2.0);
    shadow_text(app, w - fw - 20.0, 16.0, 2.0, if app.fps > 55.0 { GREEN } else if app.fps > 30.0 { AMBER } else { RED }, &fps_txt);
    if app.net.role != Role::Offline {
        shadow_text_center(app, w / 2.0, 46.0, 1.8, AMBER, &app.net.status_line());
    }

    // waypoint above the pile: hexagon + distance
    let top = Vec3::new(0.0, PILE_H + 4.5, 0.0);
    if let Some((sx, sy)) = project(app, top) {
        let dist = app.cam.pos.distance(Vec3::new(0.0, 0.0, 0.0)) as u32;
        let r = 26.0;
        app.hud.hex(sx, sy - 30.0, r, [1.0, 0.78, 0.10]);
        shadow_text_center(app, sx, sy - 40.0, 2.6, [1.0, 0.78, 0.10, 1.0], "!");
        shadow_text_center(app, sx, sy + 6.0, 2.8, WHITE, &format!("HAYSTACK - {}M", dist));
    }

    // detector radius tooltip + ping ring
    if let Some(rad) = det_radius {
        // ring at crosshair pulsing when hot
        if det_hot {
            let p = (det_pulse.sin() * 0.5 + 0.5) as f32;
            app.hud.ring(w / 2.0, h / 2.0, 14.0 + p * 8.0, 2.5, [1.0, 0.85 - p * 0.5, 0.2]);
        }
        // ray to the pile for the tooltip anchor
        let o = app.cam.pos;
        let d = app.cam.fwd();
        if let Some(t) = crate::world::World::pile_entry(o, d) {
            let hit = o + d * t;
            if let Some((sx, sy)) = project(app, hit) {
                shadow_text_center(app, sx, sy - 26.0, 2.0, WHITE, &format!("Radius - {}m", rad as u32));
            }
        }
    }

    // crosshair: dot + outline
    app.hud.rect(w / 2.0 - 3.0, h / 2.0 - 3.0, 6.0, 6.0, [0.0, 0.0, 0.0, 0.55]);
    app.hud.rect(w / 2.0 - 2.0, h / 2.0 - 2.0, 4.0, 4.0, [0.98, 0.98, 0.98, 0.95]);

    // bottom-left: money / bag / coop
    shadow_text(app, 24.0, h - 96.0, 2.4, GREEN, &format!("$ {}", money));
    let bag = if bag_value > 0 {
        format!("BAG: ${} [E] SELL", bag_value)
    } else {
        "BAG: EMPTY".to_string()
    };
    shadow_text(app, 24.0, h - 66.0, 1.9, if bag_value > 0 { AMBER } else { [0.7, 0.72, 0.76, 1.0] }, &bag);

    // bottom-right: tool + hints
    let tool = TOOLS[selected];
    let tw = Hud::text_w(tool.name, 2.2);
    shadow_text(app, w - tw - 24.0, h - 70.0, 2.2, WHITE, tool.name);
    shadow_text(app, w - 250.0, h - 42.0, 1.7, [0.7, 0.72, 0.76, 1.0], "[TAB] SHOP - [E] SELL - [ESC] PAUSE");

    // co-op players name tags
    let my_id = app.net.remote.id as usize;
    let players: Vec<(usize, crate::net::RemotePlayer)> = app
        .net
        .remote
        .players
        .iter()
        .copied()
        .enumerate()
        .filter(|(i, p)| p.active && *i != my_id)
        .collect();
    for (i, p) in players {
        let pos = Vec3::new(p.pos[0], p.pos[1] + 2.1, p.pos[2]);
        if let Some((sx, sy)) = project(app, pos) {
            let name = String::from_utf8_lossy(&p.name).trim().to_string();
            let tag = if name.is_empty() { format!("PLAYER #{}", i) } else { name.to_uppercase() };
            shadow_text_center(app, sx, sy, 1.7, AMBER, &tag);
        }
    }
}

// ---------------- shop ----------------
fn draw_shop(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    let (money, owned, sel_tool) = {
        let Some(g) = app.game.as_ref() else { return };
        (g.money, g.owned, g.selected)
    };
    let px = w / 2.0 - 330.0;
    let py = 70.0;
    let pw = 660.0;
    let ph = 82.0 + TOOLS.len() as f32 * 52.0 + 70.0;
    app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.45]);
    app.hud.rect(px, py, pw, ph, PANEL);
    app.hud.rect(px, py, pw, 3.0, AMBER);
    shadow_text_center(app, w / 2.0, py + 14.0, 3.4, AMBER, "TOOL SHOP");
    shadow_text_center(app, w / 2.0, py + 50.0, 2.2, GREEN, &format!("MONEY: ${}", money));

    for (i, def) in TOOLS.iter().enumerate() {
        let y = py + 92.0 + i as f32 * 52.0;
        let sel = i == app.shop_sel;
        let owned = owned[i];
        let equipped = sel_tool == i;
        if sel {
            app.hud.rect(px + 10.0, y - 5.0, pw - 20.0, 46.0, [1.0, 0.78, 0.10, 0.14]);
        }
        let price_txt = if owned {
            if equipped { "EQUIPPED".to_string() } else { "OWNED - [ENTER] EQUIP".to_string() }
        } else {
            format!("${}", def.price)
        };
        let can = money >= def.price || owned;
        let name_col = if equipped { GREEN } else if sel { AMBER } else { WHITE };
        shadow_text(app, px + 26.0, y, 2.2, name_col, def.name);
        let pw_txt = Hud::text_w(&price_txt, 2.0);
        shadow_text(app, px + pw - pw_txt - 26.0, y, 2.0, if owned { GREEN } else if can { AMBER } else { RED }, &price_txt);
        shadow_text(app, px + 26.0, y + 22.0, 1.7, [0.72, 0.74, 0.78, 1.0], def.desc);
        app.click.push((px + 10.0, y - 5.0, pw - 20.0, 46.0, Click::Buy(i)));
    }
    let sy = py + ph - 40.0;
    shadow_text_center(app, w / 2.0, sy, 1.9, WHITE, "[ENTER] BUY/EQUIP - [S] SELL BAG - [TAB/ESC] CLOSE");
    app.click.push((px, py, pw, ph, Click::Back));
}

// ---------------- pause / won ----------------
fn draw_pause(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.55]);
    shadow_text_center(app, w / 2.0, h / 2.0 - 110.0, 5.0, AMBER, "PAUSED");
    let items = [
        ("RESUME", Click::Resume),
        ("SAVE GAME", Click::SaveLoad),
        ("QUIT TO MENU", Click::ToMenu),
    ];
    for (i, (label, c)) in items.iter().enumerate() {
        let y = h / 2.0 - 20.0 + i as f32 * 44.0;
        let sel = i == app.menu_sel.min(2);
        if sel {
            app.hud.rect(w / 2.0 - 170.0, y - 5.0, 340.0, 32.0, [1.0, 0.78, 0.10, 0.16]);
        }
        shadow_text_center(app, w / 2.0, y, 2.4, if sel { AMBER } else { WHITE }, label);
        app.click.push((w / 2.0 - 170.0, y - 5.0, 340.0, 32.0, *c));
    }
    shadow_text_center(app, w / 2.0, h / 2.0 + 140.0, 1.8, [0.75, 0.78, 0.82, 1.0], "ESC RESUME - W/S SELECT - ENTER CONFIRM");
}

fn draw_won(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    app.hud.rect(0.0, 0.0, w, h, [0.0, 0.0, 0.0, 0.62]);
    let (won_time, best_time) = {
        let Some(g) = app.game.as_ref() else { return };
        (g.won_time, g.best_time)
    };
    shadow_text_center(app, w / 2.0, h / 2.0 - 150.0, 5.5, AMBER, "YOU FOUND THE NEEDLE!");
    shadow_text_center(app, w / 2.0, h / 2.0 - 90.0, 3.0, WHITE, &format!("TIME: {}", crate::game::format_time(won_time)));
    if let Some(b) = best_time {
        let is_best = (won_time - b).abs() < f64::EPSILON;
        shadow_text_center(app, w / 2.0, h / 2.0 - 50.0, 2.2, if is_best { GREEN } else { [0.8, 0.8, 0.85, 1.0] }, &format!("BEST: {}", crate::game::format_time(b)));
        if is_best {
            shadow_text_center(app, w / 2.0, h / 2.0 - 18.0, 2.0, GREEN, "NEW BEST TIME!");
        }
    }
    shadow_text_center(app, w / 2.0, h / 2.0 + 40.0, 2.2, WHITE, "[R] DIG AGAIN - [M] MENU");
    app.click.push((w / 2.0 - 200.0, h / 2.0 + 30.0, 400.0, 40.0, Click::Start));
}

// ---------------- toasts ----------------
fn draw_toasts(app: &mut App) {
    let (w, h) = (app.size.0 as f32, app.size.1 as f32);
    let n = app.toasts.len();
    for (i, (txt, life)) in app.toasts.iter().enumerate() {
        let a = (life.min(1.0) * 2.0).min(1.0);
        let y = h - 150.0 - (n - 1 - i) as f32 * 26.0;
        let tw = Hud::text_w(txt, 1.9);
        app.hud.rect(w / 2.0 - tw / 2.0 - 12.0, y - 5.0, tw + 24.0, 24.0, [0.0, 0.0, 0.0, 0.5 * a]);
        let mut col = AMBER;
        col[3] = a;
        app.hud.text_center(w / 2.0, y, 1.9, col, txt);
    }
}

// ---------------- input ----------------
pub fn on_key(app: &mut App, code: KeyCode, el: &ActiveEventLoop) {
    // ---- global ----
    if code == KeyCode::F5 {
        if let (Some(g), Some(w)) = (&app.game, &app.world) {
            let _ = crate::game::save_game(g, w, "save.bin");
            app.toast("GAME SAVED");
        }
        return;
    }
    if code == KeyCode::F9 {
        load_save(app);
        return;
    }

    // ---- menu ----
    if app.in_menu() {
        match app.screen {
            Screen::Main => {
                let n = 7;
                match code {
                    KeyCode::KeyW | KeyCode::ArrowUp => app.menu_sel = (app.menu_sel + n - 1) % n,
                    KeyCode::KeyS | KeyCode::ArrowDown => app.menu_sel = (app.menu_sel + 1) % n,
                    KeyCode::Enter | KeyCode::Space => activate_menu(app, app.menu_sel, el),
                    KeyCode::Escape => el.exit(),
                    _ => {}
                }
            }
            Screen::SeedInput | Screen::JoinInput => match code {
                KeyCode::Enter => {
                    if app.screen == Screen::SeedInput {
                        let seed = if app.buf.trim().is_empty() {
                            (app.time * 1000.0) as u64 ^ 0xDEADBEEF
                        } else {
                            hash_seed(app.buf.trim())
                        };
                        let count = app.settings.quality.straw_count();
                        app.buf.clear();
                        app.start_game(seed, count, app.pure_mode);
                    } else {
                        let ip = app.buf.trim().to_string();
                        app.buf.clear();
                        if !ip.is_empty() {
                            app.net = crate::net::Net::join(&ip, *b"PLAYER      ");
                            app.connecting = true;
                            app.connect_t = 0.0;
                            app.hello_t = 0.0;
                        }
                    }
                }
                KeyCode::Escape => {
                    app.screen = Screen::Main;
                    app.buf.clear();
                }
                _ => {}
            },
            Screen::HowTo => {
                if matches!(code, KeyCode::Escape | KeyCode::Enter) {
                    app.screen = Screen::Main;
                }
            }
        }
        return;
    }

    // ---- in game ----
    let state = app.game.as_ref().map(|g| g.state);
    match state {
        Some(GameState::Playing) => match code {
            KeyCode::Escape => {
                app.game.as_mut().unwrap().state = GameState::Paused;
                app.menu_sel = 0;
                app.set_grab(false);
            }
            KeyCode::Tab | KeyCode::KeyB => {
                app.game.as_mut().unwrap().state = GameState::Shop;
                app.shop_sel = 0;
                app.set_grab(false);
            }
            KeyCode::KeyE => app.game.as_mut().unwrap().sell_all(),
            _ => {}
        },
        Some(GameState::Shop) => match code {
            KeyCode::Escape | KeyCode::Tab | KeyCode::KeyB => {
                app.game.as_mut().unwrap().state = GameState::Playing;
                app.set_grab(true);
            }
            KeyCode::KeyW | KeyCode::ArrowUp => app.shop_sel = (app.shop_sel + TOOLS.len() - 1) % TOOLS.len(),
            KeyCode::KeyS | KeyCode::ArrowDown => app.shop_sel = (app.shop_sel + 1) % TOOLS.len(),
            KeyCode::Enter | KeyCode::KeyE => {
                let slot = app.shop_sel;
                app.game.as_mut().unwrap().buy(slot);
            }
            _ => {}
        },
        Some(GameState::Paused) => match code {
            KeyCode::Escape | KeyCode::Enter => {
                app.game.as_mut().unwrap().state = GameState::Playing;
                app.set_grab(true);
            }
            KeyCode::KeyW | KeyCode::ArrowUp => app.menu_sel = (app.menu_sel + 2) % 3,
            KeyCode::KeyS | KeyCode::ArrowDown => app.menu_sel = (app.menu_sel + 1) % 3,
            KeyCode::KeyM => {
                app.to_menu();
            }
            KeyCode::KeyQ => el.exit(),
            _ => {}
        },
        Some(GameState::Won) => match code {
            KeyCode::KeyR => {
                let (seed, pure) = {
                    let g = app.game.as_ref().unwrap();
                    (g.seed, g.pure_mode)
                };
                let count = app.settings.quality.straw_count();
                app.game = None;
                app.start_game(seed, count, pure);
            }
            KeyCode::KeyM => app.to_menu(),
            _ => {}
        },
        Some(GameState::Menu) | None => {}
    }
}

fn activate_menu(app: &mut App, sel: usize, el: &ActiveEventLoop) {
    match sel {
        0 => {
            let count = app.settings.quality.straw_count();
            let seed = if app.buf.trim().is_empty() {
                (app.time * 1000.0) as u64 ^ 0xC0FFEE
            } else {
                hash_seed(app.buf.trim())
            };
            app.start_game(seed, count, app.pure_mode);
        }
        1 => app.pure_mode = !app.pure_mode,
        2 => app.screen = Screen::SeedInput,
        3 => {
            app.settings.quality = app.settings.quality.next();
            app.settings.save();
            if let Some(r) = &mut app.renderer {
                r.apply_quality(&app.settings);
            }
        }
        4 => {
            // host
            app.net = crate::net::Net::host(*b"HOST        ");
            let count = app.settings.quality.straw_count();
            let seed = (app.time * 1000.0) as u64 ^ 0x5EED5EED;
            app.start_game(seed, count, app.pure_mode);
            app.toast("HOSTING ON UDP PORT 7777 - SHARE YOUR IP");
        }
        5 => app.screen = Screen::JoinInput,
        6 => el.exit(),
        _ => {}
    }
    app.buf.clear();
}

pub fn on_click(app: &mut App) {
    let (cx, cy) = app.cursor;
    let regions: Vec<(f32, f32, f32, f32, Click)> = app.click.clone();
    for (x, y, w, h, c) in regions.iter().rev() {
        if cx >= *x && cx <= *x + *w && cy >= *y && cy <= *y + *h {
            let c = *c;
            do_click(app, c);
            return;
        }
    }
}

fn do_click(app: &mut App, c: Click) {
    match c {
        Click::Start => {
            let count = app.settings.quality.straw_count();
            let seed = (app.time * 1000.0) as u64 ^ 0xC0FFEE;
            app.start_game(seed, count, app.pure_mode);
        }
        Click::TogglePure => app.pure_mode = !app.pure_mode,
        Click::Seed => app.screen = Screen::SeedInput,
        Click::Quality => {
            app.settings.quality = app.settings.quality.next();
            app.settings.save();
            if let Some(r) = &mut app.renderer {
                r.apply_quality(&app.settings);
            }
        }
        Click::Host => {
            app.net = crate::net::Net::host(*b"HOST        ");
            let count = app.settings.quality.straw_count();
            let seed = (app.time * 1000.0) as u64 ^ 0x5EED5EED;
            app.start_game(seed, count, app.pure_mode);
            app.toast("HOSTING ON UDP PORT 7777 - SHARE YOUR IP");
        }
        Click::Join => app.screen = Screen::JoinInput,
        Click::HowTo => app.screen = Screen::HowTo,
        Click::Quit => {
            // exiting from click needs event loop; just close via std::process
            std::process::exit(0);
        }
        Click::Resume => {
            if let Some(g) = &mut app.game {
                g.state = GameState::Playing;
            }
            app.set_grab(true);
        }
        Click::ToMenu => app.to_menu(),
        Click::Buy(i) => {
            if let Some(g) = &mut app.game {
                if g.owned[i] {
                    g.selected = i;
                    g.toast(format!("EQUIPPED: {}", TOOLS[i].name));
                } else {
                    g.buy(i);
                }
            }
        }
        Click::SellAll => {
            if let Some(g) = &mut app.game {
                g.sell_all();
            }
        }
        Click::Back => {
            if let Some(g) = &mut app.game {
                if g.state == GameState::Shop {
                    g.state = GameState::Playing;
                    app.set_grab(true);
                }
            }
        }
        Click::SaveLoad => {
            if let (Some(g), Some(w)) = (&app.game, &app.world) {
                let _ = crate::game::save_game(g, w, "save.bin");
                app.toast("GAME SAVED");
            }
        }
    }
}

pub fn on_text(app: &mut App, txt: &str) {
    if !matches!(app.screen, Screen::SeedInput | Screen::JoinInput) {
        return;
    }
    for ch in txt.chars() {
        let ok = ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' || ch == ' ';
        if ok && app.buf.len() < 24 {
            app.buf.push(ch);
        }
    }
}

fn load_save(app: &mut App) {
    if let Some((mut g, straw_count, bitset, removed_count)) = crate::game::load_game("save.bin") {
        let seed = g.seed;
        let w = crate::world::World::new(seed, straw_count);
        app.world = Some(w);
        {
            let w = app.world.as_mut().unwrap();
            let bytes = bitset;
            w.restore_removed(&bytes, removed_count);
        }
        g.state = GameState::Playing;
        app.game = Some(g);
        let r = app.renderer.as_mut().unwrap();
        r.set_straw_instances(app.world.as_ref().unwrap());
        // push all removals to GPU
        let w = app.world.as_ref().unwrap();
        let removed_idx: Vec<u32> = (0..w.straw_count).filter(|&i| w.is_removed(i)).collect();
        r.update_straw_states(&removed_idx);
        app.set_grab(true);
        app.toast("GAME LOADED");
    } else {
        app.toast("NO SAVE FOUND");
    }
}
