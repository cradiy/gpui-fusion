//! Optional code highlighting performed while compiling a document.
use gpui::{FontStyle, FontWeight, HighlightStyle, UnderlineStyle, px, rgba};
use std::{ops::Range, sync::LazyLock};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Clone, Copy, Default)]
pub enum SyntaxTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Default)]
pub struct SyntaxHighlighter {
    pub theme: SyntaxTheme,
}

impl SyntaxHighlighter {
    /// Unknown language identifiers retain the surrounding code style.
    pub fn highlight(
        &self,
        source: &str,
        language: &str,
    ) -> anyhow::Result<Vec<(Range<usize>, HighlightStyle)>> {
        let token = language.split_whitespace().next().unwrap_or_default();
        let Some(syntax) = SYNTAXES.find_syntax_by_token(token) else {
            return Ok(Vec::new());
        };
        let theme = match self.theme {
            SyntaxTheme::Light => "InspiredGitHub",
            SyntaxTheme::Dark => "base16-ocean.dark",
        };
        let mut highlighter = HighlightLines::new(syntax, &THEMES.themes[theme]);
        let mut offset = 0;
        let mut ranges = Vec::new();
        for line in LinesWithEndings::from(source) {
            for (style, fragment) in highlighter.highlight_line(line, &SYNTAXES)? {
                let color = style.foreground;
                let font = style.font_style;
                let highlight = HighlightStyle {
                    color: Some(
                        rgba(u32::from_be_bytes([color.r, color.g, color.b, color.a])).into(),
                    ),
                    font_weight: font
                        .contains(syntect::highlighting::FontStyle::BOLD)
                        .then_some(FontWeight::BOLD),
                    font_style: font
                        .contains(syntect::highlighting::FontStyle::ITALIC)
                        .then_some(FontStyle::Italic),
                    underline: font
                        .contains(syntect::highlighting::FontStyle::UNDERLINE)
                        .then_some(UnderlineStyle {
                            thickness: px(1.),
                            color: None,
                            wavy: false,
                        }),
                    ..Default::default()
                };
                ranges.push((offset..offset + fragment.len(), highlight));
                offset += fragment.len();
            }
        }
        Ok(ranges)
    }
}
