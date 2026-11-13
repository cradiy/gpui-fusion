// Portions copyright (c) 2024 Charles Yin. MIT license: LICENSE-liquid-glass-studio.
fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let shadow = params.slots[13];
    let center = vec2<f32>(params.slots[3].w, params.slots[6].w);
    let distance = glass_material_contour(input.position - center - shadow.zw,
        params.slots[6].xy, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12]);
    let raw = sample_raw_backdrop(input, vec2<f32>(0.0));
    let amount = exp(-abs(distance) / max(shadow.x, 0.0001)) * 0.6 * shadow.y;
    return vec4<f32>(backdrop_from_srgb(max(backdrop_to_srgb(raw.rgb) - vec3<f32>(amount), vec3<f32>(0.0))), raw.a);
}
