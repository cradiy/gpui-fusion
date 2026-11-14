// Copyright (c) 2026 Bilal Abdullah Gürşen. MIT: LICENSE-bilalgursen-liquid-glass.
fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let shadow = params.slots[13];
    let center = vec2<f32>(params.slots[3].w, params.slots[6].w);
    let p = input.position - center;
    let size = params.slots[6].xy;
    let scale = max(params.slots[10].w, 0.001);
    let distance = glass_material_contour(p, size, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12]);
    let near_distance = glass_material_contour(p - shadow.zw, size, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12]);
    let far_distance = glass_material_contour(p - shadow.zw * 1.6, size, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12]);
    let near = 1.0 - smoothstep(-6.0 * scale, max(shadow.x, 0.001), near_distance);
    let far = 1.0 - smoothstep(-10.0 * scale, max(shadow.x * 1.5, 0.001), far_distance);
    let vertical = smoothstep(-0.1, 0.9, p.y / max(size.y * 0.5, 0.001)) * 0.92 + 0.08;
    let amount = clamp((near + far) * shadow.y * vertical * smoothstep(-4.0 * scale, 14.0 * scale, distance), 0.0, 0.85);
    let raw = sample_raw_backdrop(input, vec2<f32>(0.0));
    let keep = pow(1.0 - amount, 1.0 / 2.2);
    return vec4<f32>(backdrop_from_srgb(backdrop_to_srgb(raw.rgb) * keep), raw.a + (1.0 - raw.a) * (1.0 - keep));
}
