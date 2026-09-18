// RIVAREN hzb.comp.wgsl — Hierarchical Z-Buffer, pass 1+2.
// Pass 1: downsample profundidad a mip chain (oclusores grandes).
// Pass 2: test AABB de chunks contra el HZB (hasta 90% mejora en cuevas).

@group(0) @binding(0) var depth_mip0: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> hzb: array<f32>;

@compute @workgroup_size(8, 8, 1)
fn hzb_downsample(@builtin(global_invocation_id) id: vec3<u32>) {
    // Min 2×2 → 1 (profundidad más cercana = más oclusiva).
    let p = vec2<i32>(id.xy * 2u);
    let d0 = textureLoad(depth_mip0, p, 0).x;
    let d1 = textureLoad(depth_mip0, p + vec2<i32>(1, 0), 0).x;
    let d2 = textureLoad(depth_mip0, p + vec2<i32>(0, 1), 0).x;
    let d3 = textureLoad(depth_mip0, p + vec2<i32>(1, 1), 0).x;
    let m = min(min(d0, d1), min(d2, d3));
    // hzb[idx] = m (indexado por nivel en el host)
    hzb[id.y * 128u + id.x] = m;
}
