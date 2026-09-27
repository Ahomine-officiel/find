// Game state: tools, economy, research (YARD RESEARCH), shop, win flow,
// HUD data, persistence. Money is stored in cents (i64) so $0.022/strand
// behaves exactly like the real game's price board.

use crate::mesh::ValuableKind;
use crate::rng::Rng;
use crate::world::World;
use glam::Vec3;

/// $0.022 per strand. Money is stored in CENTS (i64); strand price in
/// MILLI-CENTS (1/1000 cent): $0.022 = 2.2 cents = 2200 millicents.
pub const STRAND_PRICE_MILLICENTS: i64 = 2200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    Menu,
    Playing,
    Shop,
    Research,
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
    pub price: i64, // cents
    pub desc: &'static str,
}

pub const TOOLS: [ToolDef; 9] = [
    ToolDef { tool: Tool::Hands, name: "BARE HANDS", price: 0, desc: "Pick strands one at a time." },
    ToolDef { tool: Tool::Detector, name: "METAL DETECTOR", price: 0, desc: "Pings within a 2m radius. Starter tool." },
    ToolDef { tool: Tool::Gloves, name: "GLOVES", price: 6000, desc: "Grab 3 strands per pick." },
    ToolDef { tool: Tool::Pitchfork, name: "PITCHFORK", price: 32000, desc: "Scoop ~14 strands at once." },
    ToolDef { tool: Tool::Shovel, name: "SHOVEL", price: 90000, desc: "Scoop ~50 strands at once." },
    ToolDef { tool: Tool::DetectorII, name: "DETECTOR MK-II", price: 400000, desc: "Pings within a 6m radius." },
    ToolDef { tool: Tool::Magnet, name: "GIANT MAGNET", price: 80000, desc: "Pulls nearby valuables to you." },
    ToolDef { tool: Tool::Baler, name: "HAY BALER", price: 250000, desc: "Eats 25 strands/sec, sells 70%." },
    ToolDef { tool: Tool::Vacuum, name: "HAY VACUUM MK-II", price: 1200000, desc: "Eats 120 strands/sec, sells 70%." },
];

// ---------------------------------------------------------------------------
// YARD RESEARCH tree (representative slice of the real 391 levels)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RCategory {
    YardBuilding,
    HayLines,
    Power,
    Processing,
    Selling,
    Fitness,
    Automation,
    Prospecting,
    Water,
    HandWork,
}

