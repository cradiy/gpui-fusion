@group(1) @binding(2) var s_backdrop: sampler;

// `textureSample` requires uniform control flow: WGSL rejects it when the call
// sits behind a non-uniform branch, and every glass effect samples the backdrop
// conditionally (edge tint, per-tap weights). `textureSampleLevel` carries an
// explicit mip level instead of deriving it, so it is legal in non-uniform flow.
// Backdrop targets are single-mip (`mip_level_count: 1`), so level 0 is the only
// level and the result matches `textureSample` exactly.
fn backdrop_sample_texture(texture: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(texture, s_backdrop, uv, 0.0);
}
