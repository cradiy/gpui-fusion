// Lens material from bilalgursen/liquid-glass/src/components/ui/glass/runtime.ts.
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
fn reference_sample(input: BackdropInput, offset: vec2<f32>, lod: f32) -> vec3<f32> {
    return backdrop_to_srgb(sample_backdrop_lod(input, offset, lod).rgb);
}
fn reference_height(d: f32, bevel: f32) -> f32 {
    let t = pow(clamp(-d / bevel, 0.0, 1.0), 1.0 / 1.4);
    return sqrt(1.0 - pow(1.0 - t, 2.0));
}
fn reference_dim(c: vec3<f32>) -> vec3<f32> {
    return pow(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}
fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4<f32> {
    let p = input.position - vec2<f32>(params.slots[3].w, params.slots[6].w);
    let d = reference_distance(p, params);
    let bevel = max(params.slots[0].w, 0.001);
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
    let blur = max(params.slots[0].x * mix(surface.w, 1.0, max(pow(edge, 1.3), surface.x)) + log2(params.slots[10].w), 0.0);
    let disp = surface.z * 0.35 * (1.0 - surface.x);
    var col = vec3<f32>(reference_sample(input, zoom + refract * (1.0 + disp), blur).r,
        reference_sample(input, zoom + refract, blur).g,
        reference_sample(input, zoom + refract * (1.0 - disp), blur).b);
    let lighting = params.slots[8];
    let clear = 1.0 - surface.x;
    let nxy = p / (params.slots[6].xy * 0.5);
    let top = (1.0 - smoothstep(-0.05, 0.75, nxy.y)) * (1.0 - 0.12 * nxy.x) * lighting.z * clear * 0.6 + 0.02;
    col *= reference_dim(1.0 - vec3<f32>(1.0, 1.0, 0.72) * top);
    let depth = 1.0 - height;
    col = mix(col, mix(params.slots[14].rgb, params.slots[15].rgb, pow(depth, 1.6)), surface.x);
    col = mix(col, params.slots[1].rgb, params.slots[1].a);
    let up = clamp(-dir.y, 0.0, 1.0);
    let down = clamp(dir.y, 0.0, 1.0);
    let left = clamp(-dir.x, 0.0, 1.0);
    let right = clamp(dir.x, 0.0, 1.0);
    let inner = lighting.y * clear;
    col *= reference_dim(vec3<f32>(1.0 - smoothstep(-0.19 * bevel, -0.024 * bevel, d) * (up + 0.6 * left + 0.3 * right + 0.15 * down) * inner * 2.2));
    col *= reference_dim(vec3<f32>(1.0 - smoothstep(-0.73 * bevel, -0.13 * bevel, d) * left * inner * 2.0));
    col *= reference_dim(vec3<f32>(1.0 - exp(-pow((d + 0.45 * bevel) / (0.26 * bevel), 2.0)) * (0.9 * right + 0.5 * up + 0.4 * left + 0.3 * down) * inner * 1.5));
    let hg = vec2<f32>(reference_height(reference_distance(p + vec2<f32>(epsilon, 0.0), params), bevel),
        reference_height(reference_distance(p + vec2<f32>(0.0, epsilon), params), bevel)) - height;
    let normal = normalize(vec3<f32>(-hg / epsilon * 42.0 * params.slots[10].w, 1.0));
    let fresnel = pow(clamp(1.0 - normal.z, 0.0, 1.0), 6.0);
    let width = max(1.1, params.slots[10].w);
    let rim = 1.0 - smoothstep(0.0, width * 1.6, abs(d + width * 1.8));
    let falloff = exp(-pow(p.y / (30.0 * params.slots[10].w), 2.0));
    let rim_dir = right * right * falloff * 0.9 + down * down * 0.2 + left * left * 0.55;
    let warm = mix(vec3<f32>(0.82, 0.9, 1.08), vec3<f32>(1.2, 0.95, 0.78), 1.0 - smoothstep(-0.5, 0.1, nxy.x));
    let rim_col = mix(mix(vec3<f32>(1.0), warm, down * 0.6), vec3<f32>(0.12, 0.6, 1.2), left * 0.85);
    col += rim_col * (rim * rim_dir * lighting.x * mix(0.04, 1.0, clear) + fresnel * right * 0.02);
    col += vec3<f32>(1.0, 0.92, 0.86) * smoothstep(0.72, 0.98, depth) * down * lighting.w * clear;
    return vec4<f32>(backdrop_from_srgb(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0 - smoothstep(-0.5, 0.5, d));
}
