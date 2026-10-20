fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let source = sample_effect_image(input, input.uv);
    let gray = dot(source.rgb, vec3<f32>(0.2125, 0.7154, 0.0721));
    let saturated = vec3<f32>(gray) + (source.rgb - vec3<f32>(gray)) * params.slots[0].y;
    let adjusted = ((saturated - vec3<f32>(0.5)) * params.slots[0].z + vec3<f32>(0.5)) * params.slots[0].x;
    let premultiplied = clamp(adjusted * source.a, vec3<f32>(0.0), vec3<f32>(1.0));
    var color = clamp(premultiplied + vec3<f32>(1.0 - source.a), vec3<f32>(0.0), vec3<f32>(1.0));
    color = color * 0.475 + vec3<f32>(0.05);
    let noise = fract(52.9829189 * fract(dot(floor(input.position), vec2<f32>(0.06711056, 0.00583715)))) - 0.5;
    color += vec3<f32>(noise * params.slots[0].w / 255.0);
    let radii = params.slots[1];
    let radius = select(select(radii.z, radii.y, input.uv.y < 0.5),
        select(radii.w, radii.x, input.uv.y < 0.5), input.uv.x < 0.5);
    let q = abs((input.uv - vec2<f32>(0.5)) * input.size) - input.size * 0.5 + vec2<f32>(radius);
    let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    return vec4<f32>(color, 1.0 - smoothstep(-0.5, 0.5, distance));
}
