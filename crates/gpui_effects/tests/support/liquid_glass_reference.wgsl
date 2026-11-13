// Copyright (c) 2024 Charles Yin. MIT license: ../../src/surface/shaders/LICENSE-liquid-glass-studio.
// Main glass effect composition shader for WebGPU
// Ported from fragment-main.glsl (STEP==9 branch only)

const PI: f32 = 3.14159265359;
const N_R: f32 = 0.98; // 1.0 - 0.02
const N_G: f32 = 1.0;
const N_B: f32 = 1.02; // 1.0 + 0.02

struct Uniforms {
  u_resolution: vec2f,
  u_dpr: f32,
  _pad0: f32,
  u_mouse: vec2f,
  u_mouseSpring: vec2f,
  u_shapeWidth: f32,
  u_shapeHeight: f32,
  u_shapeRadius: f32,
  u_shapeRoundness: f32,
  u_mergeRate: f32,
  u_glareAngle: f32,
  u_shadowExpand: f32,
  u_shadowFactor: f32,
  u_shadowPosition: vec2f,
  u_bgTextureRatio: f32,
  u_bgType: i32,
  u_bgTextureReady: i32,
  u_showShape1: i32,
  u_blurRadius: i32,
  u_blurEdge: i32,
  u_tint: vec4f,
  u_refThickness: f32,
  u_refFactor: f32,
  u_refDispersion: f32,
  u_refFresnelRange: f32,
  u_refFresnelHardness: f32,
  u_refFresnelFactor: f32,
  u_glareRange: f32,
  u_glareHardness: f32,
  u_glareConvergence: f32,
  u_glareOppositeFactor: f32,
  u_glareFactor: f32,
  u_refDistance: f32,
};

var<private> u: Uniforms;
var<private> reference_input: BackdropInput;
var<private> reference_circle_radius: f32;




// Signed distance functions shared by the background and main passes.
// `#include`d into a WGSL module that already declares the `Uniforms` struct
// and the `@group(0) @binding(0) var<uniform> u: Uniforms;` binding.
// Mirrors src/shaders/lib/sdf.glsl for the WebGPU backend.

fn sdCircle(p: vec2f, r: f32) -> f32 {
  return length(p) - r;
}

fn superellipseCornerSDF(p_in: vec2f, r: f32, n: f32) -> f32 {
  let p = abs(p_in);
  let v = pow(pow(p.x, n) + pow(p.y, n), 1.0 / n);
  return v - r;
}

fn roundedRectSDF(p_in: vec2f, center: vec2f, width: f32, height: f32, cornerRadius: f32, n: f32) -> f32 {
  let p = p_in - center;
  let cr = cornerRadius * u.u_dpr;
  let d = abs(p) - vec2f(width * u.u_dpr, height * u.u_dpr) * 0.5;
  var dist: f32;
  if (d.x > -cr && d.y > -cr) {
    let cornerCenter = sign(p) * (vec2f(width * u.u_dpr, height * u.u_dpr) * 0.5 - vec2f(cr));
    let cornerP = p - cornerCenter;
    dist = superellipseCornerSDF(cornerP, cr, n);
  } else {
    dist = min(max(d.x, d.y), 0.0) + length(max(d, vec2f(0.0)));
  }
  return dist;
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
  return mix(b, a, h) - k * h * (1.0 - h);
}

fn mainSDF(p1: vec2f, p2: vec2f, p: vec2f) -> f32 {
  let p1n = p1 + p / u.u_resolution.y;
  let p2n = p2 + p / u.u_resolution.y;
  var d1: f32;
  if (u.u_showShape1 == 1) {
    d1 = sdCircle(p1n, reference_circle_radius / u.u_resolution.y);
  } else {
    d1 = 1.0;
  }
  let d2 = roundedRectSDF(
    p2n, vec2f(0.0),
    u.u_shapeWidth / u.u_resolution.y,
    u.u_shapeHeight / u.u_resolution.y,
    u.u_shapeRadius / u.u_resolution.y,
    u.u_shapeRoundness
  );
  return smin(d1, d2, u.u_mergeRate);
}

// Math helpers shared across shader passes.
// Mirrors src/shaders/lib/math.glsl for the WebGPU backend.

