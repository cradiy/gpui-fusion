fn album_glow_weight(point: vec2<f32>, center: vec2<f32>, softness: f32) -> f32 {
    let delta = point - center;
    return exp(-dot(delta, delta) * softness);
}

fn album_glow_color(input: EffectInput, uv: vec2<f32>) -> vec3<f32> {
    return sample_effect_image(input, clamp(uv, vec2<f32>(0.03), vec2<f32>(0.97))).rgb;
}

fn album_glow_hash(seed: f32) -> f32 {
    return fract(sin(seed * 91.173 + 17.17) * 43758.5453);
}

fn album_glow_flow_center(
    time: f32,
    seed: f32,
    motion: f32,
) -> vec2<f32> {
    // Every palette sample gets a different mixture of slow drift, medium
    // transport, and a small faster eddy. The frequencies are deliberately
    // incommensurate, so the paths keep evolving instead of tracing a visible
    // orbit or returning together at the end of a fixed animation cycle.
    let pace = album_glow_hash(seed + 1.0);
    let slow_speed = mix(0.018, 0.095, pace * pace);
    let medium_speed = mix(0.11, 0.42, album_glow_hash(seed + 2.0));
    let fast_speed = mix(0.38, 1.10, album_glow_hash(seed + 3.0));
    let base = vec2<f32>(
        mix(0.14, 0.86, album_glow_hash(seed + 4.0)),
        mix(0.12, 0.88, album_glow_hash(seed + 5.0)),
    );
    let slow = vec2<f32>(
        sin(time * slow_speed + seed * 1.31),
        cos(time * slow_speed * 0.731 + seed * 0.83),
    );
    let medium = vec2<f32>(
        sin(time * medium_speed * 0.619 + seed * 2.17),
        cos(time * medium_speed * 0.877 + seed * 1.73),
    );
    let eddy = vec2<f32>(
        sin(time * fast_speed + seed * 3.11),
        cos(time * fast_speed * 0.787 + seed * 2.43),
    );
    let fast_mix = smoothstep(0.38, 0.88, album_glow_hash(seed + 6.0));
    // Only part of the palette receives a large fast eddy. The rest stays on
    // the slow path, making the velocity difference visible without shaking
    // the complete background at one uniform speed.
    let eddy_amount = mix(0.010, 0.28, fast_mix);
    return base + (slow * 0.27 + medium * 0.13 + eddy * eddy_amount) * motion;
}

fn album_glow_flow_warp(uv: vec2<f32>, time: f32, motion: f32) -> vec2<f32> {
    let point = uv - vec2<f32>(0.5);
    let slow = vec2<f32>(
        sin(point.y * 4.17 + time * 0.027
            + sin(point.x * 2.73 - time * 0.019) * 0.82),
        cos(point.x * 3.61 - time * 0.033
            + sin(point.y * 3.17 + time * 0.023) * 0.74),
    );
    let medium = vec2<f32>(
        sin((point.x - point.y) * 3.23 + time * 0.091),
        cos((point.x + point.y) * 2.81 - time * 0.073),
    );
    let fast = vec2<f32>(
        sin(point.y * 7.13 - time * 0.213 + sin(point.x * 4.37) * 0.35),
        cos(point.x * 6.47 + time * 0.167 + sin(point.y * 4.91) * 0.31),
    );
    return uv + (slow * 0.065 + medium * 0.060 + fast * 0.070) * motion;
}

fn album_drop_wave(
    point: vec2<f32>,
    origin: vec2<f32>,
    aspect: f32,
    progress: f32,
    max_radius: f32,
    width: f32,
) -> vec4<f32> {
    let delta = point - origin;
    let metric_delta = delta * vec2<f32>(aspect, 1.0);
    let distance = max(length(metric_delta), 0.0001);
    let radius = progress * max_radius;
    let signed_distance = (distance - radius) / max(width, 0.0001);
    let life = sin(progress * 3.14159265359);
    let crest = exp(-signed_distance * signed_distance) * life;
    let trough_distance = signed_distance + 1.25;
    let trough = exp(-trough_distance * trough_distance * 1.25) * life;
    let displacement = signed_distance * crest;
    let direction = metric_delta / distance / vec2<f32>(aspect, 1.0);
    let halo = exp(-signed_distance * signed_distance * 0.28) * life;
    return vec4<f32>(direction * displacement, crest - trough * 0.42, halo);
}

