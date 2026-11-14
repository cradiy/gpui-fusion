// Refraction profile from bilalgursen/liquid-glass/src/components/ui/glass/runtime.ts.
// Copyright (c) 2026 Bilal Abdullah Gürşen. MIT: ../../src/surface/shaders/LICENSE-bilalgursen-liquid-glass.
fn reference_distance(p: vec2<f32>, params: BackdropParams) -> f32 {
    let r = params.slots[4].x;
    let q = abs(p) - params.slots[6].xy / 2.0 + r;
    let outside = max(q, vec2<f32>(0.0));
    let n = params.slots[11].x;
    var d = pow(pow(outside.x, n) + pow(outside.y, n), 1.0 / n) + min(max(q.x, q.y), 0.0) - r;
    let circle = params.slots[12];
    if (circle.z > 0.0) {
        let other = length(p - circle.xy) - circle.z;
        let h = clamp(0.5 + 0.5 * (other - d) / circle.w, 0.0, 1.0);
        d = mix(other, d, h) - circle.w * h * (1.0 - h);
    }
    return d;
}
fn reference_sample(input: BackdropInput, offset: vec2<f32>, amount: f32) -> vec3<f32> {
    let a = backdrop_to_srgb(sample_raw_backdrop(input, offset).rgb);
    let b = backdrop_to_srgb(sample_blurred_backdrop(input, offset).rgb);
    return mix(a, b, amount);
}
fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let p = input.position - vec2<f32>(params.slots[3].w, params.slots[6].w);
    let d = reference_distance(p, params);
    let bevel = max(min(params.slots[0].w, max(params.slots[4].x * 1.2, params.slots[10].w)), 0.001);
    let t = pow(clamp(-d / bevel, 0.0, 1.0), 1.0 / 1.4);
    let height = sqrt(1.0 - pow(1.0 - t, 2.0));
    let edge = 1.0 - smoothstep(0.0, bevel * 0.35, max(-d, 0.0));
    let epsilon = params.slots[10].w * 1.25;
    let grad = vec2<f32>(reference_distance(p + vec2<f32>(epsilon, 0.0), params) - d,
        reference_distance(p + vec2<f32>(0.0, epsilon), params) - d) / epsilon;
    let dir = grad / max(length(grad), 0.0001);
    let surface = params.slots[2];
    let refract = -dir * params.slots[0].z * pow(edge, 1.6) * mix(0.06, 1.0, 1.0 - surface.x);
    let zoom = p / (1.0 + surface.y * height) - p;
    let blur = mix(surface.w, 1.0, max(pow(edge, 1.3), surface.x));
    let disp = surface.z * 0.35 * (1.0 - surface.x);
    var col = vec3<f32>(reference_sample(input, zoom + refract * (1.0 + disp), blur).r,
        reference_sample(input, zoom + refract, blur).g,
        reference_sample(input, zoom + refract * (1.0 - disp), blur).b);
    col = mix(col, mix(params.slots[14].rgb, params.slots[15].rgb, pow(1.0 - height, 1.6)), surface.x);
    col = mix(col, params.slots[1].rgb, params.slots[1].a);
    return vec4<f32>(backdrop_from_srgb(col), 1.0 - smoothstep(-0.5, 0.5, d));
}