// asin with domain-clamped input. asin(x) returns NaN for |x| > 1 in WGSL,
// which corrupts the refraction edge just outside the shape where
// x_R_ratio > 1. Clamping keeps the result well-defined.
fn safeAsin(x: f32) -> f32 {
  return asin(clamp(x, -1.0, 1.0));
}


fn getNormal(p1: vec2f, p2: vec2f, p: vec2f) -> vec2f {
  // dFdx(gl_FragCoord.x) = 1.0 on a fullscreen quad, so eps = 1.0
  let h = vec2f(1.0, 1.0);
  let grad = vec2f(
    mainSDF(p1, p2, p + vec2f(h.x, 0.0)) - mainSDF(p1, p2, p - vec2f(h.x, 0.0)),
    mainSDF(p1, p2, p + vec2f(0.0, h.y)) - mainSDF(p1, p2, p - vec2f(0.0, h.y))
  ) / (2.0 * h);
  return grad * 1.414213562 * 1000.0;
}

// Safe normalize: returns zero vector instead of NaN when length is near zero
fn safeNormalize(v: vec2f) -> vec2f {
  let len = length(v);
  if (len < 1e-8) {
    return vec2f(0.0);
  }
  return v / len;
}

// ---- Color space functions (sRGB <-> LCH via Lab) ----
// Pure helper functions — meant to be `#include`d into a WGSL shader module.
// Mirrors src/shaders/lib/color.glsl for the WebGPU backend.

const D65_WHITE: vec3f = vec3f(0.95045592705, 1.0, 1.08905775076);

// GLSL mat3 is column-major. For `v * M`, result[j] = dot(v, column[j]).
// GLSL: mat3(col0.x,col0.y,col0.z, col1.x,col1.y,col1.z, col2.x,col2.y,col2.z)
// RGB_TO_XYZ_M columns:
const RGB_TO_XYZ_M_COL0: vec3f = vec3f(0.4124, 0.3576, 0.1805);
const RGB_TO_XYZ_M_COL1: vec3f = vec3f(0.2126, 0.7152, 0.0722);
const RGB_TO_XYZ_M_COL2: vec3f = vec3f(0.0193, 0.1192, 0.9505);

// XYZ_TO_RGB_M columns:
const XYZ_TO_RGB_M_COL0: vec3f = vec3f(3.2406255, -1.537208, -0.4986286);
const XYZ_TO_RGB_M_COL1: vec3f = vec3f(-0.9689307, 1.8757561, 0.0415175);
const XYZ_TO_RGB_M_COL2: vec3f = vec3f(0.0557101, -0.2040211, 1.0569959);

fn UNCOMPAND_SRGB(a: f32) -> f32 {
  if (a > 0.04045) {
    return pow((a + 0.055) / 1.055, 2.4);
  }
  return a / 12.92;
}

fn COMPAND_RGB(a: f32) -> f32 {
  if (a <= 0.0031308) {
    return 12.92 * a;
  }
  return 1.055 * pow(a, 0.41666666666) - 0.055;
}

fn SRGB_TO_RGB(srgb: vec3f) -> vec3f {
  return vec3f(UNCOMPAND_SRGB(srgb.x), UNCOMPAND_SRGB(srgb.y), UNCOMPAND_SRGB(srgb.z));
}

fn RGB_TO_SRGB(rgb: vec3f) -> vec3f {
  return vec3f(COMPAND_RGB(rgb.x), COMPAND_RGB(rgb.y), COMPAND_RGB(rgb.z));
}

fn RGB_TO_XYZ(rgb: vec3f) -> vec3f {
  // Matrix multiply using column vectors (GLSL mat3 is column-major)
  return vec3f(
    dot(rgb, RGB_TO_XYZ_M_COL0),
    dot(rgb, RGB_TO_XYZ_M_COL1),
    dot(rgb, RGB_TO_XYZ_M_COL2)
  );
}

fn XYZ_TO_RGB(xyz: vec3f) -> vec3f {
  return vec3f(
    dot(xyz, XYZ_TO_RGB_M_COL0),
    dot(xyz, XYZ_TO_RGB_M_COL1),
    dot(xyz, XYZ_TO_RGB_M_COL2)
  );
}