impl RCategory {
    pub const ALL: [RCategory; 10] = [
        RCategory::YardBuilding,
        RCategory::HayLines,
        RCategory::Power,
        RCategory::Processing,
        RCategory::Automation,
        RCategory::Selling,
        RCategory::Fitness,
        RCategory::Prospecting,
        RCategory::Water,
        RCategory::HandWork,
    ];
    pub fn name(&self) -> &'static str {
        match self {
            RCategory::YardBuilding => "YARD BUILDING",
            RCategory::HayLines => "HAY LINES",
            RCategory::Power => "POWER",
            RCategory::Processing => "PROCESSING",
            RCategory::Automation => "AUTOMATION",
            RCategory::Selling => "SELLING",
            RCategory::Fitness => "FITNESS",
            RCategory::Prospecting => "PROSPECTING",
            RCategory::Water => "WATER",
            RCategory::HandWork => "HAND WORK",
        }
    }
    pub fn hint(&self) -> &'static str {
        match self {
            RCategory::YardBuilding => "4 of 12 bought - 3 ready\nplatforms, walls and roots",
            RCategory::HayLines => "belts, splitters and the launcher",
            RCategory::Power => "the generator, and its wires",
            RCategory::Processing => "wraps, bricks and the silo",
            RCategory::Automation => "robots and drones",
            RCategory::Selling => "prices and the market",
            RCategory::Fitness => "walk faster, dig stronger",
            RCategory::Prospecting => "detectors and luck",
            RCategory::Water => "sprinklers and the trough",
            RCategory::HandWork => "picks, dig speed and buckets",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum REffect {
    ConveyorPlans,   // visual: belt + balls active
    FasterBelt,      // auto-sale throughput +
    AlternatingSplitter,
    BeltJoiner,
    PriorityArm,
    TubeLauncher,
    Electricity,     // required for automation
    PowerPole,
    UndergroundCable,
    BiggerBoiler,    // automation rate +
    BiggerFirebox,   // automation rate +
    BalerMachine,    // unlock baler
    LargerBaleChamber,
    FasterBalePress,
    RaiseBaleQuality, // sale % +
    ExtendTheShed,    // visual shed
    WasteLessMaterial, // sale % +
    BetterPrices1,
    BetterPrices2,
    BetterPrices3,
    WalkFaster1,
    WalkFaster2,
    StrongerArms,     // carry cap +
    StrongerBack,     // carry cap +
    DetectorPlus,     // detector radius +
    LuckyCharm,       // more valuables
    Sprinkler,        // visual
    WaterTrough,      // flavor
    // ---- expansion cards (effects are applied by RESEARCH INDEX, these are
    // documentation of what each card does) ----
    HandsPick, DigSpeed, BucketRigs, SprintTraining, WarmHands, CompoundLevers,
    YardLighting, SecondShed, ToolRacks, SecondYard,
    LongerBelts, BeltLube, WideBelts, SecondSpur, TrafficControl, SortingAlgorithm,
    Crossroads, PneumaticTubes,
    GenEfficiency, SteamTurbine, LongerSpans, LongerDrops, BackupGenerator,
    GridRedundancy,
    PulpLine, BrickPress, PaperMill, Pelletizer, TheSilo, BaleStacker,
    HydraulicPress, BaleExit,
    ArmServo, ExtraArms, ScannerOverclock, DroneFleet, ArmJoints,
    OnlineMarket, LoyalCustomers, PremiumBrand, GlobalShipping, SortLicense,
    DeepScan, TreasureMaps, IrrigationLoop, DustControl, BaleMarathon,
}

#[derive(Clone, Copy)]
pub struct ResearchDef {
    pub cat: RCategory,
    pub tier: u32,     // column: 0 = START, 1 = 1 STEP...
    pub name: &'static str,
    pub price: i64,    // cents (price of level 1; higher levels scale by LEVEL_COST_MUL)
    pub requires: u32, // index into RESEARCH of the prerequisite (u32::MAX = none)
    pub levels: u32,   // how many times this card can be bought (like the real "0/6" cards)
    pub effect: REffect,
}

macro_rules! rd {
    ($cat:expr, $tier:expr, $name:expr, $price:expr, $req:expr, $eff:expr) => {
        ResearchDef { cat: $cat, tier: $tier, name: $name, price: $price, requires: $req, levels: 1, effect: $eff }
    };
    ($cat:expr, $tier:expr, $name:expr, $price:expr, $req:expr, lv $lv:expr, $eff:expr) => {
        ResearchDef { cat: $cat, tier: $tier, name: $name, price: $price, requires: $req, levels: $lv, effect: $eff }
    };
}

/// Cost of level n+1 = base price * LEVEL_COST_MUL^n ("generator prices scale
/// exponentially" like the real tree).
pub const LEVEL_COST_MUL: f32 = 1.55;

pub fn price_for_level(def: &ResearchDef, current_level: u32) -> i64 {
    let m = LEVEL_COST_MUL.powi(current_level as i32);
    ((def.price as f32 * m) as i64).max(1)
}

pub fn total_levels() -> u32 {
    RESEARCH.iter().map(|d| d.levels).sum()
}

pub const NO_REQ: u32 = u32::MAX;

pub const RESEARCH_LEN: usize = RESEARCH.len();

pub const RESEARCH: &[ResearchDef] = &[
    // ---- indices 0..33 : the original slice (order MUST NOT change - the
    // renderer and tests reference cards by index) ----
    // YARD BUILDING
    rd!(RCategory::YardBuilding, 0, "Conveyor Plans", 15000, NO_REQ, REffect::ConveyorPlans),
    rd!(RCategory::YardBuilding, 1, "Extend the Shed", 15000, 0, lv 4, REffect::ExtendTheShed),
    rd!(RCategory::YardBuilding, 1, "Waste Less Material", 10000, 0, lv 3, REffect::WasteLessMaterial),
    rd!(RCategory::YardBuilding, 2, "Yard Platforms", 20000, 2, lv 2, REffect::WaterTrough),
    // HAY LINES
    rd!(RCategory::HayLines, 1, "Faster Belt Motor", 1500, 0, lv 2, REffect::FasterBelt),
    rd!(RCategory::HayLines, 1, "Alternating Splitter", 5000, 0, REffect::AlternatingSplitter),
    rd!(RCategory::HayLines, 2, "Belt Joiner", 8000, 5, REffect::BeltJoiner),
    rd!(RCategory::HayLines, 2, "Priority Arm", 12000, 5, REffect::PriorityArm),
    rd!(RCategory::HayLines, 3, "Tube Launcher Plans", 26000, 6, REffect::TubeLauncher),
    // POWER
    rd!(RCategory::Power, 0, "Electricity", 9000, NO_REQ, REffect::Electricity),
    rd!(RCategory::Power, 1, "Power Pole", 4000, 9, REffect::PowerPole),
    rd!(RCategory::Power, 1, "Bigger Boiler", 8000, 9, lv 6, REffect::BiggerBoiler),
    rd!(RCategory::Power, 1, "Bigger Firebox", 4000, 9, lv 3, REffect::BiggerFirebox),
    rd!(RCategory::Power, 2, "Underground Cable", 300000, 10, REffect::UndergroundCable),
    // PROCESSING
    rd!(RCategory::Processing, 0, "Baler Machine", 60000, NO_REQ, REffect::BalerMachine),
    rd!(RCategory::Processing, 1, "Larger Bale Chamber", 8000, 14, lv 6, REffect::LargerBaleChamber),
    rd!(RCategory::Processing, 1, "Faster Bale Press", 3000, 14, lv 6, REffect::FasterBalePress),
    rd!(RCategory::Processing, 1, "Raise Bale Quality", 5000, 14, lv 3, REffect::RaiseBaleQuality),
    rd!(RCategory::Processing, 2, "Hay Wrapper", 22000, 16, REffect::WasteLessMaterial),
    // AUTOMATION
    rd!(RCategory::Automation, 0, "Robot Arms", 120000, 9, REffect::PriorityArm),
    rd!(RCategory::Automation, 1, "Vacuum Line", 300000, 19, REffect::FasterBelt),
    rd!(RCategory::Automation, 1, "Drone Scout", 150000, 19, REffect::LuckyCharm),
    // SELLING
    rd!(RCategory::Selling, 0, "Market Stand", 5000, NO_REQ, REffect::BetterPrices1),
    rd!(RCategory::Selling, 1, "Bulk Contracts", 25000, 22, REffect::BetterPrices2),
    rd!(RCategory::Selling, 2, "Export License", 80000, 23, REffect::BetterPrices3),
    // FITNESS
    rd!(RCategory::Fitness, 0, "Cardio", 3000, NO_REQ, REffect::WalkFaster1),
    rd!(RCategory::Fitness, 1, "Marathon Legs", 12000, 25, REffect::WalkFaster2),
    rd!(RCategory::Fitness, 0, "Stronger Arms", 8000, NO_REQ, REffect::StrongerArms),
    rd!(RCategory::Fitness, 1, "Stronger Back", 18000, 27, REffect::StrongerBack),
    // PROSPECTING
    rd!(RCategory::Prospecting, 0, "Detector Tuning", 6000, NO_REQ, REffect::DetectorPlus),
    rd!(RCategory::Prospecting, 1, "Lucky Charm", 20000, 29, REffect::LuckyCharm),
    rd!(RCategory::Prospecting, 2, "Gold Panning", 45000, 30, REffect::LuckyCharm),
    // WATER
    rd!(RCategory::Water, 0, "Sprinkler", 4000, NO_REQ, REffect::Sprinkler),
    rd!(RCategory::Water, 1, "Water Trough", 6000, 32, REffect::WaterTrough),

    // ---- indices 34.. : expansion to mirror the real 391-level tree ----
    // HAND WORK
    rd!(RCategory::HandWork, 0, "Pick Strands", 4000, NO_REQ, lv 8, REffect::HandsPick),
    rd!(RCategory::HandWork, 0, "Dig Speed", 6000, NO_REQ, lv 10, REffect::DigSpeed),
    rd!(RCategory::HandWork, 1, "Bucket Rigs", 12000, 34, lv 6, REffect::BucketRigs),
    rd!(RCategory::HandWork, 1, "Sprint Training", 8000, 25, lv 6, REffect::SprintTraining),
    rd!(RCategory::HandWork, 1, "Warm Hands", 9000, 34, lv 5, REffect::WarmHands),
    rd!(RCategory::HandWork, 2, "Compound Levers", 20000, 35, lv 4, REffect::CompoundLevers),
    // YARD BUILDING (expansion)
    rd!(RCategory::YardBuilding, 2, "Yard Lighting", 7000, 3, lv 5, REffect::YardLighting),
    rd!(RCategory::YardBuilding, 3, "Second Shed", 25000, 1, lv 4, REffect::SecondShed),
    rd!(RCategory::YardBuilding, 2, "Tool Racks", 15000, 1, lv 4, REffect::ToolRacks),
    rd!(RCategory::YardBuilding, 4, "Second Yard", 150000, 40, lv 3, REffect::SecondYard),
    // HAY LINES (expansion)
    rd!(RCategory::HayLines, 2, "Longer Belts", 9000, 0, lv 8, REffect::LongerBelts),
    rd!(RCategory::HayLines, 3, "Belt Lubrication", 5000, 4, lv 6, REffect::BeltLube),
    rd!(RCategory::HayLines, 2, "Wide Belts", 13000, 0, lv 6, REffect::WideBelts),
    rd!(RCategory::HayLines, 3, "Second Spur Line", 18000, 5, lv 4, REffect::SecondSpur),
    rd!(RCategory::HayLines, 4, "Traffic Control", 30000, 6, lv 6, REffect::TrafficControl),
    rd!(RCategory::HayLines, 4, "Sorting Algorithm", 40000, 7, lv 6, REffect::SortingAlgorithm),
    rd!(RCategory::HayLines, 5, "Conveyor Crossroads", 80000, 46, lv 6, REffect::Crossroads),
    rd!(RCategory::HayLines, 5, "Pneumatic Tubes", 90000, 8, lv 5, REffect::PneumaticTubes),
    // POWER (expansion)
    rd!(RCategory::Power, 2, "Generator Efficiency", 10000, 9, lv 8, REffect::GenEfficiency),
    rd!(RCategory::Power, 3, "Steam Turbine", 40000, 11, lv 5, REffect::SteamTurbine),
    rd!(RCategory::Power, 3, "Longer Spans", 10000, 10, lv 8, REffect::LongerSpans),
    rd!(RCategory::Power, 3, "Longer Drops", 7000, 10, lv 6, REffect::LongerDrops),
    rd!(RCategory::Power, 4, "Backup Generator", 60000, 48, lv 5, REffect::BackupGenerator),
    rd!(RCategory::Power, 4, "Grid Redundancy", 70000, 50, lv 5, REffect::GridRedundancy),
    // PROCESSING (expansion)
    rd!(RCategory::Processing, 2, "Hay Pulp Line", 30000, 14, lv 6, REffect::PulpLine),
    rd!(RCategory::Processing, 2, "Eco Brick Press", 35000, 14, lv 7, REffect::BrickPress),
    rd!(RCategory::Processing, 3, "Paper Mill", 45000, 53, lv 4, REffect::PaperMill),
    rd!(RCategory::Processing, 3, "Pelletizer", 50000, 54, lv 4, REffect::Pelletizer),
    rd!(RCategory::Processing, 3, "The Silo", 80000, 14, lv 3, REffect::TheSilo),
    rd!(RCategory::Processing, 3, "Bale Stacker", 22000, 15, lv 3, REffect::BaleStacker),
    rd!(RCategory::Processing, 4, "Hydraulic Press", 65000, 16, lv 5, REffect::HydraulicPress),
    rd!(RCategory::Processing, 4, "Bale Conveyor Exit", 40000, 58, lv 4, REffect::BaleExit),
    // AUTOMATION (expansion)
    rd!(RCategory::Automation, 1, "Arm Servo Upgrade", 25000, 19, lv 7, REffect::ArmServo),
    rd!(RCategory::Automation, 2, "Extra Arm Batch", 50000, 59, lv 8, REffect::ExtraArms),
    rd!(RCategory::Automation, 2, "Scanner Overclock", 30000, 17, lv 5, REffect::ScannerOverclock),
    rd!(RCategory::Automation, 2, "Drone Fleet", 45000, 21, lv 2, REffect::DroneFleet),
    rd!(RCategory::Automation, 3, "Arm Precision Joints", 60000, 60, lv 5, REffect::ArmJoints),
    // SELLING (expansion)
    rd!(RCategory::Selling, 2, "Online Marketplace", 20000, 22, lv 8, REffect::OnlineMarket),
    rd!(RCategory::Selling, 3, "Loyal Customers", 15000, 63, lv 5, REffect::LoyalCustomers),
    rd!(RCategory::Selling, 3, "Premium Branding", 35000, 23, lv 2, REffect::PremiumBrand),
    rd!(RCategory::Selling, 4, "Global Shipping", 90000, 24, lv 4, REffect::GlobalShipping),
    rd!(RCategory::Selling, 4, "Hay Sorting License", 120000, 65, lv 3, REffect::SortLicense),
    // PROSPECTING (expansion)
    rd!(RCategory::Prospecting, 2, "Deep Scan Mode", 25000, 29, lv 3, REffect::DeepScan),
    rd!(RCategory::Prospecting, 2, "Treasure Maps", 30000, 30, lv 3, REffect::TreasureMaps),
    // WATER (expansion)
    rd!(RCategory::Water, 1, "Irrigation Loop", 9000, 32, lv 4, REffect::IrrigationLoop),
    rd!(RCategory::Water, 1, "Dust Control", 6000, 32, lv 5, REffect::DustControl),
    // FITNESS (expansion)
    rd!(RCategory::Fitness, 2, "Hay Bale Marathon", 18000, 26, lv 4, REffect::BaleMarathon),
];

pub struct Toast {
    pub text: String,
    pub life: f32,
}

pub struct Game {
    pub state: GameState,
    pub seed: u64,
    pub pure_mode: bool,
    pub money: i64, // cents
    pub owned: [bool; 9],
    pub selected: usize,
    pub research_lv: Vec<u8>, // level bought per RESEARCH card (0 = not bought)
    pub time: f64,
    pub dig_cd: f32,
    pub removed_count: u32,
    pub carried: u32,        // hay strands in the bucket
    pub capacity: u32,       // bucket capacity (600 base)
    pub earned_total: i64,
    pub valuables_bag: Vec<(ValuableKind, i64)>,
    pub bag_value: i64,
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
    // UI navigation
    pub ui_index: usize,     // generic list index (shop / research)
    pub research_cat: usize,
    pub research_scroll: f32,
    pub menu_index: usize,
    pub shop_index: usize,
    pub shake: f32,
}

impl Game {
    pub fn new(seed: u64, pure_mode: bool) -> Self {
        let mut owned = [false; 9];
        owned[0] = true; // hands
        owned[1] = true; // detector - starter tool
        Game {
            state: GameState::Menu,
            seed,
            pure_mode,
            money: 0,
            owned,
            selected: 1,
            research_lv: vec![0u8; RESEARCH.len()],
            time: 0.0,
            dig_cd: 0.0,
            removed_count: 0,
            carried: 0,
            capacity: 600,
            earned_total: 0,
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
            ui_index: 0,
            research_cat: 0,
            research_scroll: 0.0,
            menu_index: 0,
            shop_index: 0,
            shake: 0.0,
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

    // ----- research helpers -----

    /// Card bought at least once?
    #[inline]
    pub fn has(&self, i: u32) -> bool {
        self.lv(i) > 0
    }

    /// Level bought for card i (0 = not bought).
    #[inline]
    pub fn lv(&self, i: u32) -> u32 {
        match self.research_lv.get(i as usize) {
            Some(&l) => l as u32,
            None => 0,
        }
    }

    /// Total LEVELS bought (the real tree counts levels, not cards:
    /// "17 of 391 levels bought").
    pub fn research_count(&self) -> u32 {
        self.research_lv.iter().map(|&l| l as u32).sum()
    }

    pub fn strand_price_mul(&self) -> f32 {
        let mut m = 1.0;
        if self.has(22) { m *= 1.5; } // BetterPrices1
        if self.has(23) { m *= 1.6; } // BetterPrices2
        if self.has(24) { m *= 1.8; } // BetterPrices3
        if self.has(18) { m *= 1.15; } // Hay Wrapper
        m *= 1.0 + 0.08 * self.lv(53) as f32; // Hay Pulp Line
        m *= 1.0 + 0.05 * self.lv(54) as f32; // Eco Brick Press
        m *= 1.0 + 0.05 * self.lv(55) as f32; // Paper Mill
        m *= 1.0 + 0.05 * self.lv(56) as f32; // Pelletizer
        m *= 1.0 + 0.04 * self.lv(63) as f32; // Online Marketplace
        m *= 1.0 + 0.03 * self.lv(64) as f32; // Loyal Customers
        m *= 1.0 + 0.05 * self.lv(65) as f32; // Premium Branding
        m *= 1.0 + 0.05 * self.lv(76) as f32; // Global Shipping
        m *= 1.0 + 0.04 * self.lv(77) as f32; // Hay Sorting License
        m *= 1.0 + 0.03 * self.lv(69) as f32; // Dust Control
        m
    }

    pub fn capacity(&self) -> u32 {
        let mut c = self.capacity;
        if self.has(27) { c += 150; } // StrongerArms
        if self.has(28) { c += 300; } // StrongerBack
        c += 60 * self.lv(36);        // Bucket Rigs
        c += 40 * self.lv(44);        // Wide Belts
        c += 120 * self.lv(40);       // Second Shed
        c += 200 * self.lv(57);       // The Silo
        c += 150 * self.lv(80);       // Second Yard
        c
    }

    pub fn walk_speed(&self) -> f32 {
        let mut s = 4.3;
        if self.has(25) { s += 1.0; }
        if self.has(26) { s += 1.4; }
        s += 0.3 * self.lv(37) as f32;  // Sprint Training
        s += 0.25 * self.lv(79) as f32; // Hay Bale Marathon
        s
    }

    pub fn auto_rate(&self) -> f32 {
        if self.pure_mode { return 0.0; }
        let mut rate = 0.0f32;
        if self.owned[7] { rate += 25.0; }
        if self.owned[8] { rate += 120.0; }
        rate *= 1.0 + 0.10 * self.lv(11) as f32; // Bigger Boiler
        rate *= 1.0 + 0.07 * self.lv(12) as f32; // Bigger Firebox
        rate *= 1.0 + 0.06 * self.lv(15) as f32; // Larger Bale Chamber
        rate *= 1.0 + 0.05 * self.lv(16) as f32; // Faster Bale Press
        if self.has(20) { rate *= 1.4; }          // Vacuum Line
        rate *= 1.0 + 0.03 * self.lv(39) as f32; // Yard Lighting
        rate *= 1.0 + 0.04 * self.lv(42) as f32; // Longer Belts
        rate *= 1.0 + 0.05 * self.lv(43) as f32; // Belt Lubrication
        rate *= 1.0 + 0.06 * self.lv(45) as f32; // Second Spur Line
        rate *= 1.0 + 0.05 * self.lv(46) as f32; // Traffic Control
        rate *= 1.0 + 0.05 * self.lv(47) as f32; // Sorting Algorithm
        rate *= 1.0 + 0.05 * self.lv(48) as f32; // Generator Efficiency
        rate *= 1.0 + 0.08 * self.lv(49) as f32; // Steam Turbine
        rate *= 1.0 + 0.02 * self.lv(50) as f32; // Longer Spans
        rate *= 1.0 + 0.02 * self.lv(51) as f32; // Longer Drops
        rate *= 1.0 + 0.12 * self.lv(52) as f32; // Backup Generator
        rate *= 1.0 + 0.04 * self.lv(72) as f32; // Grid Redundancy
        rate *= 1.0 + 0.06 * self.lv(58) as f32; // Bale Stacker
        rate *= 1.0 + 0.05 * self.lv(73) as f32; // Hydraulic Press
        rate *= 1.0 + 0.05 * self.lv(74) as f32; // Bale Conveyor Exit
        rate *= 1.0 + 0.06 * self.lv(59) as f32; // Arm Servo Upgrade
        rate *= 1.0 + 0.08 * self.lv(60) as f32; // Extra Arm Batch
        rate *= 1.0 + 0.05 * self.lv(61) as f32; // Scanner Overclock
        rate *= 1.0 + 0.05 * self.lv(62) as f32; // Drone Fleet
        rate *= 1.0 + 0.07 * self.lv(75) as f32; // Arm Precision Joints
        rate *= 1.0 + 0.05 * self.lv(70) as f32; // Conveyor Crossroads
        rate *= 1.0 + 0.06 * self.lv(71) as f32; // Pneumatic Tubes
        rate *= 1.0 + 0.04 * self.lv(68) as f32; // Irrigation Loop
        rate
    }

    pub fn sale_fraction(&self) -> f32 {
        let mut f: f32 = 0.70;
        f += 0.03 * self.lv(17) as f32; // RaiseBaleQuality (lv3 = +0.09)
        f += 0.02 * self.lv(2) as f32;  // WasteLessMaterial (lv3 = +0.06)
        f.min(0.95)
    }

    pub fn detector_radius(&self) -> Option<f32> {
        if self.pure_mode { return None; }
        match self.current_tool() {
            Tool::Detector => Some(2.0 + 0.5 * self.lv(29) as f32 + self.lv(66) as f32),
            Tool::DetectorII => Some(6.0 + self.lv(66) as f32),
            _ => None,
        }
    }

    pub fn pick_n(&self) -> u32 {
        if self.pure_mode { return 1; }
        let base = match self.current_tool() {
            Tool::Hands => 1,
            Tool::Gloves => 3,
            _ => 1,
        };
        base + self.lv(34) // Pick Strands levels add to every pick
    }

    pub fn scoop_params(&self) -> Option<(f32, u32)> {
        if self.pure_mode { return None; }
        let extra = self.lv(34) as u32;
        match self.current_tool() {
            Tool::Pitchfork => Some((0.26, 14 + extra)),
            Tool::Shovel => Some((0.45, 50 + extra)),
            _ => None,
        }
    }

    pub fn cooldown(&self) -> f32 {
        let lv = self.lv(35) + self.lv(41); // Dig Speed + Tool Racks
        let mul = (1.0 - 0.045 * lv as f32).max(0.60);
        let base = match self.current_tool() {
            Tool::Hands => 0.30,
            Tool::Gloves => 0.26,
            Tool::Pitchfork => 0.42,
            Tool::Shovel => 0.55,
            _ => 0.30,
        };
        base * mul
    }

    pub fn buy_tool(&mut self, slot: usize) {
        if self.pure_mode {
            self.toast("PURE MODE: NO TOOLS. JUST YOU AND THE PILE.");
            return;
        }
        let def = TOOLS[slot];
        if def.price == 0 || self.owned[slot] {
            self.selected = slot;
            self.toast(format!("EQUIPPED: {}", def.name));
            return;
        }
        if def.tool == Tool::Baler && !self.has(14) {
            self.toast("RESEARCH 'BALER MACHINE' FIRST (YARD RESEARCH)");
            return;
        }
        if def.tool == Tool::Vacuum && !self.has(20) {
            self.toast("RESEARCH 'VACUUM LINE' FIRST (YARD RESEARCH)");
            return;
        }
        if self.money >= def.price {
            self.money -= def.price;
            self.owned[slot] = true;
            self.selected = slot;
            self.toast(format!("BOUGHT: {} (-{})", def.name, format_money(def.price)));
        } else {
            self.toast(format!("NOT ENOUGH MONEY ({} / {})", format_money(self.money), format_money(def.price)));
        }
    }

    pub fn buy_research(&mut self, idx: u32) {
        if idx as usize >= RESEARCH.len() {
            return;
        }
        let def = &RESEARCH[idx as usize];
        let cur = self.lv(idx);
        if cur >= def.levels {
            return; // already maxed
        }
        if def.requires != NO_REQ && !self.has(def.requires) {
            self.toast("LOCKED - BUY THE PREREQUISITE FIRST");
            return;
        }
        let price = price_for_level(def, cur);
        if self.money >= price {
            self.money -= price;
            self.research_lv[idx as usize] = (cur + 1) as u8;
            if def.levels > 1 {
                self.toast(format!(
                    "RESEARCHED: {} LV {}/{} (-{})",
                    def.name,
                    cur + 1,
                    def.levels,
                    format_money(price)
                ));
            } else {
                self.toast(format!("RESEARCHED: {} (-{})", def.name, format_money(price)));
            }
        } else {
            self.toast(format!(
                "NOT ENOUGH MONEY ({} / {})",
                format_money(self.money),
                format_money(price)
            ));
        }
    }

    /// Sell carried strands + valuables at the SELL HAY stall. Returns cents earned.
    pub fn sell_all(&mut self) -> i64 {
        let mut total: i64 = 0;
        if self.carried > 0 {
            // strand price: $0.022/strand = 22 millicents, times research multiplier
            let per_strand_millicents = (STRAND_PRICE_MILLICENTS as f32 * self.strand_price_mul()) as i64;
            total += (self.carried as i64) * per_strand_millicents / 1000;
            self.carried = 0;
        }
        total += self.bag_value;
        self.valuables_bag.clear();
        self.bag_value = 0;
        if total > 0 {
            self.money += total;
            self.earned_total += total;
        }
        total
    }

    pub fn collect_valuable(&mut self, kind: ValuableKind) {
        let v = kind.value();
        self.valuables_bag.push((kind, v));
        self.bag_value += v;
        self.toast(format!("FOUND: {} ({})", kind.name(), format_money(v)));
    }

    pub fn update(&mut self, dt: f32, world: &mut World, cam_pos: Vec3, cam_fwd: Vec3) {
        self.bg_fade = (self.bg_fade + dt * 3.0).min(1.0);
        self.time_accum += dt as f64;
        for t in self.toasts.iter_mut() {
            t.life -= dt;
        }
        self.toasts.retain(|t| t.life > 0.0);
        self.dig_cd -= dt;
        self.shake = (self.shake - dt * 3.0).max(0.0);

        if self.state != GameState::Playing {
            return;
        }
        self.time += dt as f64;

        // automation (baler / vacuum + research multipliers)
        let rate = self.auto_rate();
        if rate > 0.0 && self.removed_count < world.straw_count {
            self.automation_acc += dt * rate;
            let n = self.automation_acc as u32;
            if n > 0 {
                self.automation_acc -= n as f32;
                let mut rng = Rng::new((self.time_accum * 1000.0) as u64 | 1);
                let mut got = 0u32;
                for _ in 0..n.min(600) {
                    if world.pop_random_alive(&mut rng).is_some() {
                        self.removed_count += 1;
                        got += 1;
                    }
                }
                if got > 0 {
                    // 70% goes straight through the belt to the stall
                    let frac = self.sale_fraction();
                    let auto_money = ((got as f32 * frac) as i64)
                        * (STRAND_PRICE_MILLICENTS as f32 * self.strand_price_mul()) as i64
                        / 1000;
                    if auto_money > 0 {
                        self.money += auto_money;
                        self.earned_total += auto_money;
                    }
                }
            }
        }

        // detector
        if let Some(r) = self.detector_radius() {
            let dist = (world.needle.pos - cam_pos).length();
            self.detector_hot = dist < r && !world.needle.found;
            let speed = if self.detector_hot {
                2.2 - 1.6 * (dist / r).clamp(0.0, 1.0)
            } else {
                0.0
            };
            self.detector_pulse += dt * speed;
        }

        // magnet
        if !self.pure_mode && self.owned[6] {
            for v in world.valuables.iter_mut() {
                if !v.taken && (v.pos - cam_pos).length() < 2.5 {
                    v.taken = true;
                    self.collect_valuable(v.kind);
                }
            }
        }

        let _ = (cam_fwd,);
    }

    /// LMB action. Returns (hit_pos, burst_positions).
    pub fn dig(&mut self, world: &mut World, o: Vec3, d: Vec3) -> Option<(Vec3, Vec<Vec3>)> {
        if self.dig_cd > 0.0 || self.state != GameState::Playing {
            return None;
        }

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

        // full bucket?
        if self.carried >= self.capacity() {
            self.toast("BUCKET FULL! SELL YOUR HAY AT THE SELL HAY STALL");
            self.dig_cd = 0.4;
            return None;
        }

        // picking a valuable?
        for v in world.valuables.iter_mut() {
            if !v.taken && Self::aim_at_item(o, d, v.pos, 0.16) {
                v.taken = true;
                self.collect_valuable(v.kind);
                self.dig_cd = 0.15;
                return None;
            }
        }

        // normal dig: apply the tool cooldown
        self.dig_cd = self.cooldown();

        let hit;
        let removed;
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

        // strands go into the bucket (up to capacity)
        let free = self.capacity() - self.carried;
        let got = (removed.len() as u32).min(free);
        self.carried += got;
        self.shake = 0.35;

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

/// Format cents as "$13,976" (dollars, comma-grouped) - or "$13.98" under $100.
pub fn format_money(cents: i64) -> String {
    if cents < 0 {
        return format!("-{}", format_money(-cents));
    }
    if cents < 10000 {
        return format!("${}.{:02}", cents / 100, cents % 100);
    }
    let dollars = cents / 100;
    let s = dollars.to_string();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("${}", out)
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

// ----- full save game (v3: leveled research; reads v2 saves too) -----

const SAVE_MAGIC: &[u8; 4] = b"FND3";
const SAVE_MAGIC_V2: &[u8; 4] = b"FND2";
const SAVE_VERSION: u32 = 3;

pub fn save_game(game: &Game, world: &World, path: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = Vec::new();
    f.extend_from_slice(SAVE_MAGIC);
    f.extend_from_slice(&SAVE_VERSION.to_le_bytes());
    f.extend_from_slice(&game.seed.to_le_bytes());
    f.extend_from_slice(&(world.straw_count as u64).to_le_bytes());
    f.extend_from_slice(&(game.money as u64).to_le_bytes());
    f.extend_from_slice(&(game.carried as u64).to_le_bytes());
    f.extend_from_slice(&(game.capacity as u64).to_le_bytes());
    f.extend_from_slice(&(game.earned_total as u64).to_le_bytes());
    // leveled research: u8 count + one byte per card
    f.push(RESEARCH.len() as u8);
    for &l in &game.research_lv {
        f.push(l);
    }
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
    if data.len() < 60 {
        return None;
    }
    let is_v3 = &data[0..4] == SAVE_MAGIC;
    let is_v2 = &data[0..4] == SAVE_MAGIC_V2;
    if !is_v3 && !is_v2 {
        return None;
    }
    let ver = u32::from_le_bytes(data[4..8].try_into().ok()?);
    if ver != SAVE_VERSION && ver != 2 {
        return None;
    }
    let mut o = 8;
    let mut ru64 = || -> Option<u64> {
        let v = u64::from_le_bytes(data[o..o + 8].try_into().ok()?);
        o += 8;
        Some(v)
    };
    let seed = ru64()?;
    let straw_count = ru64()? as u32;
    let money = ru64()? as i64;
    let carried = ru64()? as u32;
    let capacity = ru64()? as u32;
    let earned = ru64()? as i64;

    // research: v3 = leveled byte array, v2 = u64 bitset (0/1 per card)
    let mut game = Game::new(seed, false);
    if is_v3 {
        let n = data[o] as usize;
        o += 1;
        if data.len() < o + n {
            return None;
        }
        for (i, &l) in data[o..o + n].iter().enumerate() {
            if i < game.research_lv.len() {
                game.research_lv[i] = l.min(RESEARCH[i].levels.min(255) as u8);
            }
        }
        o += n;
    } else {
        let research = ru64()?;
        for i in 0..RESEARCH.len().min(64) {
            if (research >> i) & 1 == 1 {
                game.research_lv[i] = 1;
            }
        }
    }

    let mut owned = [false; 9];
    for i in 0..9 {
        owned[i] = data[o + i] != 0;
    }
    o += 9;
    let selected = u32::from_le_bytes(data[o..o + 4].try_into().ok()?) as usize;
    o += 4;
    let pure = data[o] != 0;
    o += 1;
    let time = f64::from_le_bytes(data[o..o + 8].try_into().ok()?);
    o += 8;
    let removed_count = u32::from_le_bytes(data[o..o + 4].try_into().ok()?);
    o += 4;
    // bitset has a fixed size derived from straw_count; won_time follows it
    let words = ((straw_count as usize) + 63) / 64;
    let blen = words * 8;
    if data.len() < o + blen + 8 {
        return None;
    }
    let bitset = data[o..o + blen].to_vec();
    o += blen;
    let won_time = f64::from_le_bytes(data[o..o + 8].try_into().ok()?);

    game.pure_mode = pure;
    game.money = money;
    game.carried = carried;
    game.capacity = capacity.max(600);
    game.earned_total = earned;
    game.owned = owned;
    game.owned[0] = true;
    game.owned[1] = true;
    game.selected = selected.min(8);
    game.time = time;
    game.won_time = won_time;
    Some((game, straw_count, bitset, removed_count))
}
