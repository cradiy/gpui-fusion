fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let size = max(input.size, vec2<f32>(1.0));
    let cell = max(params.slots[0].x, 1.0);
    let start = floor(clamp(input.uv * size, vec2<f32>(0.0), size - 0.5) / cell) * cell;
    let center = start + min(vec2<f32>(cell), size - start) * 0.5;
    let inset = vec2<f32>(0.5) / size;
    let block = sample_effect_image(input, clamp(center / size, inset, 1.0 - inset));
    let source = sample_effect_image(input, input.uv);
    let strength = clamp(params.slots[1].x, 0.0, 1.0);
    let alpha = mix(source.a, block.a, strength);
    let color = mix(source.rgb * source.a, block.rgb * block.a, strength);
    return vec4<f32>(color / max(alpha, 0.00001), alpha);
}
