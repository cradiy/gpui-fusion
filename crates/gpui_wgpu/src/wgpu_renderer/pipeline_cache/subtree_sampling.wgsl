fn sample_effect_NAMEimage(input: EffectInput, uv: vec2<f32>) -> vec4<f32> {
    let pixel = input.NAMEimage_origin + uv * input.NAMEimage_size - vec2<f32>(0.5);
    let first = floor(pixel) + vec2<f32>(0.5);
    let dimensions = vec2<f32>(textureDimensions(t_effect_NAMEimage));
    // Every interpolation tap must remain inside the capture and texture.
    // At their edges the source helper supplies transparent-border sampling.
    if all(first >= max(input.NAMEimage_origin, vec2<f32>(0.5)))
        && all(first + vec2<f32>(1.0) < min(input.NAMEimage_origin + input.NAMEimage_size, dimensions)) {
        let color = textureSampleLevel(t_effect_NAMEimage, s_subtree_image, (pixel + vec2<f32>(0.5)) / dimensions, 0.0);
        return vec4<f32>(color.rgb / max(color.a, 0.000001), color.a);
    }
    return sample_effect_NAMEimage_manual(input, uv);
}
