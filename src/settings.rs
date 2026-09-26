// Settings with simple INI persistence next to the executable (cwd).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quality {
    Low = 0,
    Medium = 1,
    High = 2,
    Original = 3,
}

impl Quality {
    pub fn from_u8(v: u8) -> Quality {
        match v {
            0 => Quality::Low,
            2 => Quality::High,
            3 => Quality::Original,
            _ => Quality::Medium,
        }
    }
    pub fn next(self) -> Quality {
        match self {
            Quality::Low => Quality::Medium,
            Quality::Medium => Quality::High,
            Quality::High => Quality::Original,
            Quality::Original => Quality::Low,
        }
    }
    pub fn straw_count(&self) -> u32 {
        match self {
            Quality::Low => 250_000,
            Quality::Medium => 450_000,
            Quality::High => 800_000,
            Quality::Original => 5_000_000,
        }
    }
    pub fn msaa(&self) -> u32 {
        match self {
            Quality::Low => 1,
            Quality::Medium => 1,
            Quality::High => 4,
            Quality::Original => 1,
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Quality::Low => "LOW (POTATO)",
            Quality::Medium => "MEDIUM",
            Quality::High => "HIGH",
            Quality::Original => "5M ORIGINAL",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub quality: Quality,
    pub render_scale: f32, // 0.4..=1.0
    pub sensitivity: f32,  // 0.4..=4.0
    pub fps_cap: u32,      // 0 = uncapped (vsync still applies)
    pub clouds: bool,
    pub vsync: bool,
    pub fov: f32, // 60..100
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quality: Quality::Medium,
            render_scale: 1.0,
            sensitivity: 1.0,
            fps_cap: 0,
            clouds: true,
            vsync: true,
            fov: 75.0,
        }
    }
}

impl Settings {
    fn path() -> std::path::PathBuf {
        std::path::Path::new("settings.ini").to_path_buf()
    }

    pub fn load() -> Self {
        let mut s = Self::default();
        let Ok(text) = std::fs::read_to_string(Self::path()) else {
            return s;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let k = k.trim();
            let v = v.trim();
            match k {
                "quality" => s.quality = Quality::from_u8(v.parse().unwrap_or(1)),
                "render_scale" => {
                    s.render_scale = v.parse::<f32>().unwrap_or(1.0).clamp(0.4, 1.0)
                }
                "sensitivity" => s.sensitivity = v.parse::<f32>().unwrap_or(1.0).clamp(0.2, 5.0),
                "fps_cap" => s.fps_cap = v.parse::<u32>().unwrap_or(0),
                "clouds" => s.clouds = v == "1" || v == "true",
                "vsync" => s.vsync = v != "0" && v != "false",
                "fov" => s.fov = v.parse::<f32>().unwrap_or(75.0).clamp(55.0, 100.0),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let mut out = String::new();
        out.push_str(&format!("quality={}\n", self.quality as u8));
        out.push_str(&format!("render_scale={:.2}\n", self.render_scale));
        out.push_str(&format!("sensitivity={:.2}\n", self.sensitivity));
        out.push_str(&format!("fps_cap={}\n", self.fps_cap));
        out.push_str(&format!("clouds={}\n", if self.clouds { 1 } else { 0 }));
        out.push_str(&format!("vsync={}\n", if self.vsync { 1 } else { 0 }));
        out.push_str(&format!("fov={:.1}\n", self.fov));
        let _ = std::fs::write(Self::path(), out);
    }
}
