fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let settings = params.slots[0];
    let r = params.slots[2];
    let p = input.uv * input.size;
    let w = input.size.x;
    let h = input.size.y;
    let half_pi = 1.57079632679;
    let q = p - input.size * 0.5;
    let radius = select(select(r.y, r.x, q.x < 0.0),
                        select(r.z, r.w, q.x < 0.0), q.y >= 0.0);
    let corner = abs(q) - input.size * 0.5 + radius;
    let sdf = length(max(corner, vec2<f32>(0.0))) + min(max(corner.x, corner.y), 0.0) - radius;
    let edge = 1.0 - smoothstep(settings.z - 0.5, settings.z + 0.5, -sdf);
    if edge <= 0.0 || settings.w <= 0.0 {
        return vec4<f32>(0.0);
    }

    // Cumulative arc length of four straight edges and four quarter circles.
    let top = w - r.x - r.y;
    let right_start = top + half_pi * r.y;
    let br_start = right_start + h - r.y - r.z;
    let bottom_start = br_start + half_pi * r.z;
    let bl_start = bottom_start + w - r.z - r.w;
    let left_start = bl_start + half_pi * r.w;
    let tl_start = left_start + h - r.w - r.x;
    let perimeter = max(tl_start + half_pi * r.x, 0.001);
    var path = 0.0;
    if p.x < r.x && p.y < r.x {
        path = tl_start + (atan2(p.y - r.x, p.x - r.x) + 2.0 * half_pi) * r.x;
    } else if p.x > w - r.y && p.y < r.y {
        path = top + (atan2(p.y - r.y, p.x - w + r.y) + half_pi) * r.y;
    } else if p.x > w - r.z && p.y > h - r.z {
        path = br_start + atan2(p.y - h + r.z, p.x - w + r.z) * r.z;
    } else if p.x < r.w && p.y > h - r.w {
        path = bl_start + (atan2(p.y - h + r.w, p.x - r.w) - half_pi) * r.w;
    } else {
        let distances = vec4<f32>(p.y, w - p.x, h - p.y, p.x);
        let nearest = min(min(distances.x, distances.y), min(distances.z, distances.w));
        if nearest == distances.x {
            path = clamp(p.x - r.x, 0.0, top);
        } else if nearest == distances.y {
            path = right_start + clamp(p.y - r.y, 0.0, h - r.y - r.z);
        } else if nearest == distances.z {
            path = bottom_start + clamp(w - r.z - p.x, 0.0, w - r.z - r.w);
        } else {
            path = left_start + clamp(h - r.w - p.y, 0.0, h - r.w - r.x);
        }
    }
    let behind = fract((settings.x - path / perimeter) * params.slots[3].x) * perimeter;
    let tail = max(min(settings.y, perimeter * 0.95), 0.001);
    let fade = max(1.0 - behind / tail, 0.0);
    var light = fade * fade * smoothstep(0.0, min(2.0, tail * 0.1), behind);
    var color_position = clamp(behind / tail, 0.0, 1.0);
    if params.slots[3].y > 0.0 {
        color_position = path / perimeter;
        light = mix(params.slots[3].w, 1.0, light);
    }
    let color = trail_color(input, params, color_position);
    let coverage = 1.0 - smoothstep(-0.5, 0.5, sdf);
    return vec4<f32>(color.rgb, color.a * settings.w * edge * light * coverage);
}
