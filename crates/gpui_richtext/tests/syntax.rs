#![cfg(feature = "syntax-highlighting")]
use gpui_richtext::{CodeBlock, Document, Registry, compose::node, syntax::SyntaxHighlighter};

#[test]
#[cfg(feature = "syntax-extra")]
fn extended_languages_highlight_tokens_and_resolve_fence_labels() {
    let highlighter = SyntaxHighlighter::default();
    for (language, alias, source) in [
        (
            "JavaScriptReact",
            "widget.jsx",
            "export const Tag = () => <div title=\"温\">{19}</div>;\n",
        ),
        (
            "PowerShell",
            "pwsh",
            "<# block\ncomment #>\n$value = @\"\n温\n\"@\nWrite-Output $value\n",
        ),
        (
            "Visual Basic",
            "vbnet",
            "Public Class Reading\n  Dim Label As String = \"温\"\nEnd Class\n",
        ),
        (
            "JSON Extended",
            "json5",
            "/* settings */\n{label: '温', max: Infinity, code: 0x2a}\n",
        ),
        (
            "jsonc",
            "settings.jsonc",
            "// settings\n{\"label\": \"温\", \"max\": 19}\n",
        ),
        (
            "vue",
            "component.vue",
            "<template><p>{{ label }}</p></template>\n<script>const label = '温';</script>\n",
        ),
        (
            "svelte",
            "component.svelte",
            "<script>let label = '温';</script>\n<p>{label}</p>\n",
        ),
        ("f#", "fsharp", "let label = \"温\"\nlet count = 19\n"),
        ("DotENV", ".env.production", "LABEL=\"温\"\nPORT=7301\n"),
        (
            "CMake",
            "CMakeLists.txt",
            "cmake_minimum_required(VERSION 3.28)\nproject(Reader)\n",
        ),
        (
            "typescript",
            "TS",
            "interface Entry { label: string; }\nconst entry: Entry = { label: '夏' };\n",
        ),
        (
            "tsx",
            "panel.tsx",
            "export const Panel = () => <section title=\"夏\">{42}</section>;\n",
        ),
        (
            "toml",
            "settings.toml",
            "[storage]\npath = \"夏\"\ncapacity = 42\n",
        ),
        ("yaml", "YML", "storage:\n  path: \"夏\"\n  capacity: 42\n"),
        (
            "Dockerfile",
            "docker",
            "FROM alpine:3.20\nENV LABEL=\"夏\"\nRUN echo 42\n",
        ),
        (
            "kotlin",
            "kt",
            "val label: String = \"夏\"\nfun count(): Int = 42\n",
        ),
        (
            "bash",
            "shell",
            "# inventory\nprintf '%s' \"夏\"\nexit 42\n",
        ),
        (
            "c#",
            "csharp",
            "public class Entry { public string Label = \"夏\"; }\n",
        ),
    ] {
        let ranges = highlighter.highlight(source, language).unwrap();
        assert!(!ranges.is_empty(), "{language}");
        assert_eq!(ranges.first().unwrap().0.start, 0, "{language}");
        assert_eq!(ranges.last().unwrap().0.end, source.len(), "{language}");
        assert!(
            ranges
                .windows(2)
                .all(|pair| pair[0].0.end == pair[1].0.start),
            "{language}"
        );
        assert!(
            ranges
                .iter()
                .all(|(range, _)| source.get(range.clone()).is_some()),
            "{language}"
        );
        assert!(
            ranges
                .iter()
                .any(|(_, style)| style.color != ranges[0].1.color),
            "{language}"
        );
        assert_eq!(
            ranges,
            highlighter.highlight(source, alias).unwrap(),
            "{alias}"
        );
    }
    assert!(
        highlighter
            .highlight("const count = 42;", "plaintext")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn json_highlights_literals_without_rewriting_escaped_or_unicode_strings() {
    let source = "{\"label\":\"温\\n度\",\"count\":27,\"ready\":true,\"next\":null}";
    for theme in [
        gpui_richtext::syntax::SyntaxTheme::Light,
        gpui_richtext::syntax::SyntaxTheme::Dark,
    ] {
        let highlighter = SyntaxHighlighter { theme };
        let ranges = highlighter.highlight(source, "JSON").unwrap();
        let style_at = |text: &str| {
            ranges
                .iter()
                .find(|(range, _)| range.contains(&source.find(text).unwrap()))
                .unwrap()
                .1
        };
        assert_ne!(style_at("温").color, style_at("27").color);
        assert_ne!(style_at("温").color, style_at("true").color);
        assert_eq!(
            ranges
                .iter()
                .map(|(range, _)| &source[range.clone()])
                .collect::<String>(),
            source
        );
    }
}

#[test]
fn highlighting_tracks_multiline_state_and_utf8_without_changing_source() {
    let source = "/* α\nstill comment */\nlet count = 42;\n";
    let highlighter = SyntaxHighlighter::default();
    let ranges = highlighter.highlight(source, "rust").unwrap();
    assert_eq!(ranges.first().unwrap().0.start, 0);
    assert_eq!(ranges.last().unwrap().0.end, source.len());
    assert!(
        ranges
            .windows(2)
            .all(|pair| pair[0].0.end == pair[1].0.start)
    );
    let color_at = |index| {
        ranges
            .iter()
            .find(|(range, _)| range.contains(&index))
            .unwrap()
            .1
            .color
    };
    assert_eq!(
        color_at(source.find('α').unwrap()),
        color_at(source.find("still").unwrap())
    );
    assert_ne!(color_at(3), color_at(source.find("42").unwrap()));
    assert!(
        highlighter
            .highlight(source, "unlisted-language")
            .unwrap()
            .is_empty()
    );
    let document = Document::from_roots([node(CodeBlock::new(source).language("rust"))]).unwrap();
    let projection = Registry::with_standard_nodes().compile(&document).unwrap();
    assert_eq!(projection.text().as_ref(), source);
    assert!(
        projection
            .spans()
            .iter()
            .any(|span| span.style.color.is_some())
    );
    let uncolored = Registry::with_standard_nodes()
        .syntax_highlighter(None)
        .compile(&document)
        .unwrap();
    assert!(
        uncolored
            .spans()
            .iter()
            .all(|span| span.style.color.is_none())
    );
}
