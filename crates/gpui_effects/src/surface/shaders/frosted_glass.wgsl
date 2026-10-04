fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

fn gaussian_backdrop(input: BackdropInput, radius: f32) -> vec3<f32> {
    let weights = array<f32, 9>(
        0.02763,
        0.06628,
        0.12383,
        0.18017,
        0.20418,
        0.18017,
        0.12383,
        0.06628,
        0.02763,
    );
    let step_size = max(radius, 0.0) * 0.25;
    var color = vec3<f32>(0.0);
    for (var y = 0u; y < 9u; y = y + 1u) {
        for (var x = 0u; x < 9u; x = x + 1u) {
            let offset = vec2<f32>(f32(x) - 4.0, f32(y) - 4.0) * step_size;
            color += sample_raw_backdrop(input, offset).rgb * weights[x] * weights[y];
        }
    }
    return color;
}

fn rounded_rect(position: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let safe_half = max(half_size, vec2<f32>(1.0));
    let safe_radius = clamp(radius, 0.0, min(safe_half.x, safe_half.y));
    let corner = abs(position) - safe_half + vec2<f32>(safe_radius);
    return length(max(corner, vec2<f32>(0.0)))
        + min(max(corner.x, corner.y), 0.0)
        - safe_radius;
}

fn smooth_union(first: f32, second: f32, radius: f32) -> f32 {
    if (radius <= 0.001) {
        return min(first, second);
    }
    let h = max(radius - abs(first - second), 0.0) / radius;
    return min(first, second) - h * h * radius * 0.25;
}

fn frosted_sdf(
    position: vec2<f32>,
    shape_a: vec4<f32>,
    shape_b: vec4<f32>,
    config: vec4<f32>,
) -> f32 {
    let first = rounded_rect(position - shape_a.xy, shape_a.zw, config.x);
    if (config.w < 1.5) {
        return first;
    }
    let second = rounded_rect(position - shape_b.xy, shape_b.zw, config.y);
    return smooth_union(first, second, max(config.z, 0.0));
}

fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let shape_a = params.slots[0];
    let shape_b = params.slots[1];
    let shape_config = params.slots[2];
    let tint = params.slots[3];
    let appearance = params.slots[4];
    let edge = params.slots[5];
    let surface = params.slots[6];

    let position = input.uv * input.size;
    let sdf = frosted_sdf(position, shape_a, shape_b, shape_config);
    let coverage = 1.0 - smoothstep(-0.70, 0.70, sdf);
    let inside = max(-sdf, 0.0);

    var color = gaussian_backdrop(input, appearance.x);

    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, max(appearance.y, 0.0));
    color *= max(appearance.z, 0.0);
    color = mix(color, tint.rgb, saturate(tint.a));

    let diagonal = dot(input.uv - vec2<f32>(0.5), normalize(vec2<f32>(-1.0, 1.0)));
    let sheen = (1.0 - smoothstep(-0.28, 0.34, diagonal)) * surface.x;
    color += vec3<f32>(sheen);

    let edge_width = max(appearance.w, 0.5);
    let rim = smoothstep(0.0, 0.9, inside)
        * (1.0 - smoothstep(edge_width, edge_width + 1.0, inside));
    color = mix(color, edge.rgb, saturate(edge.a) * rim);

    return vec4<f32>(max(color, vec3<f32>(0.0)), coverage);
}
