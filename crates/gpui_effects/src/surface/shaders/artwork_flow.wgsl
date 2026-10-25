fn artwork_sample(input: EffectInput, uv: vec2<f32>, blend: f32) -> vec4<f32> {
    let centered_uv = (uv * input.image_size - vec2<f32>(0.5)) / max(input.image_size - vec2<f32>(1.0), vec2<f32>(1.0));
    let current = sample_effect_image(input, centered_uv);
    if (blend >= 1.0) { return current; }
    let inset = vec2<f32>(0.5) / input.second_image_size;
    let previous = sample_effect_second_image(input, clamp(uv, inset, vec2<f32>(1.0) - inset));
    let alpha = mix(previous.a, current.a, blend);
    let premultiplied = mix(previous.rgb * previous.a, current.rgb * current.a, blend);
    return vec4<f32>(premultiplied / max(alpha, 0.0001), alpha);
}

fn compose_artwork(input: EffectInput, p: vec2<f32>, params: EffectParams) -> vec4<f32> {
    var combined = vec4<f32>(0.0);
    let adaptive = params.slots[6].z > 0.5;
    for (var i = 0u; i < 4u; i += 1u) {
        let pose = params.slots[i + 1u];
        var center = pose.xy;
        if (i == 1u && !adaptive) { center.y *= input.size.y / input.size.x; }
        let delta = p - center;
        let uv = vec2<f32>(dot(delta, vec2<f32>(pose.z, pose.w)),
            dot(delta, vec2<f32>(-pose.w, pose.z))) / params.slots[5][i] + vec2<f32>(0.5);
        if (adaptive && i == 0u) {
            // Reflect the color field continuously across the base layer's edges.
            let mirrored = vec2<f32>(1.0) - abs(fract(uv * 0.5) * 2.0 - vec2<f32>(1.0));
            let layer = artwork_sample(input, mirrored, params.slots[0].w);
            combined = vec4<f32>(layer.rgb * layer.a, layer.a);
        } else if (all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0))) {
            var layer = artwork_sample(input, uv, params.slots[0].w);
            if (adaptive) {
                let edge = max(abs(uv.x * 2.0 - 1.0), abs(uv.y * 2.0 - 1.0));
                layer.a *= 1.0 - smoothstep(0.7, 1.0, edge);
            }
            combined = vec4<f32>(layer.rgb * layer.a, layer.a) + combined * (1.0 - layer.a);
        }
    }
    return vec4<f32>(combined.rgb / max(combined.a, 0.000001), combined.a);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    var pixel = (input.uv - vec2<f32>(0.5)) * input.size;
    let remaining = max(0.0, 1.0 - length(pixel) / params.slots[6].x);
    let angle = params.slots[6].y * remaining * remaining;
    let c = cos(angle);
    let s = sin(angle);
    if (params.slots[6].z > 0.5) {
        let p = input.uv - vec2<f32>(0.5);
        return compose_artwork(input, vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y), params);
    }
    pixel = vec2<f32>(c * pixel.x - s * pixel.y, s * pixel.x + c * pixel.y);
    let limit = max(input.size * 0.5 - vec2<f32>(0.5), vec2<f32>(0.0));
    pixel = clamp(pixel, -limit, limit);
    // Pull the composed layers through the twist before subsequent image-space blurs.
    return compose_artwork(input, pixel / input.size.x, params);
}
