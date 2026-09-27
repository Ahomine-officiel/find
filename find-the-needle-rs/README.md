# FIND THE NEEDLE — Rust + wgpu remake

A faithful, from-scratch recreation of the viral **"Find The Needle"** hay-pile
needle hunt (Steam app 5160800, clone of *Needle In A Haystack Simulator*), written
in pure **Rust + wgpu**. No engine. No textures. No external assets. Everything is
procedural geometry and math.

Built for potato PCs: the original ran at **16 FPS** on the target machine — this
one renders the whole scene in **~6 draw calls** and should run circles around it.

## The game

You are locked in a barn with a giant golden haystack. Somewhere inside is a
single sewing needle. Dig through hundreds of thousands of individually rendered
straw strands, uncover valuables, sell hay at **$0.022 per strand**, buy tools,
and research your way to automation:

- **The barn** — corrugated walls, curved roof beams, sandy floor, sliding door
  with red LED clock and the HAYWAY CO. sign. The pile pierces the roof.
- **The tools** — metal detector (it pings within its radius), gloves, pitchfork,
  shovel, giant magnet, hay baler, hay vacuum.
- **SELL HAY stall** — conveyor belt, cash register, the `$0.022 PER STRAND`
  blackboard. Walk up, press E, get paid.
- **YARD RESEARCH** — the gold-on-dark tech tree (categories, tiers, prices,
  OWNED/DONE states) with 33 upgrades across 9 categories.
- **The needle** — 7 cm long, buried somewhere in the pile. The detector only
  says *where to look*. Finding it is on you.
- **PURE MODE** — no tools at all. One strand at a time. The true experience.

## Controls

| Key | Action |
|-----|--------|
| WASD | Move |
| Shift | Sprint |
| Space | Jump |
| Mouse | Look (click the window to lock the pointer) |
| LMB | Dig / pick up the needle / take a valuable |
| RMB | Zoom (inspect up close) |
| E | Interact (sell hay / browse tools) |
| Tab | YARD RESEARCH tech tree |
| F5 / F9 | Save / load |
| F3 | FPS + stats overlay |
| Esc | Pause |

## Build & run

```bash
cargo run --release
```

That's it. No assets to download, nothing to configure. On Linux you need the
usual GPU drivers; the game targets wgpu's Vulkan / Metal / DX12 / GL backends.

### Quality presets

In the menu: **QUALITY** cycles `LOW (POTATO) 250k` / `MEDIUM 450k` /
`HIGH 800k` straw strands. `settings.ini` is created next to the executable.

## Performance design

- One instanced draw call for the entire straw pile (2 triangles per straw,
  orientation + length + tint packed into 8 bytes of instance data).
- The barn + outdoors are merged into ONE static mesh (1 draw call, ~23k verts).
- Dynamic props (needle, valuables, machines, belt cargo, viewmodels) share a
  single instanced pipeline.
- Zero texture fetches: sand, corrugated metal, wood planks, grass, conveyor
  cleats are all procedural fragment-shader noise. Fonts are a 5x7 bitmap baked
  into a 128x64 R8 atlas at startup.
- No shadow maps, no post processing, no allocations in the frame loop.
- Optional offscreen render-scale + vsync toggle.

## Project layout

```
src/
  main.rs    winit event loop
  app.rs     gameplay, input, HUD, menus
  game.rs    economy, tools, research tree, saves
  world.rs   straw generation, spatial grid, digging, needle placement
  mesh.rs    all procedural geometry (barn, stalls, terrain, tools, needle)
  render.rs  wgpu renderer (pipelines, instancing, blit)
  hud.rs     bitmap font HUD
  shaders/   main.wgsl, sky.wgsl, hud.wgsl, blit.wgsl
tests/       shader validation (naga) + gameplay logic tests
```

## Testing

```bash
cargo test --release
```

- All WGSL shaders are parsed and validated with naga in CI-style tests.
- World generation, digging, the `$0.022` economy and save/load round-trips are
  unit tested.

## Publishing to GitHub

This repository is prepared to push to `https://github.com/Ahomine_officiel/find`:

```bash
git remote -v                     # origin already set
gh auth login                     # authenticate YOURSELF, locally
git push -u origin main
```

Never paste tokens into chats or files — GitHub revokes exposed PATs
automatically, and a leaked token is a leaked account.
