fn trail_color(input: EffectInput, params: EffectParams, t: f32) -> vec4<f32> {
    let r = params.slots[2];
    let inset = params.slots[3].z;
    let w = input.size.x;
    let h = input.size.y;
    let half_pi = 1.57079632679;
    let top = w - r.x - r.y;
    let right = h - r.y - r.z;
    let bottom = w - r.z - r.w;
    let left = h - r.w - r.x;
    let arcs = r * half_pi;
    let perimeter = top + right + bottom + left + dot(arcs, vec4<f32>(1.0));
    var d = t * perimeter;
    var p: vec2<f32>;
    // Sample safely inside the captured border, where antialiasing does not reduce alpha.
    if d <= top {
        p = vec2<f32>(r.x + d, inset);
    } else if d <= top + arcs.y {
        let a = (d - top) / max(r.y, 0.001) - half_pi;
        p = vec2<f32>(w - r.y, r.y) + max(r.y - inset, 0.0) * vec2<f32>(cos(a), sin(a));
    } else if d <= top + arcs.y + right {
        p = vec2<f32>(w - inset, r.y + d - top - arcs.y);
    } else if d <= top + arcs.y + right + arcs.z {
        let a = (d - top - arcs.y - right) / max(r.z, 0.001);
        p = vec2<f32>(w - r.z, h - r.z) + max(r.z - inset, 0.0) * vec2<f32>(cos(a), sin(a));
    } else if d <= top + arcs.y + right + arcs.z + bottom {
        d -= top + arcs.y + right + arcs.z;
        p = vec2<f32>(w - r.z - d, h - inset);
    } else if d <= top + arcs.y + right + arcs.z + bottom + arcs.w {
        let a = (d - top - arcs.y - right - arcs.z - bottom) / max(r.w, 0.001) + half_pi;
        p = vec2<f32>(r.w, h - r.w) + max(r.w - inset, 0.0) * vec2<f32>(cos(a), sin(a));
    } else if d <= perimeter - arcs.x {
        p = vec2<f32>(inset, r.x + perimeter - arcs.x - d);
    } else {
        let a = (d - perimeter + arcs.x) / max(r.x, 0.001) + 2.0 * half_pi;
        p = vec2<f32>(r.x, r.x) + max(r.x - inset, 0.0) * vec2<f32>(cos(a), sin(a));
    }
    return sample_effect_image(input, (p - vec2<f32>(0.5)) / max(input.size - vec2<f32>(1.0), vec2<f32>(1.0)));
}
