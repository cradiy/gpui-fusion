fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let source = sample_effect_image(input, input.uv);
    let size = max(input.size, vec2<f32>(1.0));
    let delta = (input.uv - 0.5) * size;
    let distance = length(delta);
    let radius = max(length(size * 0.5), 1.0);
    let falloff = clamp(distance / radius, 0.0, 1.0);
    let radial = delta / max(distance, 0.00001) * falloff * falloff;
    let direction = mix(params.slots[1].xy, radial, params.slots[1].z);
    let offset = direction * params.slots[0].x / size;
    let inset = vec2<f32>(0.5) / size;
    let red = sample_effect_image(input, clamp(input.uv + offset, inset, 1.0 - inset));
    let blue = sample_effect_image(input, clamp(input.uv - offset, inset, 1.0 - inset));
    // Uncovered samples must not introduce hidden RGB or dark transparent fringes.
    let coverage = clamp(vec2<f32>(red.a, blue.a) / max(source.a, 0.00001), vec2<f32>(0.0), vec2<f32>(1.0));
    return vec4<f32>(mix(source.r, red.r, coverage.x), source.g,
        mix(source.b, blue.b, coverage.y), source.a);
}