fn SRGB_TO_XYZ(srgb: vec3f) -> vec3f {
  return RGB_TO_XYZ(SRGB_TO_RGB(srgb));
}

fn XYZ_TO_SRGB(xyz: vec3f) -> vec3f {
  return RGB_TO_SRGB(XYZ_TO_RGB(xyz));
}

fn XYZ_TO_LAB_F(x: f32) -> f32 {
  if (x > 0.00885645167) {
    return pow(x, 0.333333333);
  }
  return 7.78703703704 * x + 0.13793103448;
}

fn XYZ_TO_LAB(xyz: vec3f) -> vec3f {
  let xyz_scaled = vec3f(
    XYZ_TO_LAB_F(xyz.x / D65_WHITE.x),
    XYZ_TO_LAB_F(xyz.y / D65_WHITE.y),
    XYZ_TO_LAB_F(xyz.z / D65_WHITE.z)
  );
  return vec3f(
    116.0 * xyz_scaled.y - 16.0,
    500.0 * (xyz_scaled.x - xyz_scaled.y),
    200.0 * (xyz_scaled.y - xyz_scaled.z)
  );
}

fn SRGB_TO_LAB(srgb: vec3f) -> vec3f {
  return XYZ_TO_LAB(SRGB_TO_XYZ(srgb));
}

fn LAB_TO_LCH(Lab: vec3f) -> vec3f {
  return vec3f(Lab.x, sqrt(dot(Lab.yz, Lab.yz)), atan2(Lab.z, Lab.y) * 57.2957795131);
}

fn SRGB_TO_LCH(srgb: vec3f) -> vec3f {
  return LAB_TO_LCH(SRGB_TO_LAB(srgb));
}

fn LAB_TO_XYZ_F(x: f32) -> f32 {
  if (x > 0.206897) {
    return x * x * x;
  }
  return 0.12841854934 * (x - 0.137931034);
}

fn LAB_TO_XYZ(Lab: vec3f) -> vec3f {
  let w = (Lab.x + 16.0) / 116.0;
  return D65_WHITE * vec3f(LAB_TO_XYZ_F(w + Lab.y / 500.0), LAB_TO_XYZ_F(w), LAB_TO_XYZ_F(w - Lab.z / 200.0));
}

fn LAB_TO_SRGB(lab: vec3f) -> vec3f {
  return XYZ_TO_SRGB(LAB_TO_XYZ(lab));
}

fn LCH_TO_LAB(LCh: vec3f) -> vec3f {
  return vec3f(LCh.x, LCh.y * cos(LCh.z * 0.01745329251), LCh.y * sin(LCh.z * 0.01745329251));
}

fn LCH_TO_SRGB(lch: vec3f) -> vec3f {
  return LAB_TO_SRGB(LCH_TO_LAB(lch));
}


fn vec2ToAngle(v: vec2f) -> f32 {
  var angle = atan2(v.y, v.x);
  if (angle < 0.0) { angle += 2.0 * PI; }
  return angle;
}

// ---- Texture dispersion ----

fn getTextureDispersion(v_uv: vec2f, mixRate: f32, offset: vec2f, factor: f32) -> vec4f {
  var pixel = vec4f(1.0);

  let bgR = reference_raw(v_uv + offset * (1.0 - (N_R - 1.0) * factor), 0.0).r;
  let bgG = reference_raw(v_uv + offset * (1.0 - (N_G - 1.0) * factor), 0.0).g;
  let bgB = reference_raw(v_uv + offset * (1.0 - (N_B - 1.0) * factor), 0.0).b;

  let blurR = reference_blur(v_uv + offset * (1.0 - (N_R - 1.0) * factor), 0.0).r;
  let blurG = reference_blur(v_uv + offset * (1.0 - (N_G - 1.0) * factor), 0.0).g;
  let blurB = reference_blur(v_uv + offset * (1.0 - (N_B - 1.0) * factor), 0.0).b;

  pixel.r = mix(bgR, blurR, mixRate);
  pixel.g = mix(bgG, blurG, mixRate);
  pixel.b = mix(bgB, blurB, mixRate);

  return pixel;
}

