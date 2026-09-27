// Main scene shader: props (mat4 instances) + straws (packed instances).
// Material flags ride in vertex color alpha (and instance color alpha for
// dynamic props). Lighting: sun wrap-diffuse + sky ambient, distance fog.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    cam_fwd: vec4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    sun_dir: vec4<f32>,
    fog_color: vec4<f32>,
    sky_low: vec4<f32>,
    sky_high: vec4<f32>,
    params: vec4<f32>, // time, fog_near, fog_far, cloud_amt
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) wpos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec4<f32>,
};

// ---------------- prop pipeline (mesh + mat4 instance) ----------------

struct PropIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec4<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
    @location(7) icol: vec4<f32>,
};

@vertex
fn vs_prop(in: PropIn) -> VOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    let wpos = model * vec4<f32>(in.pos, 1.0);
    var n = model * vec4<f32>(in.nrm, 0.0);
    n = normalize(n);
    var out: VOut;
    out.clip = g.view_proj * wpos;
    out.wpos = wpos.xyz;
    out.nrm = n.xyz;
    out.col = vec4<f32>(in.col.rgb * in.icol.rgb, in.col.a + in.icol.a);
    return out;
}

const SUN_COL: vec3<f32> = vec3(1.06, 1.00, 0.90);

fn hash21(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(123.34, 456.21));
    let q2 = q + dot(q, q + vec2<f32>(45.32, 45.32));
    return fract(q2.x * q2.y);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@fragment
