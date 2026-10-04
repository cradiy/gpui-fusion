fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let metric = input.size / max(min(input.size.x, input.size.y), 1.0);
    var distances: array<f32, 4>;
    var nearest = 1e20;
    for (var i = 0u; i < 4u; i += 1u) {
        let source = params.slots[i * 2u];
        let delta = (input.uv - source.xy) * metric / max(source.z, 0.01);
        distances[i] = dot(delta, delta) * 2.0;
        if (source.z > 0.0) {
            nearest = min(nearest, distances[i]);
        }
    }
    var color = vec4<f32>(0.0);
    var total = 0.0;
    for (var i = 0u; i < 4u; i += 1u) {
        if (params.slots[i * 2u].z <= 0.0) {
            continue;
        }
        // Subtract the nearest distance so small radii cannot underflow all weights.
        let weight = exp(nearest - distances[i]);
        let source = params.slots[i * 2u + 1u];
        color += vec4<f32>(source.rgb * source.a, source.a) * weight;
        total += weight;
    }
    if (total <= 0.0 || color.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let pixel = floor(input.uv * input.size);
    let noise = fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
    return vec4<f32>(clamp(color.rgb / color.a + (noise - 0.5) / 255.0, vec3<f32>(0.0), vec3<f32>(1.0)), color.a / total);
}