// ---- Main fragment shader (STEP==9 equivalent) ----

fn reference_main(frag_coord: vec4f, v_uv: vec2f) -> vec4f {
  let u_resolution1x = u.u_resolution / u.u_dpr;

  // WebGPU frag_coord.y is top-down, flip to match GLSL bottom-up convention
  let pixel = vec2f(frag_coord.x, u.u_resolution.y - frag_coord.y);

  // center of shape 1
  let p1 = -u.u_mouse / u.u_resolution.y;
  // center of shape 2
  let p2 = (vec2f(0.0) - u.u_mouseSpring) / u.u_resolution.y;
  // merged shape
  let merged = mainSDF(p1, p2, pixel);

  var outColor: vec4f;

  if (merged < 0.005) {
    let nmerged = -1.0 * (merged * u_resolution1x.y);

    // calculate refraction edge factor
    let x_R_ratio = 1.0 - nmerged / u.u_refThickness;
    let thetaI = safeAsin(pow(x_R_ratio, 2.0));
    let thetaT = safeAsin(1.0 / u.u_refFactor * sin(thetaI));
    var edgeFactor = -1.0 * tan(thetaT - thetaI);
    if (nmerged >= u.u_refThickness) {
      edgeFactor = 0.0;
    }

    if (edgeFactor <= 0.0) {
      outColor = reference_blur(v_uv, 0.0);
      outColor = mix(outColor, vec4f(u.u_tint.r, u.u_tint.g, u.u_tint.b, 1.0), u.u_tint.a * 0.8);
    } else {
      let edgeH = nmerged / u.u_refThickness;
      let normal = getNormal(p1, p2, pixel);

      var blurMixRate: f32;
      if (u.u_blurEdge > 0) {
        blurMixRate = 1.0;
      } else {
        blurMixRate = edgeH;
      }

      // Normal is in GLSL bottom-up coords (Y up), but v_uv.y is top-down (Y down).
      // Flip the Y component of the offset to match v_uv orientation.
      let refOffset = -normal * edgeFactor * u.u_refDistance * u.u_dpr * vec2f(
        u.u_resolution.y / (u_resolution1x.x * u.u_dpr),
        1.0
      );
      let blurredPixel = getTextureDispersion(
        v_uv,
        blurMixRate,
        vec2f(refOffset.x, -refOffset.y),
        u.u_refDispersion
      );

      // basic tint
      outColor = mix(blurredPixel, vec4f(u.u_tint.r, u.u_tint.g, u.u_tint.b, 1.0), u.u_tint.a * 0.8);

      // add fresnel
      let fresnelFactor = clamp(
        pow(
          1.0 + merged * u_resolution1x.y / 1500.0 * pow(500.0 / u.u_refFresnelRange, 2.0) + u.u_refFresnelHardness,
          5.0
        ),
        0.0, 1.0
      );

      var fresnelTintLCH = SRGB_TO_LCH(
        mix(vec3f(1.0), vec3f(u.u_tint.r, u.u_tint.g, u.u_tint.b), u.u_tint.a * 0.5)
      );
      fresnelTintLCH.x += 20.0 * fresnelFactor * u.u_refFresnelFactor;
      fresnelTintLCH.x = clamp(fresnelTintLCH.x, 0.0, 100.0);

      outColor = mix(
        outColor,
        vec4f(LCH_TO_SRGB(fresnelTintLCH), 1.0),
        fresnelFactor * u.u_refFresnelFactor * 0.7 * length(normal)
      );

      // add glare
      let glareGeoFactor = clamp(
        pow(
          1.0 + merged * u_resolution1x.y / 1500.0 * pow(500.0 / u.u_glareRange, 2.0) + u.u_glareHardness,
          5.0
        ),
        0.0, 1.0
      );

      let glareAngle = (vec2ToAngle(safeNormalize(normal)) - PI / 4.0 + u.u_glareAngle) * 2.0;
      var glareFarside: i32 = 0;
      if ((glareAngle > PI * (2.0 - 0.5) && glareAngle < PI * (4.0 - 0.5)) || glareAngle < PI * (0.0 - 0.5)) {
        glareFarside = 1;
      }

      var glareSideFactor: f32;
      if (glareFarside == 1) {
        glareSideFactor = 1.2 * u.u_glareOppositeFactor;
      } else {
        glareSideFactor = 1.2;
      }

      var glareAngleFactor = (0.5 + sin(glareAngle) * 0.5) * glareSideFactor * u.u_glareFactor;
      glareAngleFactor = clamp(pow(glareAngleFactor, 0.1 + u.u_glareConvergence * 2.0), 0.0, 1.0);

      var glareTintLCH = SRGB_TO_LCH(
        mix(blurredPixel.rgb, vec3f(u.u_tint.r, u.u_tint.g, u.u_tint.b), u.u_tint.a * 0.5)
      );
      glareTintLCH.x += 150.0 * glareAngleFactor * glareGeoFactor;
      glareTintLCH.y += 30.0 * glareAngleFactor * glareGeoFactor;
      glareTintLCH.x = clamp(glareTintLCH.x, 0.0, 120.0);

      outColor = mix(
        outColor,
        vec4f(LCH_TO_SRGB(glareTintLCH), 1.0),
        glareAngleFactor * glareGeoFactor * length(normal)
      );
    }
  } else {
    outColor = reference_raw(v_uv, 0.0);
  }

  // smooth edge transition
  outColor = mix(outColor, reference_raw(v_uv, 0.0), smoothstep(-0.001, 0.001, merged));

  return outColor;
}


