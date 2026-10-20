fn artwork_sample(input: EffectInput, uv: vec2<f32>, blend: f32) -> vec4<f32> {
    let current = sample_effect_image(input, uv);
    if (blend >= 1.0) { return current; }
    let previous = sample_effect_second_image(input, uv);
    let alpha = mix(previous.a, current.a, blend);
    let premultiplied = mix(previous.rgb * previous.a, current.rgb * current.a, blend);
    return vec4<f32>(premultiplied / max(alpha, 0.0001), alpha);
}

fn artwork_layer(input: EffectInput, p: vec2<f32>, pose: vec4<f32>, span: f32, blend: f32) -> vec4<f32> {
    let delta = p - pose.xy;
    let local = vec2<f32>(pose.z * delta.x + pose.w * delta.y,
        -pose.w * delta.x + pose.z * delta.y) / span + vec2<f32>(0.5);
    let edge = min(min(local.x, local.y), min(1.0 - local.x, 1.0 - local.y));
    let mask = smoothstep(0.0, 0.22, edge);
    let sample = artwork_sample(input, clamp(local, vec2<f32>(0.0), vec2<f32>(1.0)), blend);
    return vec4<f32>(sample.rgb, sample.a * mask);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let p = (input.uv - vec2<f32>(0.5)) * input.size / max(input.size.x, input.size.y);
    // A broad, bounded bend avoids recognizable square edges in the rotating layers.
    let bend = 0.8 * max(0.0, 0.7 - dot(p, p));
    let warped = vec2<f32>(p.x - bend * p.y, p.y + bend * p.x) / sqrt(1.0 + bend * bend);
    var color = vec3<f32>(0.0);
    let spans = params.slots[5];
    for (var i = 0u; i < 4u; i += 1u) {
        let layer = artwork_layer(input, warped, params.slots[i + 1u], spans[i], params.slots[0].w);
        let opacity = select(0.58, 1.0, i == 0u);
        color = mix(color, layer.rgb, layer.a * opacity);
    }
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, params.slots[0].y);
    color = clamp((color - vec3<f32>(0.5)) * params.slots[0].z + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));
    color *= params.slots[0].x;
    let edge = input.uv - vec2<f32>(0.5);
    color *= 1.0 - params.slots[6].x * smoothstep(0.02, 0.5, dot(edge, edge));
    let noise = fract(52.9829189 * fract(dot(floor(input.position), vec2<f32>(0.06711056, 0.00583715)))) - 0.5;
    color = clamp(color + vec3<f32>(noise * params.slots[6].y / 255.0), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(color, 1.0);
}
