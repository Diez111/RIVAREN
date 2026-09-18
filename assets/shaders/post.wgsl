// RIVAREN post.wgsl — bloom, tonemap ACES + TAA, upscale con sharpen.

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

struct PostParams {
    texel: vec2<f32>,
    bloom_strength: f32,
    exposure: f32,
    taa_blend: f32,
    frame: f32,
    underwater: f32,
    sharpen: f32,
    _pad: f32,
};
@group(1) @binding(0) var src_tex: texture_2d<f32>;
@group(1) @binding(1) var bloom_tex: texture_2d<f32>;
@group(1) @binding(2) var depth_tex: texture_depth_2d;
@group(1) @binding(3) var history_tex: texture_2d<f32>;
@group(1) @binding(4) var lin_smp: sampler;
@group(1) @binding(5) var<uniform> p: PostParams;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> VsOut {
    var out: VsOut;
    let x = f32((vi & 1u) * 4u) - 1.0;
    let y = f32((vi >> 1u) * 4u) - 1.0;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5 + 0.5, 0.5 - y * 0.5);
    return out;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// ── Bright pass (detección de luminancia para bloom) ──
@fragment
fn fs_bright(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, lin_smp, in.uv).rgb;
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    let knee = smoothstep(0.7, 1.1, l);
    return vec4<f32>(c * knee, 1.0);
}

// ── Blur separable 9-tap gaussiano ──
@fragment
fn fs_blur(in: VsOut) -> @location(0) vec4<f32> {
    let dir = vec2<f32>(p.texel.x * 1.0, 0.0);
    var sum = vec3<f32>(0.0);
    let w = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    sum += textureSample(src_tex, lin_smp, in.uv).rgb * w[0];
    for (var i = 1u; i < 5u; i = i + 1u) {
        let o = dir * f32(i);
        sum += textureSample(src_tex, lin_smp, in.uv + o).rgb * w[i];
        sum += textureSample(src_tex, lin_smp, in.uv - o).rgb * w[i];
    }
    return vec4<f32>(sum, 1.0);
}

@fragment
fn fs_blur_v(in: VsOut) -> @location(0) vec4<f32> {
    let dir = vec2<f32>(0.0, p.texel.y * 1.6);
    var sum = vec3<f32>(0.0);
    let w = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    sum += textureSample(src_tex, lin_smp, in.uv).rgb * w[0];
    for (var i = 1u; i < 5u; i = i + 1u) {
        let o = dir * f32(i);
        sum += textureSample(src_tex, lin_smp, in.uv + o).rgb * w[i];
        sum += textureSample(src_tex, lin_smp, in.uv - o).rgb * w[i];
    }
    return vec4<f32>(sum, 1.0);
}

// ── Tonemap + TAA → LDR render-res + historial ──
@fragment
fn fs_tonemap(in: VsOut) -> @location(0) vec4<f32> {
    let hdr = textureSample(src_tex, lin_smp, in.uv).rgb;
    let bloom = textureSample(bloom_tex, lin_smp, in.uv).rgb * p.bloom_strength;
    var color = hdr + bloom;
    // TAA: reproyección con profundidad y clamp al vecindario.
    let depth = textureSample(depth_tex, lin_smp, in.uv);
    var mixed = color;
    if (p.taa_blend > 0.0 && depth > 0.0 && depth < 1.0) {
        let ndc = vec3<f32>(in.uv * 2.0 - 1.0, depth);
        let world = g.inv_view_proj * vec4<f32>(ndc, 1.0);
        let wp = world.xyz / world.w;
        let prev = g.prev_view_proj * vec4<f32>(wp, 1.0);
        let prev_uv = prev.xy / prev.w * 0.5 + 0.5;
        if (prev_uv.x > 0.0 && prev_uv.x < 1.0 && prev_uv.y > 0.0 && prev_uv.y < 1.0) {
            var mn = color;
            var mx = color;
            for (var i = 0; i < 9; i = i + 1) {
                let ox = f32((i % 3) - 1) * p.texel.x;
                let oy = f32((i / 3) - 1) * p.texel.y;
                let s = textureSample(src_tex, lin_smp, in.uv + vec2<f32>(ox, oy)).rgb;
                mn = min(mn, s);
                mx = max(mx, s);
            }
            var hist = textureSample(history_tex, lin_smp, prev_uv).rgb;
            hist = clamp(hist, mn, mx);
            mixed = mix(color, hist, p.taa_blend);
        }
    }
    var ldr = aces(mixed * p.exposure);
    if (p.underwater > 0.5) {
        ldr = mix(ldr, vec3<f32>(0.08, 0.22, 0.40), 0.35);
    }
    return vec4<f32>(ldr, 1.0);
}

// ── Upscale + sharpen (CAS-lite) + lluvia → surface ──
@fragment
fn fs_upscale(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, lin_smp, in.uv).rgb;
    var col = c;
    if (p.sharpen > 0.0) {
        let n = textureSample(src_tex, lin_smp, in.uv + vec2<f32>(0.0, -p.texel.y)).rgb;
        let s = textureSample(src_tex, lin_smp, in.uv + vec2<f32>(0.0, p.texel.y)).rgb;
        let e = textureSample(src_tex, lin_smp, in.uv + vec2<f32>(p.texel.x, 0.0)).rgb;
        let w = textureSample(src_tex, lin_smp, in.uv + vec2<f32>(-p.texel.x, 0.0)).rgb;
        col = col + (c * 4.0 - n - s - e - w) * p.sharpen * 0.25;
    }
    // Lluvia: estelas procedurales cuando el clima supera 0.3.
    let weather = g.misc.w;
    if (weather > 0.3) {
        let t = g.cam_pos.w * 0.016;
        let grid = in.uv * vec2<f32>(90.0, 50.0);
        let cell = floor(grid);
        let f = fract(grid);
        let rnd = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        let speed = 1.5 + rnd * 1.5;
        let ry = fract(f.y + t * speed);
        let streak = smoothstep(0.82, 1.0, ry) * smoothstep(0.06, 0.0, abs(f.x - 0.5));
        let amount = clamp((weather - 0.3) * 1.4, 0.0, 0.75) * streak;
        col = mix(col, vec3<f32>(0.72, 0.78, 0.92), amount);
    }
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
