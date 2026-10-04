fn aurora_band(distance: f32, width: f32) -> f32 {
    let normalized = distance / max(width, 0.0001);
    return exp(-normalized * normalized);
}

fn aurora_palette(position: f32, params: EffectParams) -> vec4<f32> {
    let cursor = fract(position) * 4.0;
    let distance0 = min(abs(cursor), 4.0 - abs(cursor));
    let distance1 = min(abs(cursor - 1.0), 4.0 - abs(cursor - 1.0));
    let distance2 = min(abs(cursor - 2.0), 4.0 - abs(cursor - 2.0));
    let distance3 = min(abs(cursor - 3.0), 4.0 - abs(cursor - 3.0));
    let weights = vec4<f32>(
        1.0 - smoothstep(0.0, 1.0, distance0),
        1.0 - smoothstep(0.0, 1.0, distance1),
        1.0 - smoothstep(0.0, 1.0, distance2),
        1.0 - smoothstep(0.0, 1.0, distance3),
    );
    let total = max(dot(weights, vec4<f32>(1.0)), 0.0001);
    return (
        params.slots[0] * weights.x +
        params.slots[1] * weights.y +
        params.slots[2] * weights.z +
        params.slots[3] * weights.w
    ) / total;
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let tau = 6.28318530718;
    let phase = input.time * tau;
    let scale = params.slots[4].x;
    let x = input.uv.x * scale;
    let y = input.uv.y;

    // Three independent curtains avoid the horizontal layers produced by a
    // conventional stacked gradient. Every displacement is analytic, so the
    // motion remains continuous without a noise grid becoming visible.
    let center0 = 0.42
        + sin(x * tau * 0.72 + phase) * 0.13
        + sin(x * tau * 1.85 - phase * 1.7) * 0.035;
    let center1 = 0.55
        + sin(x * tau * 0.58 - phase + 1.7) * 0.16
        + cos(x * tau * 2.2 + phase * 2.0) * 0.03;
    let center2 = 0.67
        + sin(x * tau * 0.91 + phase + 3.4) * 0.12
        + sin(x * tau * 2.65 - phase * 2.0) * 0.025;

    let folds0 = 0.72 + 0.28 * pow(0.5 + 0.5 * sin(x * tau * 8.0 + phase * 2.0), 2.0);
    let folds1 = 0.72 + 0.28 * pow(0.5 + 0.5 * sin(x * tau * 10.0 - phase), 2.0);
    let folds2 = 0.72 + 0.28 * pow(0.5 + 0.5 * cos(x * tau * 7.0 + phase), 2.0);
    let band0 = aurora_band(y - center0, 0.19) * folds0;
    let band1 = aurora_band(y - center1, 0.22) * folds1;
    let band2 = aurora_band(y - center2, 0.24) * folds2;

    // A wider, dimmer halo turns the ribbons into soft light curtains rather
    // than bright contour lines.
    let halo0 = aurora_band(y - center0, 0.38) * 0.22;
    let halo1 = aurora_band(y - center1, 0.43) * 0.19;
    let halo2 = aurora_band(y - center2, 0.46) * 0.17;
    let intensity0 = band0 + halo0;
    let intensity1 = band1 + halo1;
    let intensity2 = band2 + halo2;

    let color0 = aurora_palette(x * 0.23 + sin(phase) * 0.18, params);
    let color1 = aurora_palette(x * 0.19 + sin(-phase + 1.1) * 0.14 + 0.31, params);
    let color2 = aurora_palette(x * 0.16 + sin(phase + 0.7) * 0.11 + 0.62, params);
    let total = max(intensity0 + intensity1 + intensity2, 0.0001);
    let curtain = (
        color0 * intensity0 +
        color1 * intensity1 +
        color2 * intensity2
    ) / total;

    let background_color = aurora_palette(input.uv.x * 0.32 + sin(phase) * 0.06, params);
    let background = background_color.rgb * mix(0.16, 0.27, y);
    let strength = 1.0 - exp(-total * 0.92);
    let glow = 0.9 + 0.18 * clamp(total / 2.4, 0.0, 1.0);
    let rgb = pow(
        max(mix(background, curtain.rgb * glow, strength), vec3<f32>(0.0)),
        vec3<f32>(0.88),
    );
    let alpha = dot(vec4<f32>(
        params.slots[0].a,
        params.slots[1].a,
        params.slots[2].a,
        params.slots[3].a,
    ), vec4<f32>(0.25));
    return vec4<f32>(rgb, alpha);
}
