use crate::{CodeBlockStyle, Registry, TableStyle, TaskMarkerStyle, TextTheme};
use gpui::{Hsla, rgb, rgba};

/// Coordinated document and surface colors. Use `registry` when compiling and
/// `RichText::theme` when rendering. Changing inline colors requires recompilation.
#[derive(Clone)]
pub struct RichTextTheme {
    pub footnote: crate::FootnoteStyle,
    /// Colors in Note, Tip, Important, Warning, Caution order.
    pub alerts: [crate::AlertStyle; 5],
    pub text: TextTheme,
    pub foreground: Hsla,
    pub background: Hsla,
    pub selection: Hsla,
    pub quote_border: Hsla,
    pub code_block: CodeBlockStyle,
    pub table: TableStyle,
    pub task_marker: TaskMarkerStyle,
    #[cfg(feature = "syntax-highlighting")]
    pub syntax: crate::syntax::SyntaxTheme,
}

impl Default for RichTextTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl RichTextTheme {
    pub fn light() -> Self {
        Self {
            footnote: crate::FootnoteStyle::default(),
            alerts: Self::alert_colors([0x2563eb, 0x16805d, 0x8250c4, 0x9a6700, 0xcf353d], 0.06),
            text: TextTheme::default(),
            foreground: rgb(0x273444).into(),
            background: rgb(0xffffff).into(),
            selection: rgba(0x4488ff55).into(),
            quote_border: rgb(0xc2cad5).into(),
            code_block: CodeBlockStyle {
                background: rgb(0xf6f8fa).into(),
                header_background: rgb(0xeef2f6).into(),
                border_color: rgb(0xd6dee8).into(),
                ..Default::default()
            },
            table: TableStyle::default(),
            task_marker: TaskMarkerStyle::default(),
            #[cfg(feature = "syntax-highlighting")]
            syntax: crate::syntax::SyntaxTheme::Light,
        }
    }

    pub fn dark() -> Self {
        let mut text = TextTheme::default();
        text.link.highlight.color = Some(rgb(0x79b8ff).into());
        text.footnote_backlink.highlight.color = Some(rgb(0x79b8ff).into());
        text.footnote.highlight.color = Some(rgb(0xaab7c7).into());
        text.code.highlight.background_color = Some(rgb(0x283343).into());
        text.mark.highlight.background_color = Some(rgba(0xe5b70038).into());
        Self {
            alerts: Self::alert_colors([0x79b8ff, 0x56d6a0, 0xc4a0f5, 0xeac56b, 0xff8d95], 0.08),
            footnote: crate::FootnoteStyle::default(),
            text,
            foreground: rgb(0xdce4ee).into(),
            background: rgb(0x161b22).into(),
            selection: rgba(0x539bf555).into(),
            quote_border: rgb(0x526176).into(),
            code_block: CodeBlockStyle {
                background: rgb(0x10161e).into(),
                header_background: rgb(0x1b2430).into(),
                border_color: rgb(0x303b4a).into(),
                header_color: rgb(0x9daec2).into(),
                copy_icon_color: rgb(0xb7c6d9).into(),
                button_hover_background: rgb(0x263344).into(),
                button_active_background: rgb(0x34475e).into(),
                copy_success_color: rgb(0x56d6a0).into(),
                ..Default::default()
            },
            table: TableStyle {
                background: rgb(0x161b22).into(),
                header_background: rgb(0x212936).into(),
                alternate_background: Some(rgb(0x1b222c).into()),
                border_color: rgb(0x303b4a).into(),
                ..Default::default()
            },
            task_marker: TaskMarkerStyle {
                border: rgb(0x6b7b90).into(),
                checked_background: rgb(0x25866b).into(),
                check: rgb(0xffffff).into(),
            },
            #[cfg(feature = "syntax-highlighting")]
            syntax: crate::syntax::SyntaxTheme::Dark,
        }
    }

    fn alert_colors(colors: [u32; 5], opacity: f32) -> [crate::AlertStyle; 5] {
        use gpui_icons::GpuiIcons;
        let icons = [
            GpuiIcons::LucideInfo,
            GpuiIcons::LucideLightbulb,
            GpuiIcons::LucideCircleAlert,
            GpuiIcons::LucideTriangleAlert,
            GpuiIcons::LucideOctagonAlert,
        ];
        std::array::from_fn(|index| {
            let accent: Hsla = rgb(colors[index]).into();
            crate::AlertStyle {
                accent,
                header_background: Hsla {
                    a: opacity,
                    ..accent
                },
                header_border: Hsla { a: 0.16, ..accent },
                header_icon: Some(icons[index].path().into()),
                background: Hsla {
                    a: opacity,
                    ..accent
                },
                title: None,
            }
        })
    }

    pub fn alert_mut(&mut self, kind: crate::AlertKind) -> &mut crate::AlertStyle {
        &mut self.alerts[kind.index()]
    }

    /// Register standard nodes and the feature-enabled syntax highlighter.
    pub fn registry(&self) -> Registry {
        let registry = Registry::with_theme(self.text.clone());
        #[cfg(feature = "syntax-highlighting")]
        let registry = registry.syntax_highlighter(Some(std::sync::Arc::new(
            crate::syntax::SyntaxHighlighter { theme: self.syntax },
        )));
        registry
    }
}
