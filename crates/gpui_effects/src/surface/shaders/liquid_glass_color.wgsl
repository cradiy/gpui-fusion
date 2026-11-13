// Portions copyright (c) 2024 Charles Yin. MIT license: LICENSE-liquid-glass-studio.
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
