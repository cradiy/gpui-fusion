use super::ProgressShape;
use crate::{
    components::selection_surface::{color, length},
    style::ComputedStyle,
};
use gpui::{
    Background, Bounds, Corners, LineCap, Path, PathBuilder, PathStyle, Pixels, StrokeOptions,
    Window, point, px, quad, size, transparent_black,
};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Default)]
pub(super) struct Paths {
    track: Option<CachedArc>,
    fill: Option<CachedArc>,
}
struct CachedArc {
    key: ArcKey,
    path: Path<Pixels>,
}
#[derive(Clone, Copy, PartialEq)]
struct ArcKey {
    bounds: Bounds<Pixels>,
    width: Pixels,
    start: f32,
    sweep: f32,
}
fn arc(cache: &mut Option<CachedArc>, key: ArcKey) -> Option<Path<Pixels>> {
    if key.sweep <= 0. || key.width <= px(0.) {
        return None;
    }
    if let Some(cached) = cache {
        if cached.key == key {
            return Some(cached.path.clone());
        }
    }
    let radius = (key.bounds.size.width.min(key.bounds.size.height) - key.width) * 0.5;
    if radius <= px(0.) {
        return None;
    }
    let point_at =
        |angle: f32| key.bounds.center() + point(radius * angle.cos(), radius * angle.sin());
    let mut builder = PathBuilder::stroke(key.width).with_style(PathStyle::Stroke(
        StrokeOptions::default()
            .with_line_width(key.width.as_f32())
            .with_line_cap(LineCap::Round),
    ));
    builder.move_to(point_at(key.start));
    let segments = (key.sweep / PI).ceil() as usize;
    for segment in 1..=segments {
        builder.arc_to(
            point(radius, radius),
            px(0.),
            false,
            true,
            point_at(key.start + key.sweep * segment as f32 / segments as f32),
        );
    }
    if key.sweep >= TAU {
        builder.close();
    }
    let path = builder.build().ok()?;
    *cache = Some(CachedArc {
        key,
        path: path.clone(),
    });
    Some(path)
}
fn background(style: &ComputedStyle) -> Background {
    style
        .background_gradient
        .clone()
        .unwrap_or_else(|| color(&style.background).into())
}
pub(super) struct Frame<'a> {
    pub bounds: Bounds<Pixels>,
    pub shape: ProgressShape,
    pub value: Option<f32>,
    pub phase: f32,
    pub track: &'a ComputedStyle,
    pub fill: &'a ComputedStyle,
}
impl Frame<'_> {
    pub(super) fn paint(self, paths: &mut Paths, window: &mut Window) {
        let Self {
            bounds,
            shape,
            value,
            phase,
            track,
            fill,
        } = self;
        let thickness = length(&track.height, 6.)
            .min(bounds.size.height)
            .min(bounds.size.width);
        if thickness <= px(0.) {
            return;
        }
        match shape {
            ProgressShape::Bar => {
                let track_bounds = Bounds::new(
                    point(bounds.left(), bounds.center().y - thickness * 0.5),
                    size(bounds.size.width, thickness),
                );
                let radius = length(&track.radius, thickness.as_f32() * 0.5).min(thickness * 0.5);
                let corners = Corners::all(radius);
                window.paint_quad(quad(
                    track_bounds,
                    corners,
                    background(track),
                    px(0.),
                    transparent_black(),
                    gpui::BorderStyle::Solid,
                ));
                let (start, end) = if let Some(value) = value {
                    (0., value)
                } else {
                    let center = (1. - (phase * TAU).cos()) * 0.5;
                    (center * 0.7, center * 0.7 + 0.3)
                };
                if end > start {
                    let fill_bounds = Bounds::new(
                        point(
                            track_bounds.left() + track_bounds.size.width * start,
                            track_bounds.top(),
                        ),
                        size(track_bounds.size.width * (end - start), thickness),
                    );
                    window.paint_quad(quad(
                        fill_bounds,
                        Corners::all(radius.min(fill_bounds.size.width * 0.5)),
                        background(fill),
                        px(0.),
                        transparent_black(),
                        gpui::BorderStyle::Solid,
                    ));
                }
                if let Some(mut glass) = track.material {
                    glass.shadow.strength = 0.;
                    gpui_effects::paint_liquid_glass(track_bounds, corners, glass, window);
                }
            }
            ProgressShape::Ring => {
                let diameter = bounds.size.width.min(bounds.size.height);
                let circle = Bounds::new(
                    bounds.center() - point(diameter * 0.5, diameter * 0.5),
                    size(diameter, diameter),
                );
                let inset = thickness;
                let ring = Bounds::new(
                    circle.origin + point(inset, inset),
                    size(
                        (diameter - inset * 2.).max(px(0.)),
                        (diameter - inset * 2.).max(px(0.)),
                    ),
                );
                let key = ArcKey {
                    bounds: ring,
                    width: thickness.min(ring.size.width * 0.5),
                    start: -FRAC_PI_2,
                    sweep: TAU,
                };
                if let Some(path) = arc(&mut paths.track, key) {
                    window.paint_path(path, background(track));
                }
                let start = if value.is_some() {
                    -FRAC_PI_2
                } else {
                    phase * TAU - FRAC_PI_2
                };
                let sweep = value.map_or_else(
                    || (0.18 + 0.22 * (1. - (phase * TAU).cos()) * 0.5) * TAU,
                    |v| v * TAU,
                );
                if let Some(path) = arc(
                    &mut paths.fill,
                    ArcKey {
                        start,
                        sweep,
                        ..key
                    },
                ) {
                    window.paint_path(path, background(fill));
                }
                if let Some(glass) = track.material {
                    gpui_effects::paint_liquid_glass_ring(ring, key.width, glass, window);
                }
            }
        }
    }
}
