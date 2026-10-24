# gpui_richtext

A node-based rich text renderer for GPUI, with composable content, custom styles,
and selectable text. Applications can register their own node types and handle
interactions. This crate renders documents; it does not provide an editor.

## Getting started

- Build content with [`compose`](src/compose.rs) and `Document`.
- Register node types and compile documents with `Registry`.
- Display the result with `RichText`.
- Optional features: `markdown` for parsing, `syntax-highlighting` for basic code colors,
  and `syntax-extra` for the extended language set.
- See the [document example](examples/document.rs) for application setup.

```sh
cargo run -p gpui_richtext --example document
cargo run -p gpui_richtext --example code_highlighting --features syntax-extra
```

## License

[MIT](LICENSE)
