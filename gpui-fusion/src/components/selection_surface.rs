use crate::style::{ComputedStyle, Resolved};
use gpui::{prelude::*, *};

pub(super) fn selection_surface(
    mut element: Div,
    off: ComputedStyle,
    on: ComputedStyle,
    selected: bool,
    amount: f32,
    scale: f32,
    pressure: f32,
) -> Div {
    let active = if selected { &on } else { &off };
    let indicator_style = active.to_gpui();
    let material = active.material;
    let border_width = length(&active.border_width, 0.);
    let border_gradient = active.border_gradient.clone();
    element.style().refine(&indicator_style);
    element
        .bg(transparent_black())
        .border_color(transparent_black())
        .on_paint_before_children(move |bounds, style, window, _| {
            let dimensions = size(bounds.size.width * scale, bounds.size.height * scale);
            let bounds = Bounds::new(
                bounds.center() - point(dimensions.width / 2., dimensions.height / 2.),
                dimensions,
            );
            let corners = style
                .corner_radii
                .to_pixels(window.rem_size())
                .map(|r| *r * scale)
                .clamp_radii_for_quad_size(bounds.size);
            let background = mix(color(&off.background), color(&on.background), amount);
            let edge = mix(color(&off.border_color), color(&on.border_color), amount);
            let gradient = off.background_gradient.is_some() || on.background_gradient.is_some();
            if let Some(mut glass) = material {
                if !gradient {
                    glass.tint = background.composite_over(glass.tint);
                }
                glass.thickness *= 1. + pressure * 0.25;
                glass.refraction *= 1. + pressure * 0.5;
                glass.shadow.spread = bounds.size.height.min(bounds.size.width) * 0.15;
                glass.shadow.offset.y = px(1.) * scale;
                gpui_effects::paint_liquid_glass(bounds, corners, glass, window);
            }
            if gradient {
                for (style, opacity) in [(&off, 1. - amount), (&on, amount)] {
                    let fill = style
                        .background_gradient
                        .clone()
                        .unwrap_or_else(|| color(&style.background).into());
                    if opacity > 0. {
                        window.paint_quad(quad(
                            bounds,
                            corners,
                            fill.opacity(opacity),
                            px(0.),
                            transparent_black(),
                            BorderStyle::Solid,
                        ));
                    }
                }
            }
            let fill: Background = if material.is_some() || gradient {
                transparent_black().into()
            } else {
                background.into()
            };
            let mut border = quad(
                bounds,
                corners,
                fill,
                border_width,
                edge,
                indicator_style.border_style.unwrap_or_default(),
            );
            if let Some(gradient) = &border_gradient {
                border.border_gradient = gradient.clone();
            }
            window.paint_quad(border);
        })
}
pub(super) fn color(value: &Resolved<Hsla>) -> Hsla {
    match value {
        Resolved::Value(color) => *color,
        _ => transparent_black(),
    }
}
fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let a: Rgba = a.into();
    let b: Rgba = b.into();
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
    .into()
}

pub(super) fn length(value: &Resolved<Pixels>, default: f32) -> Pixels {
    match value {
        Resolved::Value(value) => *value,
        _ => px(default),
    }
}
