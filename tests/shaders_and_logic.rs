// Build-time validation: every WGSL shader must parse + validate through naga.
// Catches GPU compile errors without needing a window.

use find_the_needle::game::{format_money, Game};
use find_the_needle::world::World;
use glam::Vec3;

#[test]
fn validate_wgsl_shaders() {
    let files = [
        ("main.wgsl", include_str!("../src/shaders/main.wgsl")),
        ("sky.wgsl", include_str!("../src/shaders/sky.wgsl")),
        ("hud.wgsl", include_str!("../src/shaders/hud.wgsl")),
        ("blit.wgsl", include_str!("../src/shaders/blit.wgsl")),
    ];
    for (name, src) in files {
        let module = naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("{name} failed to parse: {e:?}"));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        let _info = validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name} failed validation: {e:?}"));
        // validation passing means the module is well-formed and entry points resolve
    }
}

#[test]
fn world_generation_and_digging_smoke_test() {
    let mut world = World::new(42, 50_000);
    assert_eq!(world.straw_count, 50_000);
    assert_eq!(world.straws.len(), 50_000);

    // needle is inside the pile bounds
    let n = &world.needle;
    assert!(n.pos.x.abs() <= 16.0 && n.pos.z.abs() <= 16.0 && n.pos.y <= 11.0);

    // dig from above: camera above surface aiming down
    let eye = Vec3::new(0.0, 14.0, 0.0);
    let dir = Vec3::new(0.0, -1.0, 0.0);
    let (hit, removed) = world.scoop(eye, dir, 0.4, 200);
    assert!(hit.is_some(), "ray straight down must hit the pile");
    assert!(!removed.is_empty(), "scoop must remove straws");

    // game economy: money math
    let mut game = Game::new(42, false);
    game.carried = 600;
    let earned = game.sell_all();
    // 600 strands at $0.022/strand = $13.20 = 1320 cents
    assert_eq!(earned, 1320, "600 x $0.022 must equal $13.20");
    assert_eq!(game.money, 1320);
    assert_eq!(game.carried, 0);

    // formatting
    assert_eq!(format_money(1320), "$13.20");
    assert_eq!(format_money(1397600), "$13,976");
}

#[test]
fn save_roundtrip() {
    let world = World::new(7, 10_000);
    let mut game = Game::new(7, false);
    game.money = 12_345;
    game.carried = 321;
    game.research = 0b1010;
    game.owned[3] = true;

    let path = std::env::temp_dir().join("ftn_test_save.bin");
    let path_str = path.to_str().unwrap();
    find_the_needle::game::save_game(&game, &world, path_str).unwrap();

    let (g2, count, bitset, removed) =
        find_the_needle::game::load_game(path_str).expect("save must load");
    std::fs::remove_file(path).ok();
    assert_eq!(g2.money, 12_345);
    assert_eq!(g2.carried, 321);
    assert_eq!(g2.research, 0b1010);
    assert!(g2.owned[3]);
    assert_eq!(count, 10_000);
    let mut w2 = World::new(7, 10_000);
    assert!(w2.restore_removed(&bitset, removed), "bitset must restore");
    assert_eq!(w2.removed_count, 0, "fresh dig state round-trips empty");
}
