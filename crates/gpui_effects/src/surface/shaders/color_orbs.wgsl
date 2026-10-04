fn effect_orb_weight(point: vec2<f32>, center: vec2<f32>, sharpness: f32) -> f32 {
    let delta = point - center;
    return exp(-dot(delta, delta) * sharpness);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let tau = 6.28318530718;
    let phase = input.time * tau;
    let aspect = input.size.x / max(input.size.y, 1.0);
    let point = (input.uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let radius = vec2<f32>(0.32 * aspect, 0.3);
    let center0 = vec2<f32>(cos(phase), sin(phase)) * radius;
    let center1 = vec2<f32>(cos(phase + 1.5707963), sin(phase * 2.0 + 1.2)) * radius;
    let center2 = vec2<f32>(cos(-phase + 3.0), sin(phase + 2.8)) * radius;
    let center3 = vec2<f32>(cos(phase * 2.0 + 4.5), sin(-phase + 4.0)) * radius;
    let sharpness = params.slots[4].x;
    let weights = vec4<f32>(
        effect_orb_weight(point, center0, sharpness),
        effect_orb_weight(point, center1, sharpness),
        effect_orb_weight(point, center2, sharpness),
        effect_orb_weight(point, center3, sharpness),
    );
    let total = max(dot(weights, vec4<f32>(1.0)), 0.0001);
    let color = (
        params.slots[0] * weights.x +
        params.slots[1] * weights.y +
        params.slots[2] * weights.z +
        params.slots[3] * weights.w
    ) / total;
    let glow = clamp(total * 0.42, 0.0, 1.0);
    let rgb = color.rgb * (0.72 + glow * 0.45);
    return vec4<f32>(rgb, clamp(color.a, 0.0, 1.0));
}
