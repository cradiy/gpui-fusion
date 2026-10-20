fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let delta = vec2<f32>(params.slots[0].x) / input.size;
    var total = vec4<f32>(0.0);
    for (var i = 0u; i < 4u; i += 1u) {
        let offset = vec2<f32>(select(-delta.x, delta.x, (i & 1u) != 0u),
            select(-delta.y, delta.y, (i & 2u) != 0u));
        let inset = vec2<f32>(0.5) / input.size;
        let sampled = sample_effect_image(input, clamp(input.uv + offset, inset, vec2<f32>(1.0) - inset));
        total += vec4<f32>(sampled.rgb * sampled.a, sampled.a);
    }
    return vec4<f32>(total.rgb / max(total.a, 0.000001), total.a * 0.25);
}
