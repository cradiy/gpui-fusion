fn halftone_mix(a: vec4<f32>, b: vec4<f32>, amount: f32) -> vec4<f32> {
    let alpha = mix(a.a, b.a, amount);
    let premultiplied = mix(a.rgb * a.a, b.rgb * b.a, amount);
    return vec4<f32>(premultiplied / max(alpha, 0.00001), alpha);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let source = sample_effect_image(input, input.uv);
    let spacing = max(params.slots[0].x, 2.0);
    let cosine = params.slots[1].x;
    let sine = params.slots[1].y;
    let position = input.uv * input.size;
    let screen = vec2<f32>(cosine * position.x + sine * position.y,
        -sine * position.x + cosine * position.y) / spacing;
    let cell = floor(screen) + 0.5;
    let center = vec2<f32>(cosine * cell.x - sine * cell.y,
        sine * cell.x + cosine * cell.y) * spacing;
    let sample_color = sample_effect_image(input, clamp(center / input.size, vec2<f32>(0.0), vec2<f32>(1.0)));
    let luminance = mix(1.0, dot(sample_color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), sample_color.a);
    let radius = sqrt(clamp(1.0 - luminance, 0.0, 1.0)) * 0.70710678;
    let distance = length(screen - cell);
    let aa = max(fwidth(distance), 0.001);
    var coverage = 1.0 - smoothstep(radius - aa * 0.5, radius + aa * 0.5, distance);
    if (luminance >= 1.0) { coverage = 0.0; }
    if (luminance <= 0.0) { coverage = 1.0; }
    var printed = halftone_mix(params.slots[3], params.slots[2], coverage);
    printed.a *= source.a;
    return halftone_mix(source, printed, clamp(params.slots[1].z, 0.0, 1.0));
}
