fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let optics = params.slots[0];
    let tint = params.slots[1];
    let surface = params.slots[2];
    let light = params.slots[3];
    let radii = params.slots[4];
    let lens = params.slots[5];
    let p = input.position - vec2<f32>(light.w, params.slots[6].w);
    let size = params.slots[6].xy;
    let deformation = params.slots[7];
    let distance = glass_contour(p, size, radii, deformation, max(params.slots[6].z - 1.0, 0.0));
    let inside = max(-distance, 0.0);
    let gradient = vec2<f32>(
        glass_contour(p + vec2<f32>(0.5, 0.0), size, radii, deformation, max(params.slots[6].z - 1.0, 0.0))
            - glass_contour(p - vec2<f32>(0.5, 0.0), size, radii, deformation, max(params.slots[6].z - 1.0, 0.0)),
        glass_contour(p + vec2<f32>(0.0, 0.5), size, radii, deformation, max(params.slots[6].z - 1.0, 0.0))
            - glass_contour(p - vec2<f32>(0.0, 0.5), size, radii, deformation, max(params.slots[6].z - 1.0, 0.0)),
    );
    let normal = gradient / max(length(gradient), 0.0001);
    let thickness = clamp(optics.w, 0.001, min(size.x, size.y) * 0.45);
    let t = clamp(inside / thickness, 0.0, 1.0);
    // A smooth bevel normal, with air-to-glass Snell refraction at n = 1.5.
    let cos_incident = smoothstep(0.0, 1.0, t);
    let sin_incident = sqrt(max(1.0 - cos_incident * cos_incident, 0.0));
    let eta = 1.0 / 1.5;
    let cos_transmitted = sqrt(max(1.0 - eta * eta * sin_incident * sin_incident, 0.0001));
    let bend = sin_incident * (1.0 - eta * cos_incident / cos_transmitted);
    // Keep the transmission map ordered across the curved band, including
    // dispersed channels. Wider sampling jumps fold scene boundaries back.
    let reach = min(max(optics.z, 0.0), thickness * 0.3 / (1.0 + surface.z));
    let body_offset = p * (1.0 / clamp(lens.x, 1.0, 1.25) - 1.0) - normal * reach * bend;
    let edge_width = min(max(lens.z, 0.001), thickness);
    let outward_reach = min(max(lens.y, 0.0), min(size.x, size.y) * 0.2);
    // Transmission begins at the silhouette's own scene coordinate and bends
    // inward. Surroundings enter only through the independent Fresnel reflection.
    let edge_progress = clamp(inside / edge_width, 0.0, 1.0);
    let entry = smoothstep(0.0, 0.35, edge_progress);
    let body = smoothstep(0.4, 1.0, edge_progress);
    let displacement = body_offset * body;
    let separation = normal * surface.z * reach * bend * body;
    let red_offset = displacement + separation;
    let blue_offset = displacement - separation;
    // Quad derivatives include neighboring fragments. Fade their filtering at
    // the outer boundary as well as the offset, preserving scene registration.
    let clarity = mix(1.0, surface.w, entry);
    let footprint = mix(1.0, glass_footprint(displacement), entry);
    let red_footprint = mix(1.0, glass_footprint(red_offset), entry);
    let blue_footprint = mix(1.0, glass_footprint(blue_offset), entry);
    var color = glass_sample(input, displacement, clarity, footprint);
    if (surface.z > 0.0) {
        color.r = glass_sample(input, red_offset, clarity, red_footprint).r;
        color.b = glass_sample(input, blue_offset, clarity, blue_footprint).b;
    }
    let reflection_offset = normal * outward_reach;
    var reflected = sample_blurred_backdrop(input, reflection_offset).rgb;
    if (surface.z > 0.0) {
        reflected.r = sample_blurred_backdrop(input, reflection_offset + separation).r;
        reflected.b = sample_blurred_backdrop(input, reflection_offset - separation).b;
    }
    let fresnel = 0.04 * (1.0 - smoothstep(0.0, 1.0, t))
        + 0.96 * pow(1.0 - cos_incident, 5.0);
    color = mix(color, reflected, clamp(lens.w, 0.0, 1.0) * fresnel);
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, optics.x) * optics.y;
    color = mix(color, tint.rgb, tint.a);
    let direction = light.xy / max(length(light.xy), 0.0001);
    let facing = dot(normal, direction);
    color *= 1.0 - surface.y * bend * entry;
    let rim = smoothstep(0.0, 1.0, inside)
        * (1.0 - smoothstep(max(light.z, 0.0), max(light.z, 0.0) + 1.0, inside));
    let recessed_facing = mix(-facing, facing, body);
    let highlight = surface.x * rim * (0.08 + 0.92 * pow(max(recessed_facing, 0.0), 4.0));
    let rim_reflection = glass_border_reflection(reflected, 0.35);
    color = mix(color, rim_reflection, highlight);
    let coverage = 1.0 - smoothstep(-0.5, 0.5, distance);
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), coverage);
}