fn reference_raw(uv: vec2f, level: f32) -> vec4f {
    let color = sample_raw_backdrop(reference_input, uv * u.u_resolution - reference_input.position);
    return vec4f(backdrop_to_srgb(color.rgb), color.a);
}

fn reference_blur(uv: vec2f, level: f32) -> vec4f {
    let color = sample_blurred_backdrop(reference_input, uv * u.u_resolution - reference_input.position);
    return vec4f(backdrop_to_srgb(color.rgb), color.a);
}

fn backdrop_effect(input: BackdropInput, params: BackdropParams) -> vec4f {
    reference_input = input;
    u.u_resolution = globals.viewport_size;
    u.u_mouse = vec2f(params.slots[3].w + params.slots[12].x, u.u_resolution.y - params.slots[6].w - params.slots[12].y);
    reference_circle_radius = params.slots[12].z;
    u.u_dpr = params.slots[10].w;
    u.u_mouseSpring = vec2f(params.slots[3].w, u.u_resolution.y - params.slots[6].w);
    u.u_shapeWidth = (params.slots[6].x) / u.u_dpr;
    u.u_shapeHeight = (params.slots[6].y) / u.u_dpr;
    u.u_shapeRadius = (params.slots[4].x) / u.u_dpr;
    u.u_shapeRoundness = params.slots[11].x;
    u.u_mergeRate = params.slots[12].w / u.u_resolution.y;
    u.u_showShape1 = select(0, 1, params.slots[12].z > 0.0);
    u.u_tint = params.slots[1];
    u.u_refThickness = (params.slots[0].w) / u.u_dpr;
    u.u_refFactor = params.slots[8].x;
    u.u_refDistance = (params.slots[0].z / 1414.213562) / u.u_dpr;
    u.u_refDispersion = params.slots[2].z / 0.02;
    u.u_refFresnelRange = (params.slots[8].y) / u.u_dpr;
    u.u_refFresnelHardness = params.slots[8].z;
    u.u_refFresnelFactor = params.slots[8].w;
    u.u_glareRange = (params.slots[9].x) / u.u_dpr;
    u.u_glareHardness = params.slots[9].y;
    u.u_glareFactor = params.slots[9].z;
    u.u_glareConvergence = params.slots[9].w;
    u.u_glareOppositeFactor = params.slots[10].x;
    u.u_glareAngle = params.slots[10].y;
    u.u_blurEdge = i32(params.slots[10].z);
    let color = reference_main(vec4f(input.position, 0.0, 1.0), input.position / u.u_resolution);
    return vec4f(backdrop_from_srgb(clamp(color.rgb, vec3f(0.0), vec3f(1.0))), color.a);
}
