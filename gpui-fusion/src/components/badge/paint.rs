use gpui::{Bounds, Pixels, Point, Window, point, px, size};
use gpui_effects::{StickyShape, paint_sticky_shapes};

pub(super) fn paint_elastic_tether(
    dragged_bounds: Bounds<Pixels>,
    offset: Point<Pixels>,
    threshold: Pixels,
    background: gpui::Hsla,
    window: &mut Window,
) {
    let distance = point_length(offset);
    if distance <= 3.0 || distance >= f32::from(threshold) {
        return;
    }

    let dragged_center = point(
        dragged_bounds.origin.x + dragged_bounds.size.width / 2.,
        dragged_bounds.origin.y + dragged_bounds.size.height / 2.,
    );
    let anchor_center = point(dragged_center.x - offset.x, dragged_center.y - offset.y);
    let bubble_radius = dragged_bounds.size.height / 2.;
    let tension = (distance / f32::from(threshold)).clamp(0.0, 1.0);
    let anchor_radius = bubble_radius * (1.0 - tension * 0.52).max(0.48);
    let target_scale = 1.0 + tension * 0.18;
    let target_size = size(
        dragged_bounds.size.width * target_scale,
        dragged_bounds.size.height * target_scale,
    );
    let target_radius = bubble_radius * target_scale;

    let padding = px(3.);
    let anchor_half = point(anchor_radius, anchor_radius);
    let dragged_half = point(target_size.width / 2., target_size.height / 2.);
    let field_origin = point(
        (anchor_center.x - anchor_half.x).min(dragged_center.x - dragged_half.x) - padding,
        (anchor_center.y - anchor_half.y).min(dragged_center.y - dragged_half.y) - padding,
    );
    let field_end = point(
        (anchor_center.x + anchor_half.x).max(dragged_center.x + dragged_half.x) + padding,
        (anchor_center.y + anchor_half.y).max(dragged_center.y + dragged_half.y) + padding,
    );
    let field_bounds = Bounds::new(
        field_origin,
        size(field_end.x - field_origin.x, field_end.y - field_origin.y),
    );
    let anchor = StickyShape::new(
        anchor_center - field_origin,
        size(anchor_radius * 2., anchor_radius * 2.),
        anchor_radius,
    );
    let target = StickyShape::new(dragged_center - field_origin, target_size, target_radius);
    let _ = paint_sticky_shapes(window, field_bounds, anchor, target, tension, background);
}

fn point_length(value: Point<Pixels>) -> f32 {
    f32::from(value.x).hypot(f32::from(value.y))
}
