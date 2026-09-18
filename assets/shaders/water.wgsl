// RIVAREN water.wgsl — agua animada con fresnel + especular; clip de no-agua.

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
@group(0) @binding(1) var shadow_tex: texture_depth_2d_array;
@group(0) @binding(2) var shadow_smp: sampler_comparison;

struct ChunkInfo { offset: vec4<f32> };
@group(1) @binding(0) var<uniform> chunk: ChunkInfo;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) uv_tex: u32,
    @location(2) view_depth: f32,
};

@vertex
fn vs_main(@location(0) data: vec2<u32>) -> VsOut {
    var out: VsOut;
    let rest = data.y;
    let pos_packed = rest & 65535u;
    let px = f32(pos_packed & 31u);
    let py = f32((pos_packed >> 5u) & 31u);
    let pz = f32((pos_packed >> 10u) & 31u);
    var world = vec3<f32>(px, py, pz) + chunk.offset.xyz;
    let tex = (data.x >> 16u) & 4095u;
    out.uv_tex = tex;
    // Olas: solo la cara superior se desplaza.
    if (tex == 20u) {
        let t = g.cam_pos.w;
        world.y += sin(world.x * 0.45 + t * 1.7) * 0.055
                 + sin(world.z * 0.37 + t * 2.1) * 0.055;
    }
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    out.world_pos = world;
    out.view_depth = out.pos.w;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (in.uv_tex != 20u) {
        discard;
    }
    let view_dir = normalize(g.cam_pos.xyz - in.world_pos);
    let n = normalize(vec3<f32>(
        sin(in.world_pos.x * 0.5 + g.cam_pos.w * 1.3) * 0.25,
        1.0,
        sin(in.world_pos.z * 0.5 + g.cam_pos.w * 1.1) * 0.25
    ));
    let fresnel = pow(1.0 - max(dot(view_dir, n), 0.0), 3.0);
    let spec = pow(max(dot(reflect(-g.sun_dir.xyz, n), -view_dir), 0.0), 64.0);
    let deep = vec3<f32>(0.05, 0.18, 0.38);
    var color = mix(deep, g.sky_color.rgb, clamp(fresnel * 0.9 + 0.25, 0.0, 0.95));
    color += spec * g.sun_dir.w * 0.8;
    let alpha = mix(0.62, 0.92, fresnel);
    let fog = 1.0 - exp(-g.sky_color.w * length(in.world_pos - g.cam_pos.xyz) * 0.0012);
    color = mix(color, g.horizon_color.rgb, clamp(fog, 0.0, 0.75));
    return vec4<f32>(color, alpha);
}
