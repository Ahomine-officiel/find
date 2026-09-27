// Build-time validation: every WGSL shader must parse + validate through naga.
// Catches GPU compile errors without needing a window.

use find_the_needle::game::{format_money, Game, RESEARCH, RESEARCH_LEN, total_levels};
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
    game.research_lv = vec![0u8; RESEARCH_LEN];
    game.research_lv[1] = 1;
    game.research_lv[3] = 2;
    game.owned[3] = true;

    let path = std::env::temp_dir().join("ftn_test_save.bin");
    let path_str = path.to_str().unwrap();
    find_the_needle::game::save_game(&game, &world, path_str).unwrap();

    let (g2, count, bitset, removed) =
        find_the_needle::game::load_game(path_str).expect("save must load");
    std::fs::remove_file(path).ok();
    assert_eq!(g2.money, 12_345);
    assert_eq!(g2.carried, 321);
    assert_eq!(g2.lv(1), 1);
    assert_eq!(g2.lv(3), 2);
    assert!(g2.has(3));
    assert!(!g2.has(2));
    assert!(g2.owned[3]);
    assert_eq!(count, 10_000);
    let mut w2 = World::new(7, 10_000);
    assert!(w2.restore_removed(&bitset, removed), "bitset must restore");
    assert_eq!(w2.removed_count, 0, "fresh dig state round-trips empty");
}

#[test]
fn leveled_research_buys_and_scales() {
    let mut game = Game::new(1, false);
    game.money = 1_000_000_000;
    // card 11 "Bigger Boiler" has 6 levels, price 8000 cents, x1.55 per level;
    // it requires card 9 "Electricity" first
    let def = &RESEARCH[11];
    assert_eq!(def.levels, 6);
    game.buy_research(9); // Electricity (no prerequisite)
    assert!(game.has(9));
    game.buy_research(11);
    assert_eq!(game.lv(11), 1);
    let p1 = game.money;
    game.buy_research(11);
    assert_eq!(game.lv(11), 2);
    let spent2 = 1_000_000_000 - game.money - (1_000_000_000 - p1);
    // level 2 costs 8000 * 1.55 = 12400
    assert_eq!(spent2, 12_400, "level 2 must cost base * 1.55");
    // max it out: further buys are no-ops
    for _ in 0..10 {
        game.buy_research(11);
    }
    assert_eq!(game.lv(11), 6);
    // auto rate must scale with the level
    game.owned[7] = true;
    game.research_lv[11] = 1;
    let r1 = game.auto_rate();
    game.research_lv[11] = 6;
    let r6 = game.auto_rate();
    assert!(r6 > r1 * 1.4, "boiler levels must scale auto rate ({} -> {})", r1, r6);
}

#[test]
fn research_tree_matches_real_game_scale() {
    // the real tree advertises "391 levels" - ours must stay in the same
    // ballpark (300+), across 10 categories with leveled cards
    assert!(RESEARCH.len() >= 70, "tree must have ~70+ cards, got {}", RESEARCH.len());
    let lv = total_levels();
    assert!(lv >= 300, "tree must offer 300+ levels, got {}", lv);
    // every card: at least 1 level, prerequisites in range
    for (i, d) in RESEARCH.iter().enumerate() {
        assert!(d.levels >= 1, "card {} has no levels", i);
        if d.requires != find_the_needle::game::NO_REQ {
            assert!((d.requires as usize) < RESEARCH.len(), "card {} bad req", i);
        }
    }
    // hand work category exists (the real tree's 10th category)
    assert!(RESEARCH.iter().any(|d| matches!(d.cat, find_the_needle::game::RCategory::HandWork)));
    // v2 save compat: old bitset byte layout still loads
    let world = World::new(3, 10_000);
    let mut old = Vec::new();
    old.extend_from_slice(b"FND2");
    old.extend_from_slice(&2u32.to_le_bytes());
    old.extend_from_slice(&3u64.to_le_bytes());
    old.extend_from_slice(&(world.straw_count as u64).to_le_bytes());
    old.extend_from_slice(&5_000u64.to_le_bytes());
    old.extend_from_slice(&0u64.to_le_bytes());
    old.extend_from_slice(&600u64.to_le_bytes());
    old.extend_from_slice(&0u64.to_le_bytes());
    let research_bitset: u64 = (1 << 5) | (1 << 11);
    old.extend_from_slice(&research_bitset.to_le_bytes());
    old.extend_from_slice(&[0u8; 9]);
    old.extend_from_slice(&0u32.to_le_bytes());
    old.push(0u8);
    old.extend_from_slice(&0f64.to_le_bytes());
    old.extend_from_slice(&0u32.to_le_bytes());
    let words = ((world.straw_count as usize) + 63) / 64;
    old.extend_from_slice(&vec![0u8; words * 8]);
    old.extend_from_slice(&0f64.to_le_bytes());
    let path = std::env::temp_dir().join("ftn_test_v2.bin");
    std::fs::write(&path, &old).unwrap();
    let (g2, _, _, _) = find_the_needle::game::load_game(path.to_str().unwrap()).expect("v2 must load");
    std::fs::remove_file(path).ok();
    assert!(g2.has(5) && g2.has(11) && !g2.has(4), "v2 bitset maps to levels");
}

#[test]
fn machines_mesh_fits_static_world() {
    // the merged static world (barn + machines + outdoors) must stay within
    // u16 index space, and every machine position must sit outside the pile
    let barn = find_the_needle::mesh::barn_mesh();
    let machines = find_the_needle::mesh::machines_mesh();
    let outdoor = find_the_needle::mesh::outdoor_mesh();
    let total = barn.0.len() + machines.0.len() + outdoor.0.len();
    assert!(
        total < 65535,
        "merged world {} verts exceeds u16 index space",
        total
    );
    assert!(!machines.0.is_empty(), "machines mesh must not be empty");
    let max_idx = machines.1.iter().copied().max().unwrap_or(0) as usize;
    assert!(max_idx < machines.0.len(), "index out of bounds in machines mesh");
    // belt ring stays outside the pile dome and inside the walls
    for &(x, z) in find_the_needle::mesh::BELT_RING.iter() {
        let r = (x * x + z * z).sqrt();
        assert!(r >= 16.0, "ring point ({x},{z}) inside pile (r={r})");
        assert!(x.abs() < 22.8 && z.abs() < 16.9, "ring point ({x},{z}) outside walls");
    }
}
