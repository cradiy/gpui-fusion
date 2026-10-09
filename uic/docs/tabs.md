# Tabs

`Tabs<T>` is a controlled horizontal tab bar. Keep its ID stable, provide unique
values, and update the selected value in `on_change`. Tab content may contain text,
icons or badges. The bar does not create or retain page content.

```rust
use uic::components::tabs::{Tabs, TabVariant};

Tabs::new("sections", selected)
    .label("Workspace sections")
    .tab(0, "Overview")
    .tab(1, "Activity")
    .disabled_tab(2, "Offline")
    .variant(TabVariant::Pill)
    .on_change(move |value, _, cx| {
        view.update(cx, |view, cx| {
            view.selected = value;
            cx.notify();
        });
    });
```

`TabVariant::Underline` is the default; `Pill` fills the selected tab with a rounded
marker. Both reuse the selection indicator animation and respect the system's
reduced-motion preference.

Use ordinary `Styled` methods for the bar's dimensions, padding, background,
border, corner radius and typography. `TabsAppearance` sets the marker, selected
text, hover background, focus ring and disabled opacity. Tab content inherits the
bar's typography unless it supplies its own style.

## Interaction

Tabs keep their content width and scroll horizontally when they overflow. Give the
bar a bounded width, such as `.w_full()` inside a sized parent. Selection changes
and viewport resizing reveal the selected tab with the smallest required scroll.
Manual scrolling remains where the user leaves it until selection or layout
changes. A tab wider than the viewport aligns its leading edge.

The bar is one keyboard tab stop. Left and Right select the previous or next enabled
tab, wrapping at the ends. Home and End select the first and last enabled tab.
Selection activates immediately. Modified shortcuts and Up/Down are left to the
application. Clicking an already selected tab does not emit a change.

Disabled tabs cannot activate. `.disabled(true)` disables the entire bar and
removes its keyboard tab stop. An empty bar has no keyboard tab stop. A selected
value absent from the bar hides the marker; keyboard navigation can select an
enabled tab without changing application state during rendering.

The bar exposes tab-list and tab roles, selected/disabled states and an active
descendant to accessibility consumers. Use `.label(...)` to name the group. Avoid
nesting interactive controls in tab content.

## Connecting a Pager

Keep the tab selection in sync with `PagerState::current_page()` and subscribe to
`PageChanged` to redraw the containing view when a swipe changes pages.

```rust
Tabs::new("pages", pager.read(cx).current_page().unwrap_or(0))
    .tab(0, "Overview")
    .tab(1, "Activity")
    .on_change(move |index, _, cx| {
        pager.update(cx, |state, cx| {
            state.scroll_to(index, cx);
        });
    });
```

Run `cargo run -p uic --example tabs` to try both styles and overflow navigation.
The `pager` example demonstrates tab and swipe navigation together.
