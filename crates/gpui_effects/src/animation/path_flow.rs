use std::sync::Arc;

use anyhow::{Result, ensure};
use gpui::{
    Bounds, Div, PathBuilder, Pixels, Point, Rgba, Size, div, fill, linear_color_stop,
    linear_gradient, point, px, rgb, size,
};

/// A polyline in normalized surface coordinates. Consecutive duplicate points are allowed.
/// Progress follows physical distance after scaling to the surface, including unequal aspect ratios.
#[derive(Clone, Debug)]
pub struct FlowPath(Arc<[Point<f32>]>);

impl FlowPath {
    /// Requires 2 through 4096 finite points in `0..=1`, with at least one nonzero segment.
    pub fn new(points: impl IntoIterator<Item = Point<f32>>) -> Result<Self> {
        let points: Vec<_> = points.into_iter().take(4097).collect();
        ensure!(
            (2..=4096).contains(&points.len()),
            "a flow path requires 2 through 4096 points"
        );
        ensure!(
            points
                .iter()
                .all(|p| (0. ..=1.).contains(&p.x) && (0. ..=1.).contains(&p.y)),
            "flow path coordinates must be finite and normalized"
        );
        ensure!(
            points.windows(2).any(|pair| pair[0] != pair[1]),
            "a flow path requires a nonzero segment"
        );
        Ok(Self(points.into()))
    }

