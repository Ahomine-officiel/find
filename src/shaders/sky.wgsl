// Sky: gradient + sun disc + cheap procedural clouds. Fullscreen triangle.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    cam_fwd: vec4<f32>,   // w = tan(fov/2)
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    sun_dir: vec4<f32>,
    fog_color: vec4<f32>,
    sky_low: vec4<f32>,
    sky_high: vec4<f32>,
    params: vec4<f32>, // time, fog_near, fog_far, cloud_amt
};

@group(0) @binding(0) var<uniform> g: Globals;

struct SOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) suv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> SOut {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: SOut;
    out.clip = vec4<f32>(p[vi], 0.99999, 1.0);
    out.suv = p[vi];
    return out;
}

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

fn fbm(p: vec2<f32>) -> f32 {
    return vnoise(p) * 0.62 + vnoise(p * 2.6 + vec2<f32>(13.7, 7.1)) * 0.38;
}

@fragment
fn fs(in: SOut) -> @location(0) vec4<f32> {
    let tanh_fov = g.cam_fwd.w;
    let dir = normalize(
        g.cam_fwd.xyz + g.cam_right.xyz * in.suv.x * tanh_fov + g.cam_up.xyz * in.suv.y * tanh_fov
    );

    let t = clamp(dir.y, 0.0, 1.0);
    var col = mix(g.sky_low.rgb, g.sky_high.rgb, pow(t, 0.55));
    if (dir.y < 0.0) {
        col = mix(g.sky_low.rgb, g.sky_low.rgb * 0.82, clamp(-dir.y * 4.0, 0.0, 1.0));
    }

    // sun
    let sd = max(dot(dir, g.sun_dir.xyz), 0.0);
    col += vec3<f32>(1.0, 0.95, 0.82) * (pow(sd, 1200.0) * 6.0 + pow(sd, 6.0) * 0.10);

    // clouds projected on a plane at ~90m
    if (dir.y > 0.012 && g.params.w > 0.01) {
        let cp = g.cam_pos.xz + dir.xz * (90.0 / dir.y);
        let n = fbm(cp * 0.0045 + vec2<f32>(g.params.x * 0.008, g.params.x * 0.003));
        let cov = smoothstep(0.52, 0.80, n) * g.params.w * smoothstep(0.012, 0.14, dir.y);
        let cloud_col = mix(vec3<f32>(0.82, 0.85, 0.90), vec3<f32>(1.0), smoothstep(0.52, 0.9, n));
        col = mix(col, cloud_col, cov * 0.92);
    }

    return vec4<f32>(col, 1.0);
}
