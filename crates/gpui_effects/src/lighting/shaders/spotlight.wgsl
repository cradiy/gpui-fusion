fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let light = params.slots[0];
    let color = params.slots[1];
    let settings = params.slots[2];
    let radii = params.slots[3];
    let p = input.uv * input.size;
    let q = p - input.size * 0.5;
    let radius = select(select(radii.y, radii.x, q.x < 0.0),
                        select(radii.z, radii.w, q.x < 0.0), q.y >= 0.0);
    let corner = abs(q) - input.size * 0.5 + radius;
    let sdf = length(max(corner, vec2<f32>(0.0))) + min(max(corner.x, corner.y), 0.0) - radius;
    let distance = length(p - light.xy) / max(light.z, 0.001);
    let falloff = 1.0 - smoothstep(0.0, 1.0, distance);
    let surface = settings.x * falloff * falloff;
    var edge = 0.0;
    if settings.z > 0.0 {
        edge = (1.0 - smoothstep(settings.z - 0.5, settings.z + 0.5, -sdf))
            * settings.y * falloff;
    }
    let alpha = (surface + edge * (1.0 - surface)) * light.w * color.a;
    return vec4<f32>(color.rgb, clamp(alpha, 0.0, 1.0));
}
