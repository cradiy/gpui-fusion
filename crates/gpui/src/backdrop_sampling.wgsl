@group(1) @binding(2) var s_backdrop: sampler;

fn backdrop_sample_texture(texture: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(texture, s_backdrop, uv, 0.0);
}