    /// Returns a surface-local position. Progress is clamped to `0..=1`; nonfinite progress is zero.
    pub fn position(&self, progress: f32, extent: Size<Pixels>) -> Point<Pixels> {
        let scale = |p: Point<f32>| point(extent.width * p.x, extent.height * p.y);
        let length = |pair: &[Point<f32>]| {
            let delta = scale(pair[1]) - scale(pair[0]);
            f32::from(delta.x).hypot(f32::from(delta.y))
        };
        let total: f32 = self.0.windows(2).map(length).sum();
        if !total.is_finite() {
            return point(px(0.), px(0.));
        }
        let progress = if progress.is_finite() {
            progress.clamp(0., 1.)
        } else {
            0.
        };
        let mut remaining = total * progress;
        for pair in self.0.windows(2) {
            let segment = length(pair);
            if segment > 0. && remaining <= segment {
                return scale(pair[0]) + (scale(pair[1]) - scale(pair[0])) * (remaining / segment);
            }
            remaining -= segment;
        }
        scale(*self.0.last().unwrap())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PathFlowOptions {
    pub track_color: Rgba,
    pub track_width: Pixels,
    pub color: Rgba,
    /// Half-width of the light at its head.
    pub radius: Pixels,
    /// Fading trail length as a fraction of the complete route, clamped to `0..=1`.
    pub trail: f32,
}

impl Default for PathFlowOptions {
    fn default() -> Self {
        Self {
            track_color: Rgba {
                a: 0.35,
                ..rgb(0x455268)
            },
            track_width: px(1.),
            color: rgb(0x83d8ef),
            radius: px(1.8),
            trail: 0.14,
        }
    }
}

/// Draws a route and a traveling light beneath caller-provided children.
/// The caller owns progress and playback; this element never requests animation frames.
/// Use the same progress for associated node states, and freeze it for reduced motion.
pub fn path_flow(path: FlowPath, progress: f32, options: PathFlowOptions) -> Div {
    div().on_paint_before_children(move |bounds, _, window, _| {
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return;
        }
        let width = f32::from(options.track_width);
        if width.is_finite() && width > 0. {
            let mut track = PathBuilder::stroke(px(width));
            for (index, p) in path.0.iter().enumerate() {
                let position =
                    bounds.origin + point(bounds.size.width * p.x, bounds.size.height * p.y);
                if index == 0 {
                    track.move_to(position);
                } else {
                    track.line_to(position);
                }
            }
            if let Ok(track) = track.build() {
                window.paint_path(track, options.track_color);
            }
        }
        let radius = f32::from(options.radius);
        if !radius.is_finite() || radius <= 0. {
            return;
        }
        let progress = if progress.is_finite() {
            progress.clamp(0., 1.)
        } else {
            0.
        };
        let trail = if options.trail.is_finite() {
            options.trail.clamp(0., 1.)
        } else {
            0.
        };
        let points: Vec<_> = path
            .0
            .iter()
            .map(|p| bounds.origin + point(bounds.size.width * p.x, bounds.size.height * p.y))
            .collect();
        let lengths: Vec<_> = points
            .windows(2)
            .map(|pair| {
                let delta = pair[1] - pair[0];
                f32::from(delta.x).hypot(f32::from(delta.y))
            })
            .collect();
        let total: f32 = lengths.iter().sum();
        let end = progress * total;
        let start = (progress - trail).max(0.) * total;
        let tail_length = trail * total;
        let mut ribbon = Vec::new();
        let mut distance = 0.;
        for (pair, length) in points.windows(2).zip(lengths) {
            let from = start.max(distance);
            let to = end.min(distance + length);
            if length > 0. && to > from {
                let delta = pair[1] - pair[0];
                if ribbon.is_empty() {
                    ribbon.push((pair[0] + delta * ((from - distance) / length), from));
                }
                ribbon.push((pair[0] + delta * ((to - distance) / length), to));
            }
            distance += length;
        }
        // Shared miter offsets keep adjacent ribbon segments joined at corners.
        let normals: Vec<_> = ribbon
            .windows(2)
            .map(|pair| {
                let delta = pair[1].0 - pair[0].0;
                let length = f32::from(delta.x).hypot(f32::from(delta.y));
                point(-f32::from(delta.y) / length, f32::from(delta.x) / length)
            })
            .collect();
        let offsets: Vec<_> = (0..ribbon.len())
            .map(|index| {
                let before = normals[index.saturating_sub(1).min(normals.len() - 1)];
                let after = normals[index.min(normals.len() - 1)];
                let denominator = (1. + before.x * after.x + before.y * after.y).max(0.5);
                point(
                    (before.x + after.x) / denominator,
                    (before.y + after.y) / denominator,
                )
            })
            .collect();
        for (spread, opacity) in [(4., 0.04), (2.5, 0.09), (1.5, 0.18), (1., 0.85)] {
            for (index, pair) in ribbon.windows(2).enumerate() {
                let strength = |distance: f32| (1. - (end - distance) / tail_length).clamp(0., 1.);
                let a = strength(pair[0].1);
                let b = strength(pair[1].1);
                let offset = |index: usize, strength: f32| {
                    point(
                        px(offsets[index].x * radius * spread * strength),
                        px(offsets[index].y * radius * spread * strength),
                    )
                };
                let mut strip = PathBuilder::fill();
                strip.add_polygon(
                    &[
                        pair[0].0 + offset(index, a),
                        pair[1].0 + offset(index + 1, b),
                        pair[1].0 - offset(index + 1, b),
                        pair[0].0 - offset(index, a),
                    ],
                    true,
                );
                let delta = pair[1].0 - pair[0].0;
                let angle = f32::from(delta.y).atan2(f32::from(delta.x)).to_degrees() + 90.;
                let color = |strength: f32| Rgba {
                    a: options.color.a * opacity * strength,
                    ..options.color
                };
                if let Ok(strip) = strip.build() {
                    window.paint_path(
                        strip,
                        linear_gradient(
                            angle,
                            linear_color_stop(color(a), 0.),
                            linear_color_stop(color(b), 1.),
                        ),
                    );
                }
            }
        }
        let center = bounds.origin + path.position(progress, bounds.size);
        let r = px(radius * 0.7);
        window.paint_quad(
            fill(
                Bounds::new(center - point(r, r), size(r * 2., r * 2.)),
                options.color,
            )
            .corner_radii(r),
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_progress_tracks_scaled_distance_and_skips_empty_segments() {
        let route = FlowPath::new([
            point(0., 0.),
            point(0.5, 0.),
            point(0.5, 0.),
            point(0.5, 1.),
        ])
        .unwrap();
        let extent = size(px(180.), px(30.));
        assert_eq!(route.position(0.5, extent), point(px(60.), px(0.)));
        assert_eq!(route.position(0.875, extent), point(px(90.), px(15.)));
        assert_eq!(route.position(2., extent), point(px(90.), px(30.)));
        assert!(FlowPath::new([point(0., 0.), point(f32::NAN, 1.)]).is_err());
    }
}
