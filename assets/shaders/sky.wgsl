// RIVAREN sky.wgsl — atmósfera analítica + sol/luna + estrellas + nubes 2D.

struct Globals {
    view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sky_color: vec4<f32>,
    horizon_color: vec4<f32>,
    cascade_mats: array<mat4x4<f32>, 3>,
    cascade_splits: vec4<f32>,
    misc: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    // Triángulo fullscreen.
    let x = f32((vi & 1u) * 4u) - 1.0;
    let y = f32((vi >> 1u) * 4u) - 1.0;
    out.pos = vec4<f32>(x, y, 1.0, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}

fn hash12(p: vec2<f32>) -> f32 {
    var h = fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
    h = fract(h * 43758.5453);
    return h;
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>, oct: u32) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0u; i < oct; i = i + 1u) {
        s += a * value_noise(q);
        q = q * 2.03 + vec2<f32>(11.3, 7.7);
        a *= 0.5;
    }
    return s;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var dir = (g.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0)).xyz;
    dir = normalize(dir);
    let up = clamp(dir.y, 0.0, 1.0);
    let day = clamp(g.sun_dir.w * 1.5, 0.0, 1.0);
    // Gradiente cielo.
    var color = mix(g.horizon_color.rgb, g.sky_color.rgb, pow(up, 0.6));
    // Sol/luna.
    let sun_dot = dot(dir, g.sun_dir.xyz);
    let sun_disc = smoothstep(0.9990, 0.9997, sun_dot);
    let sun_glow = pow(max(sun_dot, 0.0), 8.0) * 0.35;
    color += vec3<f32>(1.0, 0.92, 0.75) * sun_disc * 3.0 * day;
    color += vec3<f32>(1.0, 0.85, 0.6) * sun_glow * day;
    let moon_dir = -g.sun_dir.xyz;
    let moon_dot = dot(dir, moon_dir);
    color += vec3<f32>(0.75, 0.80, 0.95) * smoothstep(0.9992, 0.9998, moon_dot) * (1.0 - day);
    // Estrellas de noche.
    if (day < 0.35 && dir.y > 0.0) {
        let st = hash12(floor(dir.xz / max(dir.y, 0.05) * 180.0));
        let star = step(0.9975, st) * (1.0 - day) * smoothstep(0.0, 0.3, dir.y);
        color += vec3<f32>(star);
    }
    // Nubes 2D a altura fija.
    if (dir.y > 0.02) {
        let t = g.cam_pos.w * 0.008;
        let p = dir.xz / dir.y * 0.35 + vec2<f32>(t, t * 0.6);
        let cl = fbm(p * 0.5, 4);
        let cover = smoothstep(0.52, 0.72, cl) * clamp(dir.y * 3.0, 0.0, 1.0);
        let lit = mix(0.65, 1.0, clamp(g.sun_dir.w, 0.0, 1.0));
        let cloud_col = vec3<f32>(0.92, 0.94, 0.98) * lit;
        color = mix(color, cloud_col, cover * 0.75);
    }
    return vec4<f32>(color, 1.0);
}
