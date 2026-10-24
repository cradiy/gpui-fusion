use gpui::{App, Context, Render, Window, WindowOptions, div, prelude::*, px};
use gpui_richtext::{
    CodeBlock, CodeBlockLabels, Document, Projection, RichText, RichTextTheme, compose::node,
    syntax::SyntaxHighlighter,
};
use std::{collections::HashMap, sync::Arc};

const SAMPLES: &[(&str, &str)] = &[
    (
        "json",
        "{\n  \"service\": \"atlas\",\n  \"port\": 7300,\n  \"enabled\": true,\n  \"fallback\": null,\n  \"regions\": [\"east\", \"west\"]\n}\n",
    ),
    (
        "jsonc",
        "{\n  // Connection settings\n  \"timeout\": 2500,\n  \"retry\": true,\n}\n",
    ),
    (
        "json5",
        "{\n  label: 'Atlas',\n  limit: Infinity,\n  mask: 0xff,\n}\n",
    ),
    (
        "rust",
        "pub fn total(values: &[u64]) -> u64 {\n    // Preserve the input order.\n    values.iter().sum()\n}\n",
    ),
    (
        "python",
        "from pathlib import Path\n\ndef list_names(root: Path) -> list[str]:\n    return [entry.name for entry in root.iterdir()]\n",
    ),
    (
        "javascript",
        "export async function load(url) {\n  const response = await fetch(url);\n  return response.json();\n}\n",
    ),
    (
        "typescript",
        "interface Sensor { id: string; active: boolean }\n\nconst describe = (sensor: Sensor): string =>\n  `${sensor.id}: ${sensor.active}`;\n",
    ),
    (
        "jsx",
        "export function Badge({ label }) {\n  return <span className=\"badge\">{label}</span>;\n}\n",
    ),
    (
        "tsx",
        "type Props = { label: string };\nexport const Badge = ({ label }: Props) => <b>{label}</b>;\n",
    ),
    (
        "vue",
        "<template>\n  <button @click=\"count++\">{{ count }}</button>\n</template>\n<script setup>\nimport { ref } from 'vue';\nconst count = ref(0);\n</script>\n",
    ),
    (
        "svelte",
        "<script>\n  let count = 0;\n</script>\n<button on:click={() => count++}>{count}</button>\n",
    ),
    (
        "html",
        "<!doctype html>\n<html lang=\"en\">\n  <body><main id=\"app\">Atlas</main></body>\n</html>\n",
    ),
    (
        "css",
        ".card {\n  display: grid;\n  gap: 12px;\n  color: #a4c8ff;\n}\n",
    ),
    (
        "scss",
        "$space: 12px;\n.card {\n  padding: $space;\n  &:hover { opacity: .8; }\n}\n",
    ),
    (
        "c",
        "#include <stdint.h>\nuint32_t square(uint32_t value) {\n    return value * value;\n}\n",
    ),
    (
        "cpp",
        "#include <vector>\nint total(const std::vector<int>& values) {\n    int result = 0;\n    for (auto value : values) result += value;\n    return result;\n}\n",
    ),
    (
        "java",
        "public record Sensor(String name, boolean active) {\n    public String label() { return name.toUpperCase(); }\n}\n",
    ),
    (
        "csharp",
        "public sealed record Sensor(string Name, bool Active);\nvar sensor = new Sensor(\"Atlas\", true);\n",
    ),
    (
        "go",
        "package main\n\nfunc total(values []int) int {\n    sum := 0\n    for _, value := range values { sum += value }\n    return sum\n}\n",
    ),
    (
        "kotlin",
        "data class Sensor(val name: String, val active: Boolean)\nfun label(sensor: Sensor) = sensor.name.uppercase()\n",
    ),
    (
        "swift",
        "struct Sensor {\n    let name: String\n    var active = true\n}\n",
    ),
    (
        "dart",
        "class Sensor {\n  final String name;\n  Sensor(this.name);\n  String get label => name.toUpperCase();\n}\n",
    ),
    (
        "php",
        "<?php\nfunction label(string $name): string {\n    return strtoupper($name);\n}\n",
    ),
    (
        "ruby",
        "class Sensor\n  attr_reader :name\n  def initialize(name)\n    @name = name\n  end\nend\n",
    ),
    (
        "lua",
        "local names = { 'east', 'west' }\nfor index, name in ipairs(names) do\n  print(index, name)\nend\n",
    ),
    (
        "sql",
        "SELECT region, COUNT(*) AS sensors\nFROM inventory\nWHERE active = TRUE\nGROUP BY region;\n",
    ),
    (
        "bash",
        "#!/bin/bash\nfor trace in logs/*.log; do\n  printf '%s\\n' \"$trace\"\ndone\n",
    ),
    (
        "powershell",
        "$label = 'Atlas'\n<# Inventory summary #>\nGet-Process | Where-Object { $_.CPU -gt 10 }\nWrite-Output \"Service: $label\"\n",
    ),
    (
        "vbnet",
        "Public Module Inventory\n    Sub Main()\n        Dim label As String = \"Atlas\"\n        Console.WriteLine(label)\n    End Sub\nEnd Module\n",
    ),
    (
        "toml",
        "[service]\nname = \"atlas\"\nport = 7300\nenabled = true\n",
    ),
    (
        "yaml",
        "service:\n  name: atlas\n  port: 7300\n  enabled: true\n",
    ),
    (
        "xml",
        "<?xml version=\"1.0\"?>\n<service enabled=\"true\">\n  <name>Atlas</name>\n</service>\n",
    ),
    (
        "dockerfile",
        "FROM debian:stable-slim\nWORKDIR /srv\nCOPY assets/ ./assets/\nENV PORT=7300\n",
    ),
    (
        "terraform",
        "variable \"region\" {\n  type    = string\n  default = \"us-east-1\"\n}\n",
    ),
    (
        "graphql",
        "query Inventory {\n  sensors(active: true) { id name }\n}\n",
    ),
    (
        "protobuf",
        "syntax = \"proto3\";\nmessage Sensor {\n  string name = 1;\n  bool active = 2;\n}\n",
    ),
    (
        "diff",
        "--- a/settings\n+++ b/settings\n@@ -1 +1 @@\n-timeout = 500\n+timeout = 1500\n",
    ),
];

