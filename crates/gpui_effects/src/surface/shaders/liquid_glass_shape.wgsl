fn glass_distance(p: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    var radius = select(radii.w, radii.x, p.y < 0.0);
    if (p.x >= 0.0) {
        radius = select(radii.z, radii.y, p.y < 0.0);
    }
    let half_size = size * 0.5;
    radius = clamp(radius, 0.0, min(half_size.x, half_size.y));
    let corner = abs(p) - half_size + radius;
    return length(max(corner, vec2<f32>(0.0)))
        + min(max(corner.x, corner.y), 0.0) - radius;
}

fn glass_contour(p: vec2<f32>, size: vec2<f32>, radii: vec4<f32>, deformation: vec4<f32>, ellipticity: f32) -> f32 {
    var base = glass_distance(p, size, radii);
    if (ellipticity > 0.0) {
        let half_size = max(size * 0.5, vec2<f32>(1.0));
        let radial = p / half_size;
        let k0 = length(radial);
        let k1 = length(radial / half_size);
        let ellipse = select(-min(half_size.x, half_size.y),
            k0 * (k0 - 1.0) / max(k1, 0.0001), k1 > 0.0001);
        base = mix(base, ellipse, clamp(ellipticity, 0.0, 1.0));
    }
    if (deformation.z == 0.0 && deformation.w == 0.0) {
        return base;
    }
    let q = p / max(size * 0.5, vec2<f32>(1.0));
    let direction = q / max(length(q), 0.0001);
    var focus = deformation.xy * 2.0 - vec2<f32>(1.0);
    if (length(focus) < 0.15) {
        focus = vec2<f32>(0.6, -0.8);
    }
    focus = normalize(focus);
    let alignment = dot(direction, focus);
    let facing = clamp(alignment * 0.5 + 0.5, 0.0, 1.0);
    let local = facing * facing;
    let tangent = direction.x * focus.y - direction.y * focus.x;
    let wave = tangent * alignment;
    return base
        - deformation.z * (0.1 + local * 0.9) - deformation.w * wave;
}
