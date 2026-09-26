// Main scene shader: props (mat4 instances) + straws (packed instances).
// Lighting: sun wrap-diffuse + sky ambient, distance fog. Zero textures.

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
    @location(2) col: vec3<f32>,
    @location(3) flag: f32,
};

// ---------------- prop pipeline (mesh + mat4 instance) ----------------

struct PropIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec3<f32>,
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
    out.col = in.col;
    out.flag = in.icol.a;
    return out;
}

const SUN_COL: vec3<f32> = vec3(1.04, 0.98, 0.88);

@fragment
fn fs_prop(in: VOut) -> @location(0) vec4<f32> {
    var base = in.col;
    let t = g.params.x;

    // ground mode: light-gray concrete + faint seams (matches the real game's floor)
    if (in.flag == 0.998) {
        let gp = in.wpos.xz / 4.0;
        let fwx = max(dpdx(gp.x), 1e-4);
        let fwy = max(dpdx(gp.y), 1e-4);
        let dx = abs(fract(gp.x - 0.5) - 0.5) / fwx;
        let dy = abs(fract(gp.y - 0.5) - 0.5) / fwy;
        let line = 1.0 - min(min(dx, dy), 1.0);
        let checker = fract(floor(gp.x) * 0.5 + floor(gp.y) * 0.5) * 2.0;
        base = base * (0.975 + 0.05 * checker);
        base = mix(base, base * 0.88, line * 0.5);
        // micro speckle noise so the concrete does not look flat
        let n = fract(sin(dot(floor(in.wpos.xz * 6.0), vec2<f32>(12.9898, 78.233))) * 43758.5453);
        base = base * (0.96 + 0.08 * n);
        // fade seams far away (anti-moire)
        let dist2 = distance(g.cam_pos.xyz, in.wpos);
        base = mix(base, in.col, smoothstep(120.0, 380.0, dist2));
    }

    var n = normalize(in.nrm);
    if (n.x == 0.0 && n.y == 0.0 && n.z == 0.0) {
        n = vec3(0.0, 1.0, 0.0);
    }
    let ndl = clamp(dot(n, g.sun_dir.xyz) * 0.5 + 0.5, 0.0, 1.0);
    let sky_amb = mix(g.sky_low.rgb, g.sky_high.rgb, n.y * 0.5 + 0.5);
    var lit = base * (ndl * SUN_COL + sky_amb * 0.38);

    let viewdir = normalize(g.cam_pos.xyz - in.wpos);
    let dist = length(g.cam_pos.xyz - in.wpos);

    // needle: strong sparkle + specular glint (flag 0.5) - this is the game's core tell
    if (in.flag == 0.5) {
        let refl = reflect(-g.sun_dir.xyz, n);
        lit += vec3(1.0) * pow(max(dot(refl, viewdir), 0.0), 40.0) * 1.1;
        let pulse = 0.5 + 0.5 * sin(t * 7.0 + in.wpos.x * 3.0 + in.wpos.z * 2.0);
        lit += vec3(1.0, 1.0, 0.95) * pulse * clamp(1.6 - dist / 9.0, 0.0, 1.0) * 0.95;
    }

    // valuables: subtle glint so they catch the eye (flag 0.55)
    if (in.flag == 0.55) {
        let refl = reflect(-g.sun_dir.xyz, n);
        lit += vec3(1.0) * pow(max(dot(refl, viewdir), 0.0), 60.0) * 0.55;
        let pulse = 0.5 + 0.5 * sin(t * 5.0 + in.wpos.x * 4.0);
        lit += vec3(1.0, 0.95, 0.8) * pulse * clamp(1.0 - dist / 4.0, 0.0, 1.0) * 0.35;
    }

    // machines: working pulse (flag 0.6)
    if (in.flag == 0.6) {
        lit *= 1.0 + 0.22 * sin(t * 5.0);
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
    @location(4) idata: vec4<u32>,  // packed, cvar, state, kind
};

@vertex
fn vs_straw(in: StrawIn) -> VOut {
    if (in.idata.z == 1u) {
        var dead: VOut;
        dead.clip = vec4<f32>(0.0, 0.0, -2.0, 1.0);
        dead.wpos = vec3<f32>(0.0);
        dead.nrm = vec3<f32>(0.0, 1.0, 0.0);
        dead.col = vec3<f32>(0.0);
        dead.flag = 0.0;
        return dead;
    }
    let packed = in.idata.x;
    let yaw = f32(packed & 4095u) / 4096.0 * 6.2831853;
    let pn = f32((packed >> 12u) & 1023u) / 1024.0;
    let pitch = (pn - 0.5) * 3.14159265;
    let ln = f32((packed >> 22u) & 63u) / 63.0;
    let len = 0.12 + ln * 0.33;

    let dir = vec3<f32>(cos(pitch) * sin(yaw), sin(pitch), cos(pitch) * cos(yaw));
    var side = cross(dir, vec3<f32>(0.0, 1.0, 0.0));
    if (abs(dir.y) > 0.98) {
        side = vec3<f32>(1.0, 0.0, 0.0);
    } else {
        side = normalize(side);
    }
    let bt = normalize(cross(side, dir));

    // chunky flat hay pieces (~4 cm wide) like the real game
    let wpos = in.ipos.xyz
        + in.pos.x * 0.026 * side
        + in.pos.y * len * dir
        + in.pos.z * 0.014 * len * bt;

    var n = normalize(side * (in.pos.x * 0.7) + bt * (in.pos.z * 0.9) + dir * 0.30);

    let tint = f32(in.idata.y & 255u) / 255.0;
    // real-game palette: dark brown hay with warm amber highlights
    let dark = vec3<f32>(0.150, 0.095, 0.045);
    let amber = vec3<f32>(0.440, 0.290, 0.130);
    let gold = mix(dark, amber, tint * tint);
    var base = gold * in.ipos.w;
    if (in.col.r > 0.5) {
        // sun-kissed tip
        base = mix(base, vec3<f32>(0.72, 0.52, 0.26), 0.65) * 1.12;
    }
    if (in.idata.w == 2u) {
        base = vec3<f32>(0.78, 0.58, 0.28) * 1.05; // flying straws: bright golden
    }

    var out: VOut;
    out.clip = g.view_proj * vec4<f32>(wpos, 1.0);
    out.wpos = wpos;
    out.nrm = n;
    out.col = base;
    out.flag = 0.0;
    return out;
}

@fragment
fn fs_straw(in: VOut) -> @location(0) vec4<f32> {
    let n = normalize(in.nrm);
    let ndl = clamp(dot(n, g.sun_dir.xyz) * 0.5 + 0.5, 0.0, 1.0);
    let sky_amb = mix(g.sky_low.rgb, g.sky_high.rgb, n.y * 0.5 + 0.5);
    let lit = in.col * (ndl * SUN_COL + sky_amb * 0.42);
    let dist = length(g.cam_pos.xyz - in.wpos);
    let f = smoothstep(g.params.y, g.params.z, dist);
    return vec4<f32>(mix(lit, g.fog_color.rgb, f), 1.0);
}
