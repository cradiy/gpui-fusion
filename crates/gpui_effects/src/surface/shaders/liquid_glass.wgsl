// Lens shading adapted from bilalgursen/liquid-glass.
// Copyright (c) 2026 Bilal Abdullah Gürşen. MIT: LICENSE-bilalgursen-liquid-glass.
fn glass_optical_sample(input: BackdropInput, offset: vec2<f32>, blur: f32) -> vec4<f32> {
    let raw = sample_raw_backdrop(input, offset);
    let blurred = sample_blurred_backdrop(input, offset);
    return mix(vec4<f32>(backdrop_to_srgb(raw.rgb), raw.a),
        vec4<f32>(backdrop_to_srgb(blurred.rgb), blurred.a), blur);
}

fn glass_optical_layer(base: vec4<f32>, color: vec3<f32>, weight: f32) -> vec4<f32> {
    let alpha = clamp(mix(base.a, 1.0, weight), 0.0, 1.0);
    let premultiplied = mix(base.rgb * base.a, color, weight);
    return vec4<f32>(premultiplied / max(alpha, 0.00001), alpha);
}

fn lens_height(distance: f32, bevel: f32) -> f32 {
    let t = pow(clamp(-distance / bevel, 0.0, 1.0), 1.0 / 1.4);
    return sqrt(max(1.0 - (1.0 - t) * (1.0 - t), 0.0));
}

