//! Pipelines WGSL: nombres de shaders en assets/shaders/.

pub const SHADERS: &[&str] = &[
    "traverse.comp.wgsl", // ray marching SVDAG + AADF
    "cull.comp.wgsl",     // frustum + HZB + distancia → IndirectDrawArgs
    "hzb.comp.wgsl",      // hierarchical Z-buffer
    "fluid.comp.wgsl",    // autómatas de fluidos
    "sky.wgsl",           // cielo + scattering
    "water.wgsl",         // agua SSR 1/2
    "post.wgsl",          // bloom + tonemap + FSR upscale
];
