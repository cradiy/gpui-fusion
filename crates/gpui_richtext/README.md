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
`Document::new`. Root nodes must resolve to paragraphs, code blocks, or block groups. Register payload types
in `Registry`, compile the document, and share the resulting `Arc<Projection>`
with `RichText::new`. Keep the element ID stable between renders. Compilation
belongs outside the render loop unless the content or registry changes.

`Registry::with_standard_nodes` registers `Paragraph`, `Span`, `Text`,
`InlineStyle`, `Strong`, `Emphasis`, `Underline`, `Strikethrough`, `Mark`, `Code`,
`HardBreak`, `Group`, `Link`, and `CodeBlock`. Style nodes are inline containers; `HardBreak` is a leaf that
contributes one newline inside a paragraph.
`register::<T>` adds or replaces a type's resolver. A resolver returns a
`Contribution` and can fail. Unknown types and invalid child roles fail compilation;
content is never silently omitted. Existing projections remain unchanged when
a registration is replaced.

## Composition and presets

The `compose` module provides `paragraph()`, `text(...)`, `strong()`,
`emphasis()`, `underline()`, `strikethrough()`, `mark()`, `code()`, `hard_break()`,
`span(...)`, `group()`, and `link(target)`. Containers accept `.child(...)` and `.children(...)` and can be
nested. Application components can be ordinary functions returning `NodeTree`.
`node(payload)` composes any registered custom node with the same API.

Pass the resulting roots to `Document::from_roots`. This assigns snapshot-local
IDs in reading order; use `Document::new` for application-managed identities.
Components share one document layout and selection rather than creating separate
GPUI text views. Invalid nesting is still rejected when compiling the document.

`Registry::with_theme(TextTheme)` configures each semantic style independently.
The default presets use bold, italic, text-colored underline and strikethrough,
a translucent highlight, and inline code with a translucent background.
All presets inherit the application font. Configure `theme.code.font_family`
with a font supplied or resolved by the application to use a monospace face.
Theme colors are presets; applications may supply light and dark palettes.
Code backgrounds follow glyph runs, without extra padding or rounded corners.

`InlineStyle` combines a `HighlightStyle` with an optional font family and can
also be used as a custom container payload. Nested font families override their
ancestors; sibling content resumes its inherited family. Custom resolvers may
return `Contribution::StyledInline` to supply this typography contract.

Ancestor highlights are combined using GPUI's `HighlightStyle::highlight`:
descendant weight and style override inherited values, while colors use its
compositing semantics. The view supplies the base font, size, color, and line
height. Paragraphs are separated by two newlines in both display and copying.

The view shares GPUI's desktop selection and Android native selection integration.
Changing projected text clears the selection; style-only changes preserve it.
Selection is currently indexed in the projected text, not retained as node-based
anchors across document updates. Browser-native DOM selection is not provided.

## Groups and links

`Group` contains paragraphs, code blocks, or nested groups. It adds no text or spacing of its
own; empty groups are omitted from text flow, while explicit empty paragraphs
retain their paragraph separators. Custom resolvers can return
`Contribution::Blocks(InlineStyle)` to apply inherited typography to a block group.
Blocks cannot occur inside inline containers.

`Link::new(link)` requires a nonblank destination. `.text(text)` optionally sets
the displayed label; without it the destination itself is displayed. The compose
helpers are `link(destination)` and `link_with_text(destination, label)`.
For styled labels, a link may instead contain inline children; combining explicit
text and children is rejected. Destinations are opaque application data and nested
links are rejected. Explicit empty labels have no clickable range.
`Projection::links` exposes source IDs, destinations, and UTF-8
ranges. Copying uses the displayed text, without inserting destinations.

Attach `RichText::on_link_click` to receive activation with its `LinkSpan`, window,
and application context. The renderer does not open URLs or execute destinations.
Applications decide which schemes and actions to allow. With no callback, links
are styled text only. Dragging to select does not activate a link. Link styles
are configurable through `TextTheme::link`.

Hovering a link shows its destination by default, even without a click callback.
Use `link_tooltips(false)` to disable this or `link_tooltip(...)` to provide a
custom tooltip view. The custom builder may return `None` to suppress individual
tooltips. Destination text is displayed as plain text and is never executed.

## Code blocks

`CodeBlock::new(source)` is a block leaf containing literal source text.
`compose::code_block(source)` provides the corresponding composition helper.
Spaces, tabs, blank lines, and trailing newlines remain in the projected and
copied text. Blocks are separated by two additional newlines, like paragraphs.
Code blocks cannot have children or occur inside a paragraph or link.

`.language(name)` records metadata; it does not load a parser or add a visible
language label. `.highlights(...)` accepts application-provided syntax highlights
as sorted, non-overlapping UTF-8 ranges. Compilation rejects out-of-bounds,
overlapping, reversed, and non-character-boundary ranges. Unhighlighted ranges
inherit `TextTheme::code_block`; highlighted ranges refine that style. Configure
its font family explicitly if an application-supplied monospace font is desired.

`Projection::blocks()` exposes paragraph and code ranges, node IDs, and code
language metadata. Ranges exclude inserted separators, allowing an application
to retrieve exactly one block's content. `Projection::offset` maps positions
through syntax highlight boundaries without altering source offsets.

Code blocks have a full-width rounded surface with a subtle neutral background
and border. `RichText::code_block_style(CodeBlockStyle)` controls the background,
border color, corner radius, and padding. Documents containing code blocks reserve
a shared text gutter; vertical surface padding is capped at a quarter line height
to keep neighboring blocks separate. Surface geometry follows the wrapped text
layout, including empty and trailing lines, without inserting padding characters.

Code blocks currently share the document's font size, line height, and wrapping.
Each code block has a header displaying its optional language and a Copy button.
Copy writes only that block's original source, including whitespace, to the system
clipboard. Headers are excluded from text selection and document offsets.
They have no independent horizontal scrollbar or line numbers.
Syntax background highlights cover text runs in addition to the block surface.

## Limits

The current renderer uses one text flow with a uniform font size and line height.
It does not yet implement inline objects, mixed font sizes, custom block layout,
node-specific materials, virtualized layout, incremental
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