struct Preview {
    theme: RichTextTheme,
    dark: bool,
    samples: Vec<(&'static str, &'static str)>,
    selected: usize,
    projection: Arc<Projection>,
    cache: HashMap<(usize, bool), Arc<Projection>>,
    task: Option<gpui::Task<()>>,
    error: Option<String>,
}

impl Preview {
    fn compile(&mut self, cx: &mut Context<Self>) {
        let key = (self.selected, self.dark);
        self.error = None;
        if let Some(projection) = self.cache.get(&key) {
            self.projection = projection.clone();
            return;
        }
        let (language, source) = self.samples[self.selected];
        let document =
            Document::from_roots([node(CodeBlock::new(source).language(language))]).unwrap();
        self.projection = Arc::new(
            self.theme
                .registry()
                .syntax_highlighter(None)
                .compile(&document)
                .unwrap(),
        );
        // One worker at a time; rapid selections coalesce into the latest request.
        if self.task.is_some() {
            return;
        }
        let theme = self.theme.clone();
        let work =
            cx.background_spawn(async move { theme.registry().compile(&document).map(Arc::new) });
        self.task = Some(cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |view, cx| {
                view.task = None;
                let current = (view.selected, view.dark);
                match result {
                    Ok(projection) => {
                        view.cache.insert(key, projection.clone());
                        if current == key {
                            view.projection = projection;
                        }
                    }
                    Err(error) if current == key => view.error = Some(error.to_string()),
                    Err(_) => {}
                }
                if current != key {
                    view.compile(cx);
                }
                cx.notify();
            });
        }));
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut languages = div()
            .id("languages")
            .w(px(170.))
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for (index, (language, _)) in self.samples.iter().enumerate() {
            languages = languages.child(
                div()
                    .id(index)
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(index == self.selected, |item| {
                        item.bg(self.theme.code_block.header_background)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = index;
                        this.compile(cx);
                        cx.notify();
                    }))
                    .child(*language),
            );
        }
        div()
            .size_full()
            .p_5()
            .bg(self.theme.background)
            .text_color(self.theme.foreground)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child("Code highlighting")
                    .child(
                        div()
                            .id("theme")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(self.theme.code_block.header_background)
                            .child(if self.dark {
                                "Light theme"
                            } else {
                                "Dark theme"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dark = !this.dark;
                                this.theme = if this.dark {
                                    RichTextTheme::dark()
                                } else {
                                    RichTextTheme::light()
                                };
                                this.compile(cx);
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.error.clone(), |view, error| view.child(error))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .flex_1()
                    .min_h_0()
                    .child(languages)
                    .child(
                        div()
                            .id("code")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .child(
                                RichText::new(("sample", self.selected), self.projection.clone())
                                    .theme(&self.theme)
                                    .w_full()
                                    .text_size(px(16.))
                                    .line_height(px(26.))
                                    .code_block_labels(CodeBlockLabels {
                                        copy: "Copy code".into(),
                                        copied: "Copied".into(),
                                    }),
                            ),
                    ),
            )
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(gpui_icons::IconAssets)
        .run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| {
                    let highlighter = SyntaxHighlighter::default();
                    let mut preview = Preview {
                        theme: RichTextTheme::dark(),
                        dark: true,
                        selected: 0,
                        samples: SAMPLES
                            .iter()
                            .copied()
                            .filter(|(language, _)| highlighter.language_name(language).is_some())
                            .collect(),
                        projection: Arc::default(),
                        cache: HashMap::new(),
                        task: None,
                        error: None,
                    };
                    preview.compile(cx);
                    preview
                })
            })
            .unwrap();
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn rapid_selection_and_theme_changes_publish_only_the_current_result(
        cx: &mut gpui::TestAppContext,
    ) {
        let preview = cx.new(|_| Preview {
            theme: RichTextTheme::dark(),
            dark: true,
            samples: vec![
                ("rust", "pub const LIMIT: usize = 31;"),
                ("json", "{\"limit\":31}"),
            ],
            selected: 0,
            projection: Arc::default(),
            cache: HashMap::new(),
            task: None,
            error: None,
        });
        preview.update(cx, |view, cx| {
            view.compile(cx);
            view.selected = 1;
            view.dark = false;
            view.theme = RichTextTheme::light();
            view.compile(cx);
            assert_eq!(view.projection.text().as_ref(), view.samples[1].1);
            assert!(
                view.projection
                    .spans()
                    .iter()
                    .all(|span| span.style.color.is_none())
            );
        });
        cx.run_until_parked();
        preview.update(cx, |view, cx| {
            assert!(view.task.is_none());
            assert!(view.error.is_none());
            assert_eq!(view.projection.text().as_ref(), view.samples[1].1);
            assert!(
                view.projection
                    .spans()
                    .iter()
                    .any(|span| span.style.color.is_some())
            );
            let cached = view.projection.clone();
            view.selected = 0;
            view.dark = true;
            view.theme = RichTextTheme::dark();
            view.compile(cx);
            assert_eq!(view.projection.text().as_ref(), view.samples[0].1);
            view.selected = 1;
            view.dark = false;
            view.theme = RichTextTheme::light();
            view.compile(cx);
            assert!(view.task.is_none());
            assert!(Arc::ptr_eq(&view.projection, &cached));
        });
    }
}