fn fs_prop(in: VOut) -> @location(0) vec4<f32> {
    var base = in.col.rgb;
    // interpolated constant-per-primitive flag: compare with tolerance
    let flag = in.col.a;
    let is_grid = abs(flag - 0.998) < 0.002;
    let is_corr = abs(flag - 0.997) < 0.002;
    let is_sand = abs(flag - 0.996) < 0.002;
    let is_plank = abs(flag - 0.995) < 0.002;
    let is_grass = abs(flag - 0.994) < 0.002;
    let is_belt = abs(flag - 0.993) < 0.002;
    let is_needle = abs(flag - 0.5) < 0.002;
    let is_valuable = abs(flag - 0.55) < 0.002;
    let is_machine = abs(flag - 0.6) < 0.002;
    let is_led_r = abs(flag - 0.90) < 0.002;
    let is_led_g = abs(flag - 0.91) < 0.002;
    let t = g.params.x;

    // checker grid floor (test mode)
    if (is_grid) {
        let gp = in.wpos.xz / 4.0;
        let fwx = max(dpdx(gp.x), 1e-4);
        let fwy = max(dpdx(gp.y), 1e-4);
        let dx = abs(fract(gp.x - 0.5) - 0.5) / fwx;
        let dy = abs(fract(gp.y - 0.5) - 0.5) / fwy;
        let line = 1.0 - min(min(dx, dy), 1.0);
        let checker = fract(floor(gp.x) * 0.5 + floor(gp.y) * 0.5) * 2.0;
        base = base * (0.965 + 0.05 * checker);
        base = mix(base, base * 0.82, line * 0.85);
        let dist2 = distance(g.cam_pos.xyz, in.wpos);
        base = mix(base, in.col.rgb, smoothstep(120.0, 380.0, dist2));
    }

    // corrugated metal: vertical ribs + panel variation
    if (is_corr) {
        // rib direction: along world X unless the surface normal is mostly +/-X (then along Z)
        let nrm = normalize(in.nrm);
        let use_z = abs(nrm.x) > 0.7;
        let u = select(in.wpos.x, in.wpos.z, use_z);
        let rib = sin(u * 9.2);
        base = base * (1.0 + rib * 0.13);
        // panel tint variation
        let pnl = vnoise(floor(in.wpos.xz * 0.15));
        base = base * (0.90 + pnl * 0.20);
        // rust streaks (cheap noise stretched vertically)
        let rust = vnoise(vec2<f32>(in.wpos.x * 0.6, in.wpos.y * 0.05));
        base = mix(base, base * vec3<f32>(0.75, 0.60, 0.45), smoothstep(0.62, 0.85, rust) * 0.5);
    }

    // sandy barn floor: pebbles + mottling (like shot 5)
    if (is_sand) {
        let p1 = in.wpos.xz * 2.6;
        let peb = vnoise(p1) * 0.5 + vnoise(p1 * 3.1) * 0.3 + vnoise(p1 * 9.0) * 0.2;
        base = base * (0.86 + peb * 0.30);
        let pebble = smoothstep(0.78, 0.90, vnoise(in.wpos.xz * 7.0));
        base = mix(base, base * vec3<f32>(0.72, 0.66, 0.58), pebble * 0.7);
        let mtl = vnoise(in.wpos.xz * 0.18);
        base = base * (0.92 + mtl * 0.14);
        // darker where the pile shade falls (fake AO near pile base)
        let dr = length(in.wpos.xz) / 16.0;
        base = base * mix(0.72, 1.0, smoothstep(0.9, 1.25, dr));
    }

    // wood planks: horizontal boards
    if (is_plank) {
        let row = floor(in.wpos.y * 3.4);
        let grout = fract(in.wpos.y * 3.4);
        let line = 1.0 - smoothstep(0.06, 0.14, min(grout, 1.0 - grout) * 12.0);
        let off = hash21(vec2<f32>(row, floor(in.wpos.x * 0.2 + in.wpos.z * 0.13)));
        base = base * (0.85 + off * 0.30);
        base = mix(base, base * 0.55, line * 0.8);
        let grain = vnoise(vec2<f32>(in.wpos.y * 40.0, in.wpos.x * 1.5 + in.wpos.z));
        base = base * (0.93 + grain * 0.13);
    }

    // grass: mottled green
    if (is_grass) {
        let n1 = vnoise(in.wpos.xz * 0.5);
        let n2 = vnoise(in.wpos.xz * 3.0);
        base = base * (0.85 + n1 * 0.28) * (0.92 + n2 * 0.15);
        base = mix(base, vec3<f32>(0.55, 0.50, 0.28), smoothstep(0.72, 0.9, n1) * 0.35);
    }

    // conveyor belt: dark rubber with moving cleats
    if (is_belt) {
        let u = dot(in.wpos.xz, normalize(vec2<f32>(0.75, 0.42)));
        let cleat = fract(u * 1.4 - t * 0.55);
        let c = smoothstep(0.06, 0.12, cleat) * smoothstep(0.30, 0.24, cleat);
        base = base * (1.0 + c * 0.35);
        base = mix(base, vec3<f32>(0.13, 0.13, 0.14), 0.35);
    }

    var n = normalize(in.nrm);
    if (n.x == 0.0 && n.y == 0.0 && n.z == 0.0) {
        n = vec3(0.0, 1.0, 0.0);
    }
    let ndl = clamp(dot(n, g.sun_dir.xyz) * 0.5 + 0.5, 0.0, 1.0);
    let sky_amb = mix(g.sky_low.rgb, g.sky_high.rgb, n.y * 0.5 + 0.5);
    var lit = base * (ndl * SUN_COL + sky_amb * 0.42);

    let viewdir = normalize(g.cam_pos.xyz - in.wpos);
    let dist = length(g.cam_pos.xyz - in.wpos);

    // needle: sparkle + specular glint (flag 0.5)
    if (is_needle) {
        let refl = reflect(-g.sun_dir.xyz, n);
        lit += vec3(1.0) * pow(max(dot(refl, viewdir), 0.0), 40.0) * 0.9;
        let pulse = 0.5 + 0.5 * sin(t * 7.0 + in.wpos.x * 3.0 + in.wpos.z * 2.0);
        lit += vec3(1.0, 1.0, 0.95) * pulse * clamp(1.4 - dist / 7.0, 0.0, 1.0) * 0.9;
    }

    // valuables: subtle twinkle so they pop out of the hay (flag 0.55)
    if (is_valuable) {
        let refl = reflect(-g.sun_dir.xyz, n);
        lit += vec3(1.0, 0.95, 0.75) * pow(max(dot(refl, viewdir), 0.0), 24.0) * 0.7;
    }

    // machines: working pulse (flag 0.6)
    if (is_machine) {
        lit *= 1.0 + 0.22 * sin(t * 5.0);
    }

    // emissive red LED (clock digits, flag 0.90)
    if (is_led_r) {
        let pulse = 0.85 + 0.15 * sin(t * 2.0);
        lit = base * vec3<f32>(2.4, 0.35, 0.22) * pulse;
    }
    // emissive green LED (flag 0.91)
    if (is_led_g) {
        lit = base * vec3<f32>(0.5, 2.2, 0.6);
    }

    let f = smoothstep(g.params.y, g.params.z, dist);
    let col = mix(lit, g.fog_color.rgb, f);
    return vec4<f32>(col, 1.0);
}

