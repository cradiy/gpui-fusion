const SRGB: bool = false;
@group(0) @binding(0) var source: texture_2d<f32>;

fn encode(c: vec4<f32>) -> vec4<f32> {
    if (!SRGB) { return c; }
    return vec4<f32>(select(c.rgb * 12.92,
        1.055 * pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055,
        c.rgb > vec3<f32>(0.0031308)), c.a);
}
fn decode(c: vec4<f32>) -> vec4<f32> {
    if (!SRGB) { return c; }
    return vec4<f32>(select(c.rgb / 12.92,
        pow((max(c.rgb, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4)),
        c.rgb > vec3<f32>(0.04045)), c.a);
}
@vertex
fn vertex(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fragment(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let size = textureDimensions(source);
    let target_size = max(size / 2u, vec2<u32>(1u));
    let at = p.xy / vec2<f32>(target_size) * vec2<f32>(size) - 0.5;
    let low = vec2<i32>(floor(at));
    let high = min(low + 1, vec2<i32>(size) - 1);
    let lo = max(low, vec2<i32>(0));
    let f = fract(at);
    let a = mix(encode(textureLoad(source, lo, 0)), encode(textureLoad(source, vec2<i32>(high.x, lo.y), 0)), f.x);
    let b = mix(encode(textureLoad(source, vec2<i32>(lo.x, high.y), 0)), encode(textureLoad(source, high, 0)), f.x);
    return decode(mix(a, b, f.y));
}
