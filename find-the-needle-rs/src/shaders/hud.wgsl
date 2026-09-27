// HUD: bitmap font + solid quads + ring/hexagon SDF shapes. Ortho, pixel space.

struct HudP {
    params: vec4<f32>, // width_px, height_px, ui_scale, time
};

@group(0) @binding(0) var<uniform> hp: HudP;
@group(0) @binding(1) var font_tex: texture_2d<f32>;
@group(0) @binding(2) var font_samp: sampler;

struct HIn {
    @location(0) a: vec4<f32>, // x, y, w, h (pixels)
    @location(1) b: vec4<f32>, // mode params / uv rect
    @location(2) c: vec4<f32>, // color, a: 0.999 ring, 0.998 hex
};

struct HOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) auv: vec2<f32>,  // atlas uv
    @location(1) lpos: vec2<f32>, // 0..1 local
    @location(2) qa: vec4<f32>,
    @location(3) qb: vec4<f32>,
    @location(4) qc: vec4<f32>,
};

@vertex
fn vs(in: HIn, @builtin(vertex_index) vi: u32) -> HOut {
    // 6 verts, two triangles covering the quad exactly:
    // (0,0),(1,0),(0,1) + (1,0),(1,1),(0,1)
    var corners = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u), vec2<u32>(1u, 0u), vec2<u32>(0u, 1u),
        vec2<u32>(1u, 0u), vec2<u32>(1u, 1u), vec2<u32>(0u, 1u),
    );
    let local = vec2<f32>(corners[vi]);
    let px = in.a.xy + in.a.zw * local;
    let ndc = vec2<f32>(
        px.x / hp.params.x * 2.0 - 1.0,
        1.0 - px.y / hp.params.y * 2.0,
    );
    var out: HOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.auv = in.b.xy + local * in.b.zw;
    out.lpos = local;
    out.qa = in.a;
    out.qb = in.b;
    out.qc = in.c;
    return out;
}

@fragment
fn fs(in: HOut) -> @location(0) vec4<f32> {
    let c = in.qc;

    // ring mode
    if (c.a == 0.999) {
        let r = in.qb.x;
        let th = in.qb.y;
        let d = length((in.lpos - vec2<f32>(0.5)) * in.qa.zw);
        let band = 1.0 - smoothstep(th - 1.5, th + 0.5, abs(d - r));
        return vec4<f32>(c.rgb, band);
    }

    // hexagon outline mode
    if (c.a == 0.998) {
        let r = in.qb.x;
        let border = r * in.qb.y;
        let p = (in.lpos - vec2<f32>(0.5)) * in.qa.zw;
        let hd = max(abs(p.x) * 0.866025 + abs(p.y) * 0.5, abs(p.y));
        let outer = 1.0 - smoothstep(r - 0.75, r + 0.75, hd);
        let inner = 1.0 - smoothstep(r - border - 0.75, r - border + 0.75, hd);
        let alpha = outer - inner;
        return vec4<f32>(c.rgb, clamp(alpha, 0.0, 1.0));
    }

    // solid rect (uv all zero) vs glyph
    if (in.qb.x == 0.0 && in.qb.y == 0.0 && in.qb.z == 0.0 && in.qb.w == 0.0) {
        return vec4<f32>(c.rgb, c.a);
    }
    let tex = textureSampleLevel(font_tex, font_samp, in.auv, 0.0).r;
    return vec4<f32>(c.rgb, tex * c.a);
}
