struct GlobalParams {
    viewport_size: vec2<f32>,
    premultiplied_alpha: u32,
    pad: u32,
}

struct Bounds {
    origin: vec2<f32>,
    size: vec2<f32>,
}

struct BackdropParams {
    slots: array<vec4<f32>, 8>,
}

struct BackdropInstance {
    bounds: Bounds,
    content_mask: Bounds,
    corner_radii: vec4<f32>,
    blur_radius: f32,
    opacity: f32,
    time: f32,
    pointer_active: f32,
    direction: vec2<f32>,
    pointer: vec2<f32>,
    uniforms: BackdropParams,
}

@group(0) @binding(0) var<uniform> globals: GlobalParams;
@group(1) @binding(0) var<storage, read> b_backdrops: array<BackdropInstance>;
@group(1) @binding(1) var t_backdrop: texture_2d<f32>;

struct BackdropVarying {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) instance_id: u32,
}

fn unit_vertex(vertex_id: u32) -> vec2<f32> {
    return vec2<f32>(f32(vertex_id & 1u), 0.5 * f32(vertex_id & 2u));
}

fn to_device_position(unit: vec2<f32>, bounds: Bounds) -> vec4<f32> {
    let position = unit * bounds.size + bounds.origin;
    let device = position / globals.viewport_size * vec2<f32>(2.0, -2.0)
        + vec2<f32>(-1.0, 1.0);
    return vec4<f32>(device, 0.0, 1.0);
}

@vertex
fn vs_backdrop(
    @builtin(vertex_index) vertex_id: u32,
    @builtin(instance_index) instance_id: u32,
) -> BackdropVarying {
    var out: BackdropVarying;
    out.position = to_device_position(unit_vertex(vertex_id), b_backdrops[instance_id].bounds);
    out.instance_id = instance_id;
    return out;
}

@fragment
fn fs_blur(input: BackdropVarying) -> @location(0) vec4<f32> {
    let instance = b_backdrops[input.instance_id];
    let uv = input.position.xy / max(instance.content_mask.size, vec2<f32>(1.0));
    let dimensions = vec2<f32>(textureDimensions(t_backdrop));
    let radius = max(instance.blur_radius, 0.0)
        * dot(abs(instance.direction), dimensions / max(globals.viewport_size, vec2<f32>(1.0)));
    if (radius <= 0.0) {
        return sample_backdrop(uv);
    }
    // Sample the whole Gaussian support in source texels. Pair adjacent taps
    // through bilinear filtering; scaling a sparse kernel repeats fine detail.
    let sigma = max(radius * 0.4375, 0.001);
    let limit = i32(ceil(radius));
    var color = vec4<f32>(0.0);
    var total_weight = 0.0;
    for (var tap = -limit; tap <= limit; tap += 2) {
        let first = f32(tap);
        let second = first + 1.0;
        let w0 = exp(-0.5 * first * first / (sigma * sigma));
        let w1 = select(0.0, exp(-0.5 * second * second / (sigma * sigma)), tap + 1 <= limit);
        let weight = w0 + w1;
        let offset = (first * w0 + second * w1) / max(weight, 0.000001);
        color += sample_backdrop(uv + instance.direction * offset / dimensions) * weight;
        total_weight += weight;
    }
    return color / max(total_weight, 0.000001);
}

fn corner_radius(point: vec2<f32>, radii: vec4<f32>) -> f32 {
    if (point.x < 0.0) {
        return select(radii.w, radii.x, point.y < 0.0);
    }
    return select(radii.z, radii.y, point.y < 0.0);
}

fn rounded_rect_distance(position: vec2<f32>, bounds: Bounds, radii: vec4<f32>) -> f32 {
    let half_size = bounds.size * 0.5;
    let centered = position - (bounds.origin + half_size);
    let radius = corner_radius(centered, radii);
    let corner = abs(centered) - half_size + radius;
    return length(max(corner, vec2<f32>(0.0)))
        + min(max(corner.x, corner.y), 0.0)
        - radius;
}

@fragment
fn fs_backdrop(input: BackdropVarying) -> @location(0) vec4<f32> {
    let instance = b_backdrops[input.instance_id];
    let position = input.position.xy;
    let mask_end = instance.content_mask.origin + instance.content_mask.size;
    if (any(position < instance.content_mask.origin) || any(position > mask_end)) {
        discard;
    }

    let distance = rounded_rect_distance(position, instance.bounds, instance.corner_radii);
    let coverage = 1.0 - smoothstep(-0.5, 0.5, distance);
    let factor = coverage * instance.opacity;
    let sampled = sample_backdrop(
        position / max(globals.viewport_size, vec2<f32>(1.0)),
    );

    if (globals.premultiplied_alpha != 0u) {
        return vec4<f32>(sampled.rgb * factor, sampled.a * factor);
    }
    return vec4<f32>(sampled.rgb, sampled.a * factor);
}
