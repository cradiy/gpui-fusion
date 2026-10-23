use gpui::{
    FontStyle, FontWeight, HighlightStyle, SharedString, StrikethroughStyle, UnderlineStyle, px,
    rgba,
};

/// Inherited inline typography. A missing field preserves its ancestor's value.
#[derive(Clone, Debug, Default)]
pub struct InlineStyle {
    pub highlight: HighlightStyle,
    pub font_family: Option<SharedString>,
}

impl InlineStyle {
    pub(crate) fn inherit(&self, child: &Self) -> Self {
        Self {
            highlight: self.highlight.highlight(child.highlight),
            font_family: child
                .font_family
                .clone()
                .or_else(|| self.font_family.clone()),
        }
    }
}

impl From<HighlightStyle> for InlineStyle {
    fn from(highlight: HighlightStyle) -> Self {
        Self {
            highlight,
            font_family: None,
        }
    }
}

/// Strong emphasis container.
pub struct Strong;
/// Emphasis container.
pub struct Emphasis;
/// Underlined inline container.
pub struct Underline;
/// Struck-through inline container.
pub struct Strikethrough;
/// Highlighted inline container.
pub struct Mark;
/// Inline code container. Its children remain selectable text.
pub struct Code;
/// A leaf contributing one newline within a paragraph.
pub struct HardBreak;
/// A group of paragraphs or nested block groups.
pub struct Group;
/// An inline link. Activation is delegated to the view's application callback.
pub struct Link {
    pub link: SharedString,
    pub text: Option<SharedString>,
}

impl Link {
    pub fn new(link: impl Into<SharedString>) -> Self {
        Self {
            link: link.into(),
            text: None,
        }
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }
}

/// Styles captured by standard node registration. Set fields before registering;
/// recompiling with a different theme does not mutate existing projections.
#[derive(Clone, Debug)]
pub struct TextTheme {
    pub table_header: InlineStyle,
    pub table_cell: InlineStyle,
    pub headings: [InlineStyle; 6],
    pub quote: InlineStyle,
    pub list_item: InlineStyle,
    pub strong: InlineStyle,
    pub emphasis: InlineStyle,
    pub underline: InlineStyle,
    pub strikethrough: InlineStyle,
    pub mark: InlineStyle,
    pub code: InlineStyle,
    pub code_block: InlineStyle,
    pub link: InlineStyle,
}

impl Default for TextTheme {
    fn default() -> Self {
        Self {
            table_header: HighlightStyle::from(FontWeight::SEMIBOLD).into(),
            table_cell: InlineStyle::default(),
            headings: std::array::from_fn(|index| {
                HighlightStyle::from(if index < 2 {
                    FontWeight::BOLD
                } else {
                    FontWeight::SEMIBOLD
                })
                .into()
            }),
            quote: HighlightStyle::from(FontStyle::Italic).into(),
            list_item: InlineStyle::default(),
            strong: HighlightStyle::from(FontWeight::BOLD).into(),
            emphasis: HighlightStyle::from(FontStyle::Italic).into(),
            underline: HighlightStyle {
                underline: Some(UnderlineStyle {
                    thickness: px(1.),
                    color: None,
                    wavy: false,
                }),
                ..Default::default()
            }
            .into(),
            strikethrough: HighlightStyle {
                strikethrough: Some(StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                }),
                ..Default::default()
            }
            .into(),
            mark: HighlightStyle {
                background_color: Some(rgba(0xe5b70040).into()),
                ..Default::default()
            }
            .into(),
            code: InlineStyle {
                highlight: HighlightStyle {
                    background_color: Some(rgba(0x80808020).into()),
                    ..Default::default()
                },
                font_family: None,
            },
            link: HighlightStyle {
                color: Some(rgba(0x2563ebff).into()),
                underline: Some(UnderlineStyle {
                    thickness: px(1.),
                    color: None,
                    wavy: false,
                }),
                ..Default::default()
            }
            .into(),
            code_block: InlineStyle {
                highlight: HighlightStyle {
                    font_style: Some(FontStyle::Normal),
                    font_weight: Some(FontWeight::NORMAL),
                    ..Default::default()
                },
                font_family: Some("monospace".into()),
            },
        }
    }
}
