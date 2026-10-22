# gpui_richtext

Experimental read-only node-based text rendering for GPUI. The crate depends on
GPUI and does not require UIC or a document format parser.

## Supported text flow

- Immutable typed node payloads, ordered child ownership, and validated roots.
- Typed registration of custom payloads that contribute text or styled containers.
- Paragraphs and nested inline styles, compiled into a shared text stream.
- UTF-8 leaf source ranges, including empty leaves.
- A `Styled` view with selectable text using GPUI's read-only selection bridge.

Create `Node` values with document-local `NodeId`s, then validate them with
`Document::new`. Root nodes must resolve to paragraphs. Register payload types
in `Registry`, compile the document, and share the resulting `Arc<Projection>`
with `RichText::new`. Keep the element ID stable between renders. Compilation
belongs outside the render loop unless the content or registry changes.

`Registry::with_standard_nodes` registers `Paragraph`, `Span`, and `Text`.
`register::<T>` adds or replaces a type's resolver. A resolver returns a
`Contribution` and can fail. Unknown types and invalid child roles fail compilation;
content is never silently omitted. Existing projections remain unchanged when
a registration is replaced.

Ancestor highlights are combined using GPUI's `HighlightStyle::highlight`:
descendant weight and style override inherited values, while colors use its
compositing semantics. The view supplies the base font, size, color, and line
height. Paragraphs are separated by two newlines in both display and copying.

The view shares GPUI's desktop selection and Android native selection integration.
Changing projected text clears the selection; style-only changes preserve it.
Selection is currently indexed in the projected text, not retained as node-based
anchors across document updates. Browser-native DOM selection is not provided.

## Limits

The current renderer uses one text flow with a uniform font size and line height.
It does not yet implement inline objects, mixed font sizes, custom block layout,
node-specific materials, link activation, virtualized layout, incremental
compilation, or editing. Custom contributions cannot return arbitrary GPUI
elements. `Projection::offset` validates scalar boundaries; it is not a grapheme
navigation API. Source mappings refer to text returned by the resolver.

Validation limits documents to 100,000 nodes and 256 levels of nesting. All nodes
must be reachable exactly once from the roots. Projections contain only text,
styles, and source mappings; they retain neither the document nor its registry.

## Example

```sh
cargo run -p gpui_richtext --example document
```

The example registers a custom text payload and presents multiple styled
paragraphs in a single selectable view.
