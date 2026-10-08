# Range controls

UIC separates read-only progress from user-adjustable values. `Progress`
describes work that has already happened; `Slider` is a focusable control that
changes application state.

## Progress

`Progress` uses `0.0..=1.0` by default and accepts another finite ordered range:

```rust,ignore
Progress::new("download-progress", downloaded_bytes as f64)
    .label("Download progress")
    .range(0.0..=total_bytes as f64)
    .w_full()
    .h(px(6.))
    .rounded_full()
    .bg(rgba(0xffffff18))
```

A secondary value renders behind the primary value and is useful for buffered
media or prefetched work:

```rust,ignore
Progress::new("playback-progress", playback_seconds)
    .range(0.0..=duration_seconds)
    .secondary_value(buffered_seconds)
```

Use `Progress::indeterminate("loading-progress")` when there is no numeric
completion value. Each progress indicator takes a stable element id so multiple
indicators and list rows remain distinct in the accessibility tree.

The outer track implements `Styled`. Width, height, background, border, corner
radius, shadow, and opacity therefore use normal GPUI style methods. Internal
colors are semantic fields of `ProgressAppearance`.

## Slider

`SliderState` owns the value, range, focus, pointer capture, step, and disabled
state:

```rust,ignore
let volume = cx.new(|cx| {
    SliderState::new(0.65, 0.0..=1.0, cx).step(0.01)
});

Slider::new(&volume)
    .label("Volume")
    .cursor(CursorStyle::ResizeLeftRight)
    .thumb_visibility(SliderThumbVisibility::Hover)
    .hover_track_height(px(8.))
    .value_tooltip(SliderTooltipPlacement::Top, |value, _, _| {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgba(0x020617f2))
            .child(format!("{:.0}%", value * 100.0))
    })
    .w(px(280.))
    .h(px(28.))
```

`SliderThumbVisibility::Hover` keeps the outer layout and hit area stable while
the thumb appears and the visible track grows. Use `thumb_visible(false)` or
`SliderThumbVisibility::Never` when the thumb should never render; in that mode
the track uses its complete width for the numeric range. Pointer, keyboard,
accessibility, and change events remain available.

`value_tooltip` maps the pointer position through the slider's range and step,
then passes that value to the caller's render closure. The caller owns the
tooltip content and visual style. `SliderTooltipPlacement::Top` and `Bottom`
place it on the corresponding side of the interaction area. The tooltip follows
the pointer without changing the slider value and remains visible while dragging.

Programmatic `set_value` updates the control without producing a user event.
Pointer movement emits `SliderEvent::Changing`; pointer release, keyboard
adjustment, and accessibility actions emit `SliderEvent::Changed`.
`SliderState::is_dragging()` is true from pointer press through release, so a
view can also render a pressed thumb or retain a preview-only value while the
drag is active.

```rust,ignore
match event {
    SliderEvent::Changing(value) => preview(value),
    SliderEvent::Changed(value) => commit(value),
}
```

The arrow keys adjust by one step. Page Up and Page Down adjust by ten steps,
and Home and End select the range boundaries. A zero step keeps pointer input
continuous and uses one percent of the range for keyboard and accessibility
adjustments. Accessibility increment and decrement actions follow the same
step and range limits. Disabled sliders remain identifiable as disabled controls
and reject adjustments.

## Custom surfaces

The `Progress` Styled surface is its track. `SliderAppearance` also implements
`Styled`, targeting the slider track independently from the larger interaction
area. This keeps ordinary track properties in normal GPUI style APIs:

```rust,ignore
let appearance = SliderAppearance::default()
    .h(px(8.))
    .rounded_full()
    .bg(rgba(0xffffff18))
    .active_track(accent_gradient);
```

Semantic fill fields accept solid colors, gradients, and GPUI patterns.

For a different visual composition, the components accept normal GPUI elements:

```rust,ignore
Slider::new(&timeline)
    .track_content(custom_track)
    .secondary_content(buffer_pattern)
    .active_content(active_gradient)
    .thumb_content(icon)
```

The component clips track content to the correct numeric layer and positions
thumb content inside the thumb surface. Pointer capture, keyboard behavior,
focus, and accessibility remain owned by `Slider`.

Set the corresponding appearance background or border to transparent when the
custom element should completely replace that visual layer. Labels, tick marks,
and value text can be composed around the control without becoming part of its
interaction state.
