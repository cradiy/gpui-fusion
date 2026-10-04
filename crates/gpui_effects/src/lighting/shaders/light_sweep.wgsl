fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let band = params.slots[0];
    let color = params.slots[1];
    let p = (input.uv - vec2<f32>(0.5)) * input.size;
    let distance = abs(dot(p, band.xy) - band.z) / max(band.w, 0.001);
    let light = 1.0 - smoothstep(0.0, 1.0, distance);
    return vec4<f32>(color.rgb, color.a * params.slots[2].x * light);
}
