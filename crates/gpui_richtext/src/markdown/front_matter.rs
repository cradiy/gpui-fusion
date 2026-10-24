use super::*;

/// Syntax of a document's leading metadata block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontMatterFormat {
    Yaml,
    Toml,
}

/// Raw metadata without delimiters. Decoding and validation belong to the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatter {
    pub format: FrontMatterFormat,
    pub source: gpui::SharedString,
    /// False while the opening block has no closing delimiter.
    pub complete: bool,
}

/// A parsed document and its separate, non-selectable metadata.
pub struct ParsedMarkdown {
    pub document: Document,
    pub front_matter: Option<FrontMatter>,
}

/// Parse Markdown and retain leading YAML (`---`) or TOML (`+++`) metadata.
/// Delimiters occupy unindented lines. YAML also accepts `...` as its closing line.
/// An unclosed metadata block has no body and reports `complete: false`.
/// Metadata values are preserved verbatim; no YAML or TOML deserialization occurs.
pub fn parse_with_metadata(source: &str) -> Result<ParsedMarkdown> {
    let (front_matter, body) = split(source);
    let document = super::parse_body(body)?;
    Ok(ParsedMarkdown {
        document,
        front_matter,
    })
}

fn split(source: &str) -> (Option<FrontMatter>, &str) {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let Some(open_end) = source.find('\n') else {
        return (None, source);
    };
    let format = match source[..open_end].trim_end_matches('\r') {
        "---" => FrontMatterFormat::Yaml,
        "+++" => FrontMatterFormat::Toml,
        _ => return (None, source),
    };
    let start = open_end + 1;
    let mut offset = start;
    for line in source[start..].split_inclusive('\n') {
        let delimiter = line.trim_end_matches(['\r', '\n']);
        let closes = match format {
            FrontMatterFormat::Yaml => matches!(delimiter, "---" | "..."),
            FrontMatterFormat::Toml => delimiter == "+++",
        };
        if closes {
            return (
                Some(FrontMatter {
                    format,
                    source: source[start..offset].to_owned().into(),
                    complete: true,
                }),
                &source[offset + line.len()..],
            );
        }
        offset += line.len();
    }
    (
        Some(FrontMatter {
            format,
            source: source[start..].to_owned().into(),
            complete: false,
        }),
        "",
    )
}
