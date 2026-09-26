// Shader sources embedded at compile time. Zero runtime file IO.

pub const MAIN_WGSL: &str = include_str!("shaders/main.wgsl");
pub const SKY_WGSL: &str = include_str!("shaders/sky.wgsl");
pub const HUD_WGSL: &str = include_str!("shaders/hud.wgsl");
pub const BLIT_WGSL: &str = include_str!("shaders/blit.wgsl");
