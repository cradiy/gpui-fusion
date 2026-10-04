fn spectrum_palette(position: f32, params: EffectParams) -> vec4<f32> {
    let cursor = fract(position) * 4.0;
    let index = u32(floor(cursor));
    let next_index = (index + 1u) % 4u;
    let t = fract(cursor);
    let smooth_t = t * t * (3.0 - 2.0 * t);
    return mix(params.slots[index], params.slots[next_index], smooth_t);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let tau = 6.28318530718;
    let phase = input.time * tau;
    let scale = max(params.slots[4].x, 0.1);
    let bend = params.slots[4].y;
    let shimmer = params.slots[4].z;
    let wave = sin(input.uv.y * tau * 1.35 + phase) * bend
        + sin((input.uv.x + input.uv.y) * tau * 0.72 - phase * 2.0) * bend * 0.45;
    let cursor = input.uv.x * scale + wave - input.time;
    var color = spectrum_palette(cursor, params);
    let light = 1.0 + sin((input.uv.x * 1.7 - input.uv.y) * tau + phase * 2.0) * shimmer;
    color = vec4<f32>(max(color.rgb * light, vec3<f32>(0.0)), color.a);
    return color;
}
