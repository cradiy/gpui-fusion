//! Optional code highlighting performed while compiling a document.
use gpui::{FontStyle, FontWeight, HighlightStyle, SharedString, UnderlineStyle, px, rgba};
use std::{
    ops::Range,
    sync::{Arc, LazyLock},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{HighlightState, ThemeSet},
    parsing::{ParseState, SyntaxSet},
    util::LinesWithEndings,
};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SyntaxTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Default)]
pub struct SyntaxHighlighter {
    pub theme: SyntaxTheme,
}

pub(crate) struct CachedHighlight {
    source: SharedString,
    language: SharedString,
    theme: SyntaxTheme,
    pub(crate) ranges: Vec<(Range<usize>, HighlightStyle)>,
    complete_bytes: usize,
    complete_ranges: usize,
    checkpoint: Option<(HighlightState, ParseState)>,
}

impl SyntaxHighlighter {
    /// Unknown language identifiers retain the surrounding code style.
    pub fn highlight(
        &self,
        source: &str,
        language: &str,
    ) -> anyhow::Result<Vec<(Range<usize>, HighlightStyle)>> {
        Ok(self
            .highlight_cached(source.to_owned().into(), language, None)?
            .ranges
            .clone())
    }

    pub(crate) fn highlight_cached(
        &self,
        source: SharedString,
        language: &str,
        previous: Option<&Arc<CachedHighlight>>,
    ) -> anyhow::Result<Arc<CachedHighlight>> {
        let token = language.split_whitespace().next().unwrap_or_default();
        let previous = previous
            .filter(|previous| previous.language.as_ref() == token && previous.theme == self.theme);
        if let Some(previous) = previous.filter(|previous| previous.source == source) {
            return Ok(previous.clone());
        }
        let Some(syntax) = SYNTAXES.find_syntax_by_token(token) else {
            return Ok(Arc::new(CachedHighlight {
                source,
                language: token.to_owned().into(),
                theme: self.theme,
                ranges: Vec::new(),
                complete_bytes: 0,
                complete_ranges: 0,
                checkpoint: None,
            }));
        };
        let theme = match self.theme {
            SyntaxTheme::Light => "InspiredGitHub",
            SyntaxTheme::Dark => "base16-ocean.dark",
        };
        let theme = &THEMES.themes[theme];
        let previous = previous.filter(|previous| {
            previous.checkpoint.is_some()
                && source.get(..previous.complete_bytes)
                    == previous.source.get(..previous.complete_bytes)
        });
        let mut checkpoint = previous.and_then(|previous| previous.checkpoint.clone());
        let mut highlighter = if let Some((highlight, parse)) = &checkpoint {
            HighlightLines::from_state(theme, highlight.clone(), parse.clone())
        } else {
            HighlightLines::new(syntax, theme)
        };
        let mut offset = previous.map_or(0, |previous| previous.complete_bytes);
        let mut complete_ranges = previous.map_or(0, |previous| previous.complete_ranges);
        let mut ranges = previous.map_or_else(Vec::new, |previous| {
            previous.ranges[..complete_ranges].to_vec()
        });
        let complete_bytes = source.rfind('\n').map_or(0, |index| index + 1);
        for line in LinesWithEndings::from(&source[offset..]) {
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
            if offset == complete_bytes {
                let (highlight, parse) = highlighter.state();
                highlighter = HighlightLines::from_state(theme, highlight.clone(), parse.clone());
                checkpoint = Some((highlight, parse));
                complete_ranges = ranges.len();
            }
        }
        Ok(Arc::new(CachedHighlight {
            source,
            language: token.to_owned().into(),
            theme: self.theme,
            ranges,
            complete_bytes,
            complete_ranges,
            checkpoint,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumed_highlighting_matches_cold_parsing_after_tail_and_prefix_changes() {
        let highlighter = SyntaxHighlighter::default();
        let source = "/* outer\n/* nested */\nend */\nlet label = r#\"first\n第二行\"#;\n";
        let mut previous = None;
        for end in source
            .char_indices()
            .map(|(index, _)| index)
            .skip(1)
            .chain(std::iter::once(source.len()))
        {
            let current = highlighter
                .highlight_cached(source[..end].to_owned().into(), "rust", previous.as_ref())
                .unwrap();
            let cold = highlighter.highlight(&source[..end], "rust").unwrap();
            assert_eq!(format!("{:?}", current.ranges), format!("{cold:?}"));
            previous = Some(current);
        }
        for changed in [
            source.replace("/* outer", "// outer"),
            source[..source.find("let").unwrap()].to_owned(),
            "let mode = true;".into(),
        ] {
            let current = highlighter
                .highlight_cached(changed.clone().into(), "rust", previous.as_ref())
                .unwrap();
            assert_eq!(
                format!("{:?}", current.ranges),
                format!("{:?}", highlighter.highlight(&changed, "rust").unwrap())
            );
            previous = Some(current);
        }
        let dark = SyntaxHighlighter {
            theme: SyntaxTheme::Dark,
        };
        let changed = dark
            .highlight_cached("let mode = true;".into(), "javascript", previous.as_ref())
            .unwrap();
        assert_eq!(
            format!("{:?}", changed.ranges),
            format!(
                "{:?}",
                dark.highlight("let mode = true;", "javascript").unwrap()
            )
        );
    }
}
