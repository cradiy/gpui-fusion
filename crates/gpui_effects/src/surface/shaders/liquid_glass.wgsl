fn glass_optical_sample(input: BackdropInput, offset: vec2<f32>, clarity: f32) -> vec3<f32> {
    return mix(backdrop_to_srgb(sample_blurred_backdrop(input, offset).rgb),
        backdrop_to_srgb(sample_raw_backdrop(input, offset).rgb), clarity);
}

// Portions copyright (c) 2024 Charles Yin. MIT license: LICENSE-liquid-glass-studio.
fn glass_reflection_band(inside: f32, range: f32, hardness: f32, scale: f32) -> f32 {
    if (range <= 0.0) { return 0.0; }
    let base = max(1.0 - inside * scale / 1500.0 * pow(500.0 / range, 2.0) + hardness, 0.0);
    return clamp(pow(base, 5.0), 0.0, 1.0);
}

fn liquid_glass_material(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let optics = params.slots[0];
    let tint = params.slots[1];
    let surface = params.slots[2];
    let light = params.slots[3];
    let radii = params.slots[4];
    let edge_tint = params.slots[5];
    let fresnel_settings = params.slots[8];
    let glare = params.slots[9];
    let lighting = params.slots[10];
    let deformed = params.slots[6].z > 0.0;
    let p = select((input.uv - vec2<f32>(0.5)) * input.size,
        input.position - vec2<f32>(light.w, params.slots[6].w), deformed);
    let shape_size = select(input.size, params.slots[6].xy, deformed);
    let deformation = select(vec4<f32>(0.0), params.slots[7], deformed);
    let ellipticity = max(params.slots[6].z - 1.0, 0.0);
    let roundness = max(params.slots[11].x, 2.0);
    let merged_circle = params.slots[12];
    let distance = glass_material_contour(p, shape_size, radii, deformation, ellipticity, roundness, merged_circle);
    if (distance >= 0.5) { return vec4<f32>(0.0); }
    let inside = max(-distance, 0.0);
    let gradient = vec2<f32>(
        glass_material_contour(p + vec2<f32>(1.0, 0.0), shape_size, radii, deformation, ellipticity, roundness, merged_circle)
            - glass_material_contour(p - vec2<f32>(1.0, 0.0), shape_size, radii, deformation, ellipticity, roundness, merged_circle),
        glass_material_contour(p + vec2<f32>(0.0, 1.0), shape_size, radii, deformation, ellipticity, roundness, merged_circle)
            - glass_material_contour(p - vec2<f32>(0.0, 1.0), shape_size, radii, deformation, ellipticity, roundness, merged_circle),
    ) * 0.5;
    let normal = gradient / max(length(gradient), 0.0001);
    let thickness = max(optics.w, 0.0001);
    let edge_height = clamp(inside / thickness, 0.0, 1.0);
    let incident = asin(clamp(pow(1.0 - edge_height, 2.0), 0.0, 1.0));
    let transmitted = asin(clamp(sin(incident) / max(fresnel_settings.x, 1.0), 0.0, 1.0));
    var edge_factor = max(tan(incident - transmitted), 0.0);
    if (fresnel_settings.x <= 1.0 || optics.w <= 0.0 || inside >= optics.w) { edge_factor = 0.0; }
    var displacement = -gradient * edge_factor * optics.z;
    // Independent sharp/blurred inputs retain chromatic separation at the rim.
    let blur_mix = select(edge_height, 1.0, lighting.z > 0.0);
    let clarity = select(0.0, 1.0 - blur_mix, edge_factor > 0.0);
    var color = glass_optical_sample(input, displacement, clarity);
    if (surface.z > 0.0 && edge_factor > 0.0) {
        color.r = glass_optical_sample(input, displacement * (1.0 + surface.z), clarity).r;
        color.b = glass_optical_sample(input, displacement * (1.0 - surface.z), clarity).b;
    }
    let background = color;
    color = mix(color, tint.rgb, clamp(tint.a * 0.8, 0.0, 1.0));

    if (edge_factor > 0.0) {
        let scale = max(lighting.w, 1.0);
        // The reference normal is differentiated in physical viewport coordinates.
        let normal_length = length(gradient) * 1414.213562 / max(globals.viewport_size.y, 1.0);
        let fresnel = glass_reflection_band(inside, fresnel_settings.y, fresnel_settings.z, scale);
        var fresnel_lch = SRGB_TO_LCH(mix(vec3<f32>(1.0), tint.rgb, tint.a * 0.5));
        fresnel_lch.x = clamp(fresnel_lch.x + 20.0 * fresnel * fresnel_settings.w, 0.0, 100.0);
        color = mix(color, LCH_TO_SRGB(fresnel_lch), fresnel * fresnel_settings.w * 0.7 * normal_length);

        let glare_geo = glass_reflection_band(inside, glare.x, glare.y, scale);
        // Convert GPUI's downward Y axis to the material's upward Y axis.
        var angle = atan2(-normal.y, normal.x);
        if (angle < 0.0) { angle += 6.28318530718; }
        let glare_angle = (angle - 0.785398163397 + lighting.y) * 2.0;
        let far_side = (glare_angle > 4.71238898038 && glare_angle < 10.9955742876)
            || glare_angle < -1.57079632679;
        let side = 1.2 * select(1.0, lighting.x, far_side);
        let angular = clamp(pow(max((0.5 + sin(glare_angle) * 0.5) * side * glare.z, 0.0),
            0.1 + glare.w * 2.0), 0.0, 1.0);
        var glare_lch = SRGB_TO_LCH(mix(background, tint.rgb, tint.a * 0.5));
        glare_lch.x = clamp(glare_lch.x + 150.0 * angular * glare_geo, 0.0, 120.0);
        glare_lch.y += 30.0 * angular * glare_geo;
        color = mix(color, LCH_TO_SRGB(glare_lch), angular * glare_geo * normal_length);
    }

    let coverage = 1.0 - smoothstep(-0.5, 0.5, distance);
    return vec4<f32>(backdrop_from_srgb(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), coverage);
}
