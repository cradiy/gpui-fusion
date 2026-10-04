fn grain_hash(value: u32) -> f32 {
    var h = value;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return f32(h >> 8u) / 16777215.0;
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let cell = vec2<u32>(floor(input.uv * input.size / max(params.slots[0].x, 1.0)));
    let seed = cell.x * 1597334677u ^ cell.y * 3812015801u;
    let value = grain_hash(seed) * 2.0 - 1.0;
    var noise = vec3<f32>(value);
    if (params.slots[0].z > 0.5) {
        noise = vec3<f32>(value,
            grain_hash(seed ^ 0x68bc21ebu) * 2.0 - 1.0,
            grain_hash(seed ^ 0x02e5be93u) * 2.0 - 1.0);
    }
    let magnitude = max(max(abs(noise.x), abs(noise.y)), abs(noise.z));
    let color = 0.5 + noise * (0.5 / max(magnitude, 0.0001));
    return vec4<f32>(color, magnitude * clamp(params.slots[0].y, 0.0, 1.0));
}
