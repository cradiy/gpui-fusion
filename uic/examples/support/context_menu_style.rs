use gpui::{IntoElement, div, prelude::*, px, rgb, rgba, transparent_black};
use gpui_effects::{LiquidGlass, LiquidGlassStyle};
use uic::components::context_menu::{ContextMenu, ContextMenuAppearance};

#[derive(Clone, Copy)]
pub enum MenuMaterial {
    DarkFrosted,
    LightFrosted,
    Plain,
}

impl MenuMaterial {
    pub fn label(self) -> &'static str {
        match self {
            Self::DarkFrosted => "Dark frosted",
            Self::LightFrosted => "Light frosted",
            Self::Plain => "Plain div",
        }
    }
}

pub fn menu(material: MenuMaterial) -> ContextMenu {
    let light = matches!(material, MenuMaterial::LightFrosted);
    let foreground = if light { rgb(0x242428) } else { rgb(0xf5f5f7) };
    let muted = if light { rgb(0x73737b) } else { rgb(0xa8a8b1) };
    let mut glass = if light {
        LiquidGlassStyle::frosted()
    } else {
        LiquidGlassStyle::dark()
    };
    glass.refraction = px(6.);
    glass.thickness = px(14.);
    glass.blur_radius = px(12.);
    glass.frost = 0.62;
    glass.optics.rim_strength = 0.35;
    glass.optics.top_reflection = 0.08;
    glass.optics.inner_shading = 0.04;
    glass.shadow.strength = 0.;

    ContextMenu::new()
        .appearance(
            ContextMenuAppearance::default()
                .muted_foreground(muted.into())
                .danger_foreground(if light { rgb(0xbe123c) } else { rgb(0xfb7185) }.into())
                .selected_background(
                    if light {
                        rgba(0x0000000e)
                    } else {
                        rgba(0xffffff12)
                    }
                    .into(),
                )
                .selected_foreground(foreground.into())
                .item_height(px(36.0))
                .item_padding_x(px(12.0))
                .item_radius(px(8.0))
                .separator(
                    if light {
                        rgba(0x24242814)
                    } else {
                        rgba(0xffffff18)
                    }
                    .into(),
                )
                .separator_margin(px(5.0)),
        )
        .w(px(232.0))
        .max_h(px(420.0))
        .p(px(6.0))
        .rounded(px(14.0))
        .border(px(0.0))
        .bg(transparent_black())
        .text_color(foreground)
        .text_size(px(14.))
        .font_family(gpui::GenericFontFamily::SystemUi)
        .surface(move |state, content, _, _| match material {
            MenuMaterial::DarkFrosted | MenuMaterial::LightFrosted => {
                LiquidGlass::with_style(glass)
                    .id((
                        "context-menu-glass",
                        state.session_id * 10 + state.depth as u64,
                    ))
                    .rounded(px(14.))
                    .shadow_lg()
                    .child(content)
                    .into_any_element()
            }
            MenuMaterial::Plain => div()
                .rounded(px(14.))
                .border_1()
                .border_color(rgb(0x334155))
                .bg(rgb(0x111827))
                .shadow_lg()
                .child(content)
                .into_any_element(),
        })
}
