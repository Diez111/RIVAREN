// RIVAREN cull.comp.wgsl — frustum + distancia → IndirectDrawArgs.
// 1 thread por chunk. instance_count 0/1 (sin CPU loop). Fase 4 GPU-driven.

struct ChunkInfo {
    min_xyz_pad: vec4<f32>, // min del chunk + radio
    draw_index: u32,
};

struct Camera {
    view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    max_dist: f32,
};

@group(0) @binding(0) var<storage, read> chunks: array<ChunkInfo>;
@group(0) @binding(1) var<storage, read_write> draws: array<vec4<u32>>;
@group(0) @binding(2) var<uniform> cam: Camera;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= arrayLength(&chunks)) {
        return;
    }
    let c = chunks[i];
    // Distancia (LOD + cull).
    let d = distance(c.min_xyz_pad.xyz + vec3<f32>(16.0), cam.cam_pos);
    var visible = 1u;
    if (d > cam.max_dist) {
        visible = 0u;
    }
    // Frustum: 6 planos extraídos de view_proj (versión compacta: test esfera).
    let clip = cam.view_proj * vec4<f32>(c.min_xyz_pad.xyz + vec3<f32>(16.0), 1.0);
    let r = c.min_xyz_pad.w * (1.0 + d * 0.001);
    if (clip.w + r < abs(clip.x) || clip.w + r < abs(clip.y) || clip.w + r < abs(clip.z)) {
        visible = 0u;
    }
    draws[i] = vec4<u32>(draws[i].x, visible, draws[i].z, draws[i].w);
}
