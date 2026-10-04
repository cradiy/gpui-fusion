fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let tau = 6.28318530718;
    let phase = input.time * tau;
    let point = (input.uv - vec2<f32>(0.5)) * params.slots[4].x;
    let a = sin((point.x * 5.0 + phase) + sin(point.y * 4.0 - phase));
    let b = sin((point.y * 6.0 - phase) + cos(point.x * 3.0 + phase));
    let c = sin(length(point + vec2<f32>(cos(phase), sin(phase)) * 0.35) * 9.0 - phase);
    let first = mix(params.slots[0], params.slots[1], 0.5 + 0.5 * a);
    let second = mix(params.slots[2], params.slots[3], 0.5 + 0.5 * b);
    return mix(first, second, 0.5 + 0.5 * c);
}
