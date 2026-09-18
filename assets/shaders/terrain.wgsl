// RIVAREN terrain.wgsl — raster de chunks con PackedVertex (8B) + luz + sombras CSM.
// data.x = uv_tex (u:8 v:8 tex:12), data.y = pos_packed:16 | ao:4 sky:4 | block:4 pad:4

struct Globals {
    view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,       // xyz, time
    sun_dir: vec4<f32>,       // xyz, intensity
    sky_color: vec4<f32>,     // rgb, fog_density
    horizon_color: vec4<f32>,
    cascade_mats: array<mat4x4<f32>, 3>,
    cascade_splits: vec4<f32>, // xyz splits, w count
    misc: vec4<f32>,           // day_time, exposure, underwater, weather
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_tex: texture_depth_2d_array;
@group(0) @binding(2) var shadow_smp: sampler_comparison;

struct ChunkInfo {
    offset: vec4<f32>, // xyz world offset, w = lod
};
@group(1) @binding(0) var<uniform> chunk: ChunkInfo;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) shade: f32,
    @location(2) world_pos: vec3<f32>,
    @location(3) normal: u32,
    @location(4) light: vec2<f32>, // sky, block (0..1)
    @location(5) view_depth: f32,
};

fn block_color(tex: u32) -> vec3<f32> {
    switch tex {
        case 1u: { return vec3<f32>(0.32, 0.58, 0.26); } // hierba ferral
        case 2u: { return vec3<f32>(0.44, 0.31, 0.20); } // tierra
        case 4u: { return vec3<f32>(0.54, 0.56, 0.60); } // piedra cristal
        case 6u: { return vec3<f32>(0.84, 0.74, 0.50); } // arena canto
        case 20u: { return vec3<f32>(0.16, 0.40, 0.78); } // agua viva
        case 21u: { return vec3<f32>(0.68, 0.66, 0.74); } // losa plaza
        case 30u: { return vec3<f32>(0.72, 0.40, 0.92); } // marco portal
        default: { return vec3<f32>(0.78, 0.32, 0.80); }
    }
}

fn normal_vector(n: u32) -> vec3<f32> {
    switch n {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 3u: { return vec3<f32>(0.0, -1.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

@vertex
fn vs_main(@location(0) data: vec2<u32>) -> VsOut {
    var out: VsOut;
    let uv_tex_packed = data.x;
    let rest = data.y;
    let pos_packed = rest & 65535u;
    let ao_light = (rest >> 16u) & 255u;
    let px = f32(pos_packed & 31u);
    let py = f32((pos_packed >> 5u) & 31u);
    let pz = f32((pos_packed >> 10u) & 31u);
    let normal = (pos_packed >> 15u) & 7u;
    let tex = (uv_tex_packed >> 16u) & 4095u;
    let ao = f32(ao_light & 15u) / 15.0;
    let sky = f32((ao_light >> 4u) & 15u) / 15.0;
    let block = f32((ao_light >> 8u) & 15u) / 15.0;
    let world = vec3<f32>(px, py, pz) + chunk.offset.xyz;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    out.color = block_color(tex);
    out.world_pos = world;
    out.normal = normal;
    out.light = vec2<f32>(sky, block);
    out.shade = 0.45 + 0.55 * ao;
    out.view_depth = out.pos.w;
    return out;
}

fn sample_shadow(world: vec3<f32>, n: vec3<f32>) -> f32 {
    let count = u32(g.cascade_splits.w);
    var cascade = 0u;
    // view depth aproximado por distancia a cámara
    let dist = distance(world, g.cam_pos.xyz);
    if (count > 1u && dist > g.cascade_splits.x) { cascade = 1u; }
    if (count > 2u && dist > g.cascade_splits.y) { cascade = 2u; }
    let light_clip = g.cascade_mats[cascade] * vec4<f32>(world, 1.0);
    let ndc = light_clip.xyz / light_clip.w;
    if (ndc.z < 0.0 || ndc.z > 1.0 || abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0) {
        return 1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let bias = max(0.0015, 0.004 * (1.0 - dot(n, g.sun_dir.xyz)));
    var sum = 0.0;
    let texel = 1.0 / 2048.0;
    for (var dx = -1; dx <= 1; dx++) {
        for (var dy = -1; dy <= 1; dy++) {
            let o = vec2<f32>(f32(dx), f32(dy)) * texel;
            sum += textureSampleCompare(shadow_tex, shadow_smp, uv + o, i32(cascade), ndc.z + bias);
        }
    }
    return sum / 9.0;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normal_vector(in.normal);
    // Sol + sombra.
    let ndl = max(dot(n, g.sun_dir.xyz), 0.0);
    let shadow = sample_shadow(in.world_pos, n);
    let sun = g.sun_dir.w * ndl * shadow;
    // Sombras de bloque (si block light llega con normal opuesta).
    let block_light = in.light.y;
    let sky_light = in.light.x;
    let ambient = 0.10 + 0.28 * sky_light;
    let light = ambient + sun * 0.85 + block_light * 0.35 * vec3<f32>(0.6, 0.5, 0.9);
    var color = in.color * in.shade * light;
    // Niebla exponencial suave (el mundo mantiene su color a media distancia).
    let fog = 1.0 - exp(-g.sky_color.w * length(in.world_pos - g.cam_pos.xyz) * 0.0009);
    color = mix(color, g.horizon_color.rgb, clamp(fog, 0.0, 0.65));
    // Underwater tint.
    if (g.misc.z > 0.5) {
        color = mix(color, vec3<f32>(0.10, 0.25, 0.45), 0.55);
    }
    return vec4<f32>(color, 1.0);
}
