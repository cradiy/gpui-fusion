//! Optional code highlighting performed while compiling a document.
use gpui::{FontStyle, FontWeight, HighlightStyle, SharedString, UnderlineStyle, px, rgba};
use std::{
    ops::Range,
    sync::{Arc, LazyLock},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{HighlightState, ThemeSet},
    parsing::{ParseState, SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

#[cfg(feature = "syntax-extra")]
mod supplemental;

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(|| {
    #[cfg(feature = "syntax-extra")]
    {
        let mut builder = two_face::syntax::extra_newlines().into_builder();
        supplemental::register(&mut builder);
        builder.build()
    }
    #[cfg(not(feature = "syntax-extra"))]
    {
        SyntaxSet::load_defaults_newlines()
    }
});
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Notices for the embedded third-party syntax definitions.
#[cfg(feature = "syntax-extra")]
pub use two_face::acknowledgement as licenses;

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

/// A bundled language available to code blocks and application language pickers.
pub struct SyntaxLanguage {
    pub name: &'static str,
    pub extensions: &'static [String],
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
    pub fn supported_languages(&self) -> impl Iterator<Item = SyntaxLanguage> {
        SYNTAXES
            .syntaxes()
            .iter()
            .filter(|syntax| !syntax.hidden && syntax.name != "Plain Text")
            .map(|syntax| SyntaxLanguage {
                name: &syntax.name,
                extensions: &syntax.file_extensions,
            })
    }
    /// Resolve a language identifier, common alias, extension or file name.
    pub fn language_name(&self, language: &str) -> Option<&'static str> {
        resolve_syntax(language).map(|syntax| syntax.name.as_str())
    }

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
        let syntax = resolve_syntax(language);
        let token = syntax.map_or(language, |syntax| syntax.name.as_str());
        let previous = previous
            .filter(|previous| previous.language.as_ref() == token && previous.theme == self.theme);
        if let Some(previous) = previous.filter(|previous| previous.source == source) {
            return Ok(previous.clone());
        }
        let Some(syntax) = syntax else {
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

fn resolve_syntax(language: &str) -> Option<&'static SyntaxReference> {
    let language = language.trim();
    if matches!(
        language.to_ascii_lowercase().as_str(),
        "plain text" | "text" | "txt" | "plaintext" | "plain"
    ) {
        return None;
    }
    if let Some(syntax) = SYNTAXES
        .syntaxes()
        .iter()
        .find(|syntax| syntax.name.eq_ignore_ascii_case(language))
    {
        return Some(syntax);
    }
    let token = language.split_whitespace().next().unwrap_or_default();
    let token = token
        .strip_prefix("{.")
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(token);
    let token = token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .to_ascii_lowercase();
    if let Some(syntax) = SYNTAXES.find_syntax_by_extension(&token) {
        if syntax.name == "Plain Text" {
            return None;
        }
        // Explicit names disambiguate extensions shared by different languages.
        if !matches!(token.as_str(), "fs" | "v" | "h") {
            return Some(syntax);
        }
    }
    let token = token.trim_start_matches('.');
    let token = token.strip_prefix("language-").unwrap_or(&token);
    let token = match token {
        "text" | "txt" | "plaintext" | "plain" => return None,
        "shell" | "sh" | "shellscript" | "zsh" => "bash",
        "csharp" | "cs" => "c#",
        "cxx" | "cc" => "c++",
        "py" | "python3" => "python",
        "js" | "node" | "nodejs" => "javascript",
        "ts" => "typescript",
        "jsx" | "javascriptreact" | "react" => "TypeScriptReact",
        "tsreact" | "typescriptreact" => "TypeScriptReact",
        "vuejs" => "vue",
        "sveltejs" => "svelte",
        "fsharp" | "fs" => "f#",
        "vb" | "vbnet" | "visualbasic" => "Visual Basic",
        "objc" | "objectivec" => "objective-c",
        "objcpp" => "objective-c++",
        "c++" | "cplusplus" => "c++",
        "h" => "c",
        "v" => "verilog",
        "pwsh" => "powershell",
        "bat" | "cmd" | "dos" | "batch" => "Batch File",
        "jsonc" | "json5" => "JSON Extended",
        "dotenv" => "DotENV",
        "tf" | "hcl" => "Terraform",
        "gql" => "GraphQL",
        "proto" => "Protocol Buffer",
        "html5" => "html",
        "tex" => "LaTeX",
        "asm" | "assembly" => "x86_64 Assembly",
        "yml" => "yaml",
        "golang" => "go",
        "kt" | "kts" => "kotlin",
        "ps1" | "ps" => "powershell",
        "docker" => "dockerfile",
        other => other,
    };
    SYNTAXES
        .syntaxes()
        .iter()
        .find(|syntax| syntax.name.eq_ignore_ascii_case(token))
        .or_else(|| SYNTAXES.find_syntax_by_token(token))
        .or_else(|| {
            token
                .match_indices('.')
                .find_map(|(index, _)| SYNTAXES.find_syntax_by_extension(&token[index + 1..]))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "syntax-extra")]
    fn typescript_aliases_reuse_state_without_losing_template_interpolation() {
        let highlighter = SyntaxHighlighter {
            theme: SyntaxTheme::Dark,
        };
        let source =
            "type Message = { id: number };\nconst message = `prefix\n${3 + 5}\nsuffix`;\n";
        let first = highlighter
            .highlight_cached(source.into(), "ts", None)
            .unwrap();
        let alias = highlighter
            .highlight_cached(source.into(), "TypeScript", Some(&first))
            .unwrap();
        assert!(Arc::ptr_eq(&first, &alias));
        let mut previous = None;
        for end in source
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(source.len()))
        {
            let current = highlighter
                .highlight_cached(source[..end].to_owned().into(), "ts", previous.as_ref())
                .unwrap();
            assert_eq!(
                current.ranges,
                highlighter.highlight(&source[..end], "typescript").unwrap()
            );
            previous = Some(current);
        }
    }

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
