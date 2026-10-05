fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let optics = params.slots[0];
    let surface = params.slots[1];
    let tint = params.slots[2];
    let axis = vec2<f32>(optics.z, optics.w);
    let phase = dot(input.uv * input.size, axis) / max(optics.x, 1.0) * 6.2831853;
    let offset = -axis * sin(phase) * optics.y;
    let footprint = max(length(vec2<f32>(1.0, 0.0) + dpdx(offset)),
        length(vec2<f32>(0.0, 1.0) + dpdy(offset)));
    var color = mix(sample_blurred_backdrop(input, offset).rgb,
        sample_raw_backdrop(input, offset).rgb, surface.x / max(footprint, 1.0));
    color = mix(color, tint.rgb, tint.a);
    let relief = clamp(optics.y / max(optics.x * 0.25, 1.0), 0.0, 1.0);
    let shine = pow(max(cos(phase - 0.65), 0.0), 10.0) * surface.y * relief;
    let shade = pow(max(-cos(phase - 0.65), 0.0), 4.0) * surface.y * relief * 0.35;
    color = mix(color * (1.0 - shade), vec3<f32>(1.0), shine);
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
