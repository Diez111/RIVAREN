// RIVAREN entity.wgsl — cajas instanciadas para mobs y entidades.

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
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world: vec3<f32>,
};

@vertex
fn vs_main(
    @location(0) v: vec3<f32>,
    @location(1) inst_pos: vec3<f32>,
    @location(2) inst_size: vec3<f32>,
    @location(3) inst_color: vec4<f32>,
) -> VsOut {
    var out: VsOut;
    let local = v * inst_size;
    let world = inst_pos + local;
    out.pos = g.view_proj * vec4<f32>(world, 1.0);
    out.color = inst_color;
    out.normal = normalize(v);
    out.world = world;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let ndl = max(dot(n, g.sun_dir.xyz), 0.0);
    let light = 0.35 + 0.65 * ndl * g.sun_dir.w;
    var color = in.color.rgb * light;
    let fog = 1.0 - exp(-g.sky_color.w * length(in.world - g.cam_pos.xyz) * 0.0009);
    color = mix(color, g.horizon_color.rgb, clamp(fog, 0.0, 0.6));
    return vec4<f32>(color, in.color.a);
}