fn lens_dim(value: vec3<f32>) -> vec3<f32> {
    return pow(clamp(value, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

fn liquid_glass_material(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let optics = params.slots[0];
    let tint = params.slots[1];
    let surface = params.slots[2];
    let lighting = params.slots[8];
    let scale = max(params.slots[10].w, 0.001);
    let deformed = params.slots[6].z > 0.0;
    let p = select((input.uv - vec2<f32>(0.5)) * input.size,
        input.position - vec2<f32>(params.slots[3].w, params.slots[6].w), deformed);
    let shape_size = select(input.size, params.slots[6].xy, deformed);
    let radii = params.slots[4];
    let deformation = select(vec4<f32>(0.0), params.slots[7], deformed);
    let ellipticity = max(params.slots[6].z - 1.0, 0.0);
    let roundness = max(params.slots[11].x, 2.0);
    let merged = params.slots[12];
    let distance = glass_material_contour(p, shape_size, radii, deformation, ellipticity, roundness, merged);
    if (distance >= 0.5) { return vec4<f32>(0.0); }

    // Limit rectangular bevels to their corners to avoid diagonal shading seams.
    let corner = select(select(radii.w, radii.x, p.y < 0.0),
        select(radii.z, radii.y, p.y < 0.0), p.x >= 0.0);
    let bevel = max(min(optics.w, max(corner * 1.2, scale)), 0.001);
    let eps = 1.25 * scale;
    let dx = glass_material_contour(p + vec2<f32>(eps, 0.0), shape_size, radii, deformation, ellipticity, roundness, merged);
    let dy = glass_material_contour(p + vec2<f32>(0.0, eps), shape_size, radii, deformation, ellipticity, roundness, merged);
    let gradient = vec2<f32>(dx - distance, dy - distance) / eps;
    let direction = gradient / max(length(gradient), 0.0001);
    let height = lens_height(distance, bevel);
    let height_gradient = vec2<f32>(lens_height(dx, bevel), lens_height(dy, bevel)) - vec2<f32>(height);
    let normal = normalize(vec3<f32>(-height_gradient / eps * 42.0 * scale, 1.0));
    let depth = clamp(1.0 - height, 0.0, 1.0);
    let edge = select(0.0, 1.0 - smoothstep(0.0, bevel * 0.35, max(-distance, 0.0)), optics.w > 0.0);
    let clear = 1.0 - surface.x;
    let displacement = -direction * optics.z * pow(edge, 1.6) * mix(0.06, 1.0, clear);
    let magnified = p / (1.0 + surface.y * height) - p;
    let blur = mix(surface.w, 1.0, max(pow(edge, 1.3), surface.x));
    let dispersion = surface.z * 0.35 * clear;
    var color = glass_optical_sample(input, magnified + displacement, blur);
    if (dispersion > 0.0 && edge > 0.0) {
        color.r = glass_optical_sample(input, magnified + displacement * (1.0 + dispersion), blur).r;
        color.b = glass_optical_sample(input, magnified + displacement * (1.0 - dispersion), blur).b;
    }
    let normalized = p / max(shape_size * 0.5, vec2<f32>(0.001));
    let top = ((1.0 - smoothstep(-0.05, 0.75, normalized.y)) * (1.0 - normalized.x * 0.12)
        * lighting.z * (0.6 + 0.02 / 0.3)) * clear;
    color = vec4<f32>(color.rgb * lens_dim(vec3<f32>(1.0) - vec3<f32>(1.0, 1.0, 0.72) * top), color.a);
    let frost_color = mix(params.slots[14].rgb, params.slots[15].rgb, pow(depth, 1.6));
    color = glass_optical_layer(color, frost_color, surface.x);
    color = glass_optical_layer(color, tint.rgb, clamp(tint.a, 0.0, 1.0));

    let up = clamp(-direction.y, 0.0, 1.0);
    let right = clamp(direction.x, 0.0, 1.0);
    let left = clamp(-direction.x, 0.0, 1.0);
    let down = clamp(direction.y, 0.0, 1.0);
    let inner = lighting.y * clear;
    let inner_band = smoothstep(-bevel * 0.19, -bevel * 0.024, distance);
    let deep_band = smoothstep(-bevel * 0.73, -bevel * 0.13, distance);
    let ring = exp(-pow((distance + bevel * 0.45) / (bevel * 0.26), 2.0));
    var shade = lens_dim(vec3<f32>(1.0 - inner_band * (up + left * 0.6 + right * 0.3 + down * 0.15) * inner * 2.2));
    shade *= lens_dim(vec3<f32>(1.0 - deep_band * left * inner * 2.0));
    shade *= lens_dim(vec3<f32>(1.0 - ring * (right * 0.9 + up * 0.5 + left * 0.4 + down * 0.3) * inner * 1.5));
    color = vec4<f32>(color.rgb * shade, color.a);

    let fresnel = pow(clamp(1.0 - normal.z, 0.0, 1.0), 6.0);
    let rim_width = max(1.1, scale);
    let rim = 1.0 - smoothstep(0.0, rim_width * 1.6, abs(distance + rim_width * 1.8));
    let falloff = exp(-pow(p.y / (30.0 * scale), 2.0));
    let static_rim = right * right * falloff * 0.9 + down * down * 0.2 + left * left * 0.55;
    let light = params.slots[9].xy / max(length(params.slots[9].xy), 0.0001);
    let facing = clamp(dot(direction, light), 0.0, 1.0);
    let away = clamp(-dot(direction, light), 0.0, 1.0);
    let lean = params.slots[9].z * 0.4;
    let rim_direction = mix(static_rim, facing * facing * 0.9 + away * away * 0.55, lean);
    let warm_cool = mix(vec3<f32>(0.82, 0.9, 1.08), vec3<f32>(1.2, 0.95, 0.78),
        1.0 - smoothstep(-0.5, 0.1, normalized.x));
    var rim_color = mix(vec3<f32>(1.0), warm_cool, down * 0.6);
    rim_color = mix(rim_color, vec3<f32>(0.12, 0.6, 1.2), mix(left, away, lean) * 0.85);
    let reflection = rim_color * (rim * rim_direction * lighting.x + fresnel * mix(right, facing, lean) * 0.02 * lighting.x / 0.55) * clear;
    let caustic = vec3<f32>(1.0, 0.92, 0.86) * smoothstep(0.72, 0.98, depth) * down * lighting.w * clear;
    color = vec4<f32>(color.rgb + reflection + caustic, color.a);
    let coverage = 1.0 - smoothstep(-0.5, 0.5, distance);
    return vec4<f32>(backdrop_from_srgb(clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), coverage * color.a);
}
