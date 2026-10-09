fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let position = vec3<f32>(input.uv * input.size, 1.0);
    let source = vec2<f32>(dot(params.slots[0].xyz, position), dot(params.slots[1].xyz, position));
    if (any(source < vec2<f32>(0.0)) || any(source >= input.size)) {
        return vec4<f32>(0.0);
    }
    let half_size = input.size * 0.5;
    let p = input.uv * input.size - half_size;
    let top = select(params.slots[2].x, params.slots[2].y, p.x > 0.0);
    let bottom = select(params.slots[2].w, params.slots[2].z, p.x > 0.0);
    let radius = select(top, bottom, p.y > 0.0);
    var coverage = 1.0;
    if (radius > 0.0) {
        let q = abs(p) - half_size + vec2<f32>(radius);
        let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
        coverage = 1.0 - smoothstep(-0.5, 0.5, distance);
    }
    return sample_effect_image(input, source / max(input.size, vec2<f32>(0.0001))) * coverage;
}
