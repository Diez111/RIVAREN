// RIVAREN ui.wgsl — quads instanciados con SDF rounded-rect + borde + clip.
// Instancias como vertex buffer (compatible Vulkan 1.1 / Mali-G52, sin storage en VS).

struct VsIn {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: vec4<f32>, // radius, border, _, _
    @location(3) border_color: vec4<f32>,
    @location(4) clip: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) clip: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: vec4<f32>; // w, h, scale, _

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    in: VsIn,
) -> VsOut {
    var out: VsOut;
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let px = mix(in.rect.x, in.rect.z, c.x);
    let py = mix(in.rect.y, in.rect.w, c.y);
    let ndc = vec2<f32>(px / screen.x * 2.0 - 1.0, 1.0 - py / screen.y * 2.0);
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.size = in.rect.zw - in.rect.xy;
    out.local = c * out.size;
    out.color = in.color;
    out.params = in.params;
    out.border_color = in.border_color;
    out.clip = in.clip;
    return out;
}

fn sd_round_rect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let rr = min(r, min(half.x, half.y));
    let q = abs(p) - half + vec2<f32>(rr);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let frag = vec2<f32>(in.pos.x, in.pos.y);
    if (frag.x < in.clip.x || frag.y < in.clip.y || frag.x > in.clip.z || frag.y > in.clip.w) {
        discard;
    }
    let half = in.size * 0.5;
    let p = in.local - half;
    let d = sd_round_rect(p, half, in.params.x);
    let aa = 0.75;
    var col = in.color;
    let border = in.params.y;
    if (border > 0.0) {
        let ring = abs(d + border * 0.5) - border * 0.5;
        let border_mask = 1.0 - smoothstep(-aa, aa, ring);
        col = mix(col, in.border_color, border_mask);
    }
    let alpha = col.a * (1.0 - smoothstep(-aa, aa, d));
    if (alpha <= 0.001) {
        discard;
    }
    return vec4<f32>(col.rgb, alpha);
}
