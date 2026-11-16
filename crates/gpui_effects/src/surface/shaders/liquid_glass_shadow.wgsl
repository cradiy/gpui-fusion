// Copyright (c) 2026 Bilal Abdullah Gürşen. MIT: LICENSE-bilalgursen-liquid-glass.
fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let shadow = params.slots[13];
    let center = vec2<f32>(params.slots[3].w, params.slots[6].w);
    let p = input.position - center;
    let size = params.slots[6].xy;
    let scale = max(params.slots[10].w, 0.001);
    let distance = glass_material_contour(p, size, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12], params.slots[11].yzw);
    let near_distance = glass_material_contour(p - shadow.zw, size, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12], params.slots[11].yzw);
    let far_distance = glass_material_contour(p - shadow.zw * 1.6, size, params.slots[4], params.slots[7],
        max(params.slots[6].z - 1.0, 0.0), params.slots[11].x, params.slots[12], params.slots[11].yzw);
    let near = 1.0 - smoothstep(-6.0 * scale, max(shadow.x, 0.001), near_distance);
    let far = 1.0 - smoothstep(-10.0 * scale, max(shadow.x * 1.5, 0.001), far_distance);
    var local_y = p.y;
    var local_height = size.y;
    let pane = params.slots[11].yzw;
    let merged = params.slots[12];
    if (all(pane.xy > vec2<f32>(0.0))) {
        let primary = glass_distance(p, size, params.slots[4]);
        let secondary = glass_distance(p - merged.xy, pane.xy, vec4<f32>(pane.z));
        let weight = clamp(0.5 + 0.5 * (primary - secondary) / max(merged.w, 0.001), 0.0, 1.0);
        local_y -= merged.y * weight;
        local_height = mix(size.y, pane.y, weight);
    }
    let vertical = smoothstep(-0.1, 0.9, local_y / max(local_height * 0.5, 0.001)) * 0.92 + 0.08;
    let amount = clamp((near + far) * shadow.y * vertical * smoothstep(-4.0 * scale, 14.0 * scale, distance), 0.0, 0.85);
    let raw = sample_raw_backdrop(input, vec2<f32>(0.0));
    let keep = pow(1.0 - amount, 1.0 / 2.2);
    return vec4<f32>(backdrop_from_srgb(backdrop_to_srgb(raw.rgb) * keep), raw.a + (1.0 - raw.a) * (1.0 - keep));
}
