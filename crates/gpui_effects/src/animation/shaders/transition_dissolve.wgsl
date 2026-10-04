fn dissolve_hash(p: vec2<f32>) -> f32 {
    var q = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    q += dot(q, q.yzx + vec3<f32>(33.33));
    return fract((q.x + q.y) * q.z);
}

fn dissolve_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let blend = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(
        mix(dissolve_hash(cell), dissolve_hash(cell + vec2<f32>(1.0, 0.0)), blend.x),
        mix(dissolve_hash(cell + vec2<f32>(0.0, 1.0)), dissolve_hash(cell + vec2<f32>(1.0, 1.0)), blend.x),
        blend.y);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let progress = clamp(params.slots[0].x, 0.0, 1.0);
    if (progress == 0.0) { return sample_effect_image(input, input.uv); }
    if (progress == 1.0) { return sample_effect_second_image(input, input.uv); }
    let p = input.uv * input.size / max(params.slots[2].x, 1.0) + vec2<f32>(7.31, 19.17);
    let noise = 0.65 * dissolve_noise(p)
        + 0.25 * dissolve_noise(p * 2.0 + vec2<f32>(13.7, 5.3))
        + 0.10 * dissolve_noise(p * 4.0 + vec2<f32>(3.1, 27.9));
    let width = max(params.slots[0].z, 0.001);
    let threshold = mix(-width, 1.0 + width, progress);
    let coverage = smoothstep(noise - width, noise + width, threshold);
    return transition_mix(sample_effect_image(input, input.uv),
        sample_effect_second_image(input, input.uv), coverage);
}