fn effect(input: EffectInput, params: EffectParams) -> vec4<f32> {
    let tau = 6.28318530718;
    let blur = max(params.slots[0].x, 0.0);
    let saturation = max(params.slots[0].y, 0.0);
    let brightness = max(params.slots[0].z, 0.0);
    let motion = max(params.slots[0].w, 0.0);
    let glow_strength = max(params.slots[1].x, 0.0);
    let ripple_displacement = max(params.slots[1].y, 0.0);
    let ripple_light = max(params.slots[1].z, 0.0);
    let ring_definition = clamp(params.slots[1].w, 0.0, 1.0);
    let diffusion = clamp(blur / 0.18, 0.0, 1.0);
    let ripple_phase = input.time * tau;

    // These nine stratified samples cover equal parts of the artwork. Giving
    // every sample one vote preserves the cover's approximate color ratio:
    // repeated dominant colors occupy more of the field, while a small accent
    // remains an accent instead of being promoted by saturation.
    let palette = array<vec3<f32>, 9>(
        album_glow_color(input, vec2<f32>(0.16, 0.16)),
        album_glow_color(input, vec2<f32>(0.50, 0.14)),
        album_glow_color(input, vec2<f32>(0.84, 0.18)),
        album_glow_color(input, vec2<f32>(0.14, 0.50)),
        album_glow_color(input, vec2<f32>(0.50, 0.50)),
        album_glow_color(input, vec2<f32>(0.86, 0.50)),
        album_glow_color(input, vec2<f32>(0.18, 0.84)),
        album_glow_color(input, vec2<f32>(0.50, 0.86)),
        album_glow_color(input, vec2<f32>(0.82, 0.82)),
    );

    let flowing_point = album_glow_flow_warp(input.uv, input.time, motion);
    var palette_color = vec3<f32>(0.0);
    var flow_color = vec3<f32>(0.0);
    var flow_total = 0.0;
    var local_total = 0.0;
    for (var index: u32 = 0u; index < 9u; index = index + 1u) {
        let seed = 2.7 + f32(index) * 17.173;
        let center = album_glow_flow_center(input.time, seed, motion);
        let softness = mix(8.0, 17.0, album_glow_hash(seed + 7.0));
        let local = album_glow_weight(flowing_point, center, softness);
        let secondary_center = album_glow_flow_center(input.time, seed + 71.9, motion * 0.74);
        let secondary = album_glow_weight(flowing_point, secondary_center, softness * 0.48);
        // A small global contribution prevents hard territories. Two broad
        // moving lobes then let the same color reappear elsewhere and mingle
        // with its neighbors rather than travelling as one isolated blob.
        let weight = 0.004 + local * 0.82 + secondary * 0.24;
        palette_color += palette[index];
        flow_color += palette[index] * weight;
        flow_total += weight;
        local_total += local + secondary * 0.5;
    }
    palette_color /= 9.0;
    var glow_rgb = flow_color / max(flow_total, 0.0001);
    let glow_density = smoothstep(1.4, 5.8, local_total);
    glow_rgb += mix(palette_color, glow_rgb, 0.65)
        * glow_density
        * glow_strength
        * 0.10;

    // Three staggered drops produce one local wavefront each. Distances are
    // measured in element-height units, preserving a circular wave on a wide card.
    let aspect = input.size.x / max(input.size.y, 1.0);
    let ripple_origin0 = vec2<f32>(0.20, 0.44) +
        vec2<f32>(cos(ripple_phase), sin(ripple_phase)) * vec2<f32>(0.025, 0.02) * motion;
    let ripple_origin1 = vec2<f32>(0.78, 0.58) +
        vec2<f32>(cos(-ripple_phase + 1.8), sin(-ripple_phase + 1.8)) * vec2<f32>(0.022, 0.018) * motion;
    let ripple_origin2 = vec2<f32>(0.50, 0.34) +
        vec2<f32>(cos(ripple_phase + 3.6), sin(ripple_phase + 3.6)) * vec2<f32>(0.024, 0.018) * motion;
    let max_ripple_radius = mix(1.55, 1.2 + diffusion * 0.25, ring_definition);
    let diffuse_width = 0.22 + diffusion * 0.16;
    let defined_width = 0.10 + diffusion * 0.08;
    let ripple_width = mix(diffuse_width, defined_width, ring_definition);
    let ripple0 = album_drop_wave(
        input.uv,
        ripple_origin0,
        aspect,
        fract(input.time),
        max_ripple_radius,
        ripple_width,
    );
    let ripple1 = album_drop_wave(
        input.uv,
        ripple_origin1,
        aspect,
        fract(input.time + 0.3333333),
        max_ripple_radius,
        ripple_width * 1.06,
    );
    let ripple2 = album_drop_wave(
        input.uv,
        ripple_origin2,
        aspect,
        fract(input.time + 0.6666667),
        max_ripple_radius,
        ripple_width * 0.96,
    );
    let point = input.uv + (ripple0.xy + ripple1.xy + ripple2.xy)
        * ripple_displacement
        * motion;
    let center0 = vec2<f32>(-0.04, 0.08) +
        vec2<f32>(cos(ripple_phase + 0.2), sin(ripple_phase + 0.2)) * vec2<f32>(0.13, 0.09) * motion;
    let center1 = vec2<f32>(0.82, -0.08) +
        vec2<f32>(cos(ripple_phase + 1.4), sin(ripple_phase + 1.4)) * vec2<f32>(0.12, 0.10) * motion;
    let center2 = vec2<f32>(-0.06, 0.78) +
        vec2<f32>(cos(ripple_phase + 2.6), sin(ripple_phase + 2.6)) * vec2<f32>(0.14, 0.09) * motion;
    let center3 = vec2<f32>(1.04, 0.62) +
        vec2<f32>(cos(ripple_phase + 3.8), sin(ripple_phase + 3.8)) * vec2<f32>(0.13, 0.11) * motion;
    let center4 = vec2<f32>(0.38, 0.42) +
        vec2<f32>(cos(-ripple_phase + 0.8), sin(-ripple_phase + 0.8)) * vec2<f32>(0.17, 0.13) * motion;
    let center5 = vec2<f32>(0.68, 1.04) +
        vec2<f32>(cos(-ripple_phase + 2.2), sin(-ripple_phase + 2.2)) * vec2<f32>(0.15, 0.10) * motion;

    let softness = mix(5.2, 2.4, diffusion);
    let weight0 = album_glow_weight(point, center0, softness);
    let weight1 = album_glow_weight(point, center1, softness);
    let weight2 = album_glow_weight(point, center2, softness);
    let weight3 = album_glow_weight(point, center3, softness);
    let weight4 = album_glow_weight(point, center4, softness);
    let weight5 = album_glow_weight(point, center5, softness);
    let total = max(weight0 + weight1 + weight2 + weight3 + weight4 + weight5, 0.0001);
    var ripple_rgb = (
        palette[0] * weight0
        + palette[2] * weight1
        + palette[3] * weight2
        + palette[5] * weight3
        + palette[6] * weight4
        + palette[8] * weight5
    ) / total;

    let glow_softness = softness * 2.5;
    let glow0 = album_glow_weight(point, center0, glow_softness);
    let glow1 = album_glow_weight(point, center1, glow_softness);
    let glow2 = album_glow_weight(point, center2, glow_softness);
    let glow3 = album_glow_weight(point, center3, glow_softness);
    let glow4 = album_glow_weight(point, center4, glow_softness);
    let glow5 = album_glow_weight(point, center5, glow_softness);
    let glow_total = max(glow0 + glow1 + glow2 + glow3 + glow4 + glow5, 0.0001);
    let glow_color = (
        palette[0] * glow0 +
        palette[2] * glow1 +
        palette[3] * glow2 +
        palette[5] * glow3 +
        palette[6] * glow4 +
        palette[8] * glow5
    ) / glow_total;
    let glow_mask = clamp(glow_total * 0.22, 0.0, 0.5);
    ripple_rgb += glow_color * glow_mask * glow_strength;

    // The crest is a single soft ring, while the wider halo gives it the diffuse
    // light of a drop spreading across calm water.
    let ripple_crest = ripple0.z + ripple1.z + ripple2.z;
    let ripple_halo = ripple0.w + ripple1.w + ripple2.w;
    ripple_rgb *= 1.0
        + ripple_crest * ripple_light * ring_definition * motion
        + ripple_halo
            * ripple_light
            * (1.0 - ring_definition)
            * 0.14
            * motion;

    var rgb = mix(glow_rgb, ripple_rgb, ring_definition);

    let luminance = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    rgb = mix(vec3<f32>(luminance), rgb, saturation) * brightness;
    let centered = input.uv - vec2<f32>(0.5);
    let vignette_point = centered * vec2<f32>(0.85, 1.15);
    let vignette = 1.0 - smoothstep(0.18, 0.82, dot(vignette_point, vignette_point));
    let lower_shade = mix(1.0, 0.72, smoothstep(0.35, 1.0, input.uv.y));
    let light_center = album_glow_flow_center(input.time, 113.7, motion * 0.45);
    let light_delta = (input.uv - light_center) * vec2<f32>(1.0, 1.35);
    let moving_light = exp(-dot(light_delta, light_delta) * 5.0);
    rgb *= mix(0.72, 1.0, vignette)
        * lower_shade
        * (0.94 + moving_light * 0.10 * motion);
    return vec4<f32>(max(rgb, vec3<f32>(0.0)), 1.0);
}
