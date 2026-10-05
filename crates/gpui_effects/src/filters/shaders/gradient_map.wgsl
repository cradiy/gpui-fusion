fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let source = sample_effect_image(input, input.uv);
    let luminance = clamp(dot(source.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
    var mapped = sample_effect_second_image(input, vec2<f32>(luminance, 0.5));
    mapped = select(mapped, params.slots[1], luminance <= 0.0);
    mapped = select(mapped, params.slots[2], luminance >= 1.0);
    mapped.a *= source.a;
    let strength = clamp(params.slots[0].x, 0.0, 1.0);
    let alpha = mix(source.a, mapped.a, strength);
    let color = mix(source.rgb * source.a, mapped.rgb * mapped.a, strength);
    return vec4<f32>(color / max(alpha, 0.00001), alpha);
}
