# Text view

`TextView` displays read-only text with selection enabled by default. Use a stable
element ID to preserve the selection across renders. Changing the displayed text
clears the selection.

```rust
use gpui::{prelude::*, px};
use uic::components::text_view::TextView;

let body = TextView::new("article-body", article_text)
    .w_full()
    .text_size(px(18.))
    .line_height(px(28.));
```

Layout, typography and surface styling use `Styled`. `TextView::styled` accepts
`StyledText` for highlighted runs. `selection_color` customizes the selection
highlight; `selectable(false)` disables selection.

Attach a persistent `gpui::TextSelectionHandle` with `.selection_handle(&handle)`
to query selection and range geometry without changing focus or selection.
Use one handle per text occurrence. `selection(window)` returns the displayed
text and UTF-8 byte offsets for its anchor and head; `range()` orders those offsets.
`range_geometry(range, window)` returns separate visual fragments for wrapped
lines and bidirectional runs. Each fragment has full bounds and optional visible
bounds in window logical pixels, accounting for ancestor clips, transforms and
the viewport. Other elements' occlusion is not included.

Query after the text has prepainted, such as when positioning a deferred overlay.
During drawing, queries use that frame; otherwise they use the last completed
frame. Removed or not-yet-registered text returns `None`. Invalid UTF-8 ranges
also return `None`; empty ranges return no fragments. Fully clipped fragments
have no visible bounds. Range geometry is available with selection disabled,
while the selection snapshot is not.

Desktop supports dragging, Shift-click, double-click selection, Ctrl/Cmd+A and
Ctrl/Cmd+C with the system clipboard. Android uses the host's floating text menu
and themed selection handles on long press. Read-only selection does not open
the software keyboard and does not offer cut, paste or autofill. Android hosts
must be generated from a matching gpuiforge template.

Each view owns its selection; selections do not span separate views. Browser
DOM selection is not provided by this component.

Run `cargo run -p uic --example text_view` to inspect the component.
