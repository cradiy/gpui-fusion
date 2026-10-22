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

Desktop supports dragging, Shift-click, double-click selection, Ctrl/Cmd+A and
Ctrl/Cmd+C with the system clipboard. Android uses the host's floating text menu
and themed selection handles on long press. Read-only selection does not open
the software keyboard and does not offer cut, paste or autofill. Android hosts
must be generated from a matching gpuiforge template.

Each view owns its selection; selections do not span separate views. Browser
DOM selection is not provided by this component.

Run `cargo run -p uic --example text_view` to inspect the component.