// ---------------- straw pipeline (packed instances) ----------------

struct StrawIn {
    @location(0) pos: vec3<f32>,
    @location(2) col: vec3<f32>,
    @location(3) ipos: vec4<f32>,   // xyz + shade
    @location(4) idata: vec4<u32>,  // packed, tint, state, kind
};

@vertex
fn vs_straw(in: StrawIn) -> VOut {
    if (in.idata.z == 1u) {
        var dead: VOut;
        dead.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
        dead.wpos = vec3<f32>(0.0);
        dead.nrm = vec3<f32>(0.0, 1.0, 0.0);
        dead.col = vec4<f32>(0.0);
        return dead;
    }
    let packed = in.idata.x;
    let yaw = f32(packed & 4095u) / 4096.0 * 6.2831853;
    let pn = f32((packed >> 12u) & 1023u) / 1024.0;
    let pitch = (pn - 0.5) * 3.14159265;
    let ln = f32((packed >> 22u) & 63u) / 63.0;
    let len = 0.28 + ln * 0.32;

    let dir = vec3<f32>(cos(pitch) * sin(yaw), sin(pitch), cos(pitch) * cos(yaw));
    var side = cross(dir, vec3<f32>(0.0, 1.0, 0.0));
    if (abs(dir.y) > 0.98) {
        side = vec3<f32>(1.0, 0.0, 0.0);
    } else {
        side = normalize(side);
    }
    let bt = normalize(cross(side, dir));

    let wpos = in.ipos.xyz
        + in.pos.x * 0.012 * side
        + in.pos.y * len * dir
        + in.pos.z * len * bt;

    var n = normalize(side * (in.pos.x * 0.6) + bt * (in.pos.z * 0.8) + dir * 0.25);

    let tint = f32(in.idata.y & 255u) / 255.0;
    // BRIGHT golden straw palette (reference: pale cream -> gold -> rare dark)
    var gold = mix(vec3<f32>(0.96, 0.89, 0.66), vec3<f32>(0.88, 0.72, 0.38), smoothstep(0.0, 1.0, tint));
    let dark_pick = step(0.88, fract(tint * 7.13));
    gold = mix(gold, vec3<f32>(0.52, 0.38, 0.18), dark_pick * 0.85);
    var base = gold * in.ipos.w;
    if (in.col.r > 0.5) {
        base = base * 1.18; // sun-bleached tips
    }
    if (in.idata.w == 2u) {
        base = gold * 1.12; // flying straws
    }

    var out: VOut;
    out.clip = g.view_proj * vec4<f32>(wpos, 1.0);
    out.wpos = wpos;
    out.nrm = n;
    out.col = vec4<f32>(base, 0.0);
    return out;
}

@fragment
fn fs_straw(in: VOut) -> @location(0) vec4<f32> {
    let n = normalize(in.nrm);
    let ndl = clamp(dot(n, g.sun_dir.xyz) * 0.5 + 0.5, 0.0, 1.0);
    let sky_amb = mix(g.sky_low.rgb, g.sky_high.rgb, n.y * 0.5 + 0.5);
    var lit = in.col.rgb * (ndl * SUN_COL + sky_amb * 0.46);
    // subtle golden sheen toward the sun (fake translucency on the tips)
    let viewdir = normalize(g.cam_pos.xyz - in.wpos);
    lit += in.col.rgb * pow(max(dot(viewdir, -g.sun_dir.xyz), 0.0), 3.0) * 0.10;
    let dist = length(g.cam_pos.xyz - in.wpos);
    let f = smoothstep(g.params.y, g.params.z, dist);
    return vec4<f32>(mix(lit, g.fog_color.rgb, f), 1.0);
}
