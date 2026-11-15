# Styles and themes

Import `gpui_fusion::prelude::Styled` for Fusion's symbolic style builders.
Components implement this trait by exposing their `StylePatch` and a typed
`Part` identifier. GPUI's `Styled` is used only by the final style adapter.

Controls are grouped under `gpui_fusion::components::{button, switch, slider, segmented}`.
Theme models live in `gpui_fusion::theme`; its `catalog`, `runtime`, and `session`
modules provide directory discovery, application injection, and synchronization.
Symbolic values and component recipes live in `gpui_fusion::style`.

## Values

`StyleValue<T>` stores a literal, a typed token reference, or a derived expression.
Supported token types are `Hsla`, `Pixels`, and `f32`. Color expressions support
opacity multiplication; numeric expressions support scaling. Mixing interpolates
two resolved values. Color mixing interpolates straight, encoded sRGB components.
Fixed values remain unchanged across mode changes.

`Token<T>` is relative to the selected style set. `in_style_set` qualifies a
reference and bypasses local token overrides, including for relative aliases
inside that qualified definition. This allows a local override to derive from
the original theme token without creating a self-reference.

## Layers and states

`Property::Unset` preserves a lower layer. `Property::Clear` removes its value.
`Property::Set` stores an expression. Repeated setters replace the same property;
spacing shorthands assign their individual edges in call order.

`resolve_style` accepts layers from lowest to highest priority. A layer's base
properties are applied first, followed by matching state rules in declaration
order. Rules require all states in their selector. State combinations do not
introduce implicit specificity. Higher layers override both the base and state
properties of lower layers.

`Recipe` contains a base patch, named variants, and declared part names. Recipe
resolution applies base, variant, the active glass adaptation, then instance.
Instance overrides remain last. `ComponentStyles` owns the recipes independently
of theme palettes. Unknown variants and undeclared parts are errors. Components must validate recipes against their actual supported parts
before accepting externally supplied part declarations.

Parts resolve independently from root properties. Repeated `part` builders
refine the existing patch. State rules may target parts, and parts may have their
own state rules. Nested parts are not supported. Input handling supplies the
active state set; style resolution does not change component behavior.

## Themes and local scopes

`Theme` contains named modes and named `StyleSet` values. Each style set can
inherit from one parent and register semantic tokens. Themes do not contain
component names, parts, or recipes. A token's
mode-specific value precedes its default. A definition without either continues
lookup through outer scopes and parent style sets.

`ResolveContext` binds a theme, mode, and selected style set. `scoped` introduces
local token overrides and optionally selects another style set. Inner scopes
precede outer scopes. Inherited aliases resolve against the selected child's
overrides. Component recipes resolve their symbolic values through this context.

`ThemeStore` owns a validated theme and active mode. Successful changes increment
its revision. `replace_json` validates the replacement before changing either the
theme or revision, and rejects replacements that omit the active mode. The
`ThemeRuntime` GPUI entity wraps this store and notifies subscribed Fusion
components after successful changes. Install it in the application context before
creating components:

```rust,ignore
ThemeRuntime::install(
    ThemeStore::new(default_theme(), "light")?,
    "neutral",
    cx,
)?;
let button = cx.new(|cx| Button::new("Apply", window, cx));
```

`ThemeRuntime::global(cx)` returns the injected entity for mode changes and JSON
replacement. `install` uses `default_styles()`; `install_with_styles` accepts a
separate `ComponentStyles` registry. `replace_styles_json` replaces that registry
after validating it against the active theme. `set_style_set` changes the application default; explicit component
scopes retain their style-set selection. Reinstalling updates the same entity,
including existing observers, without resetting component values or focus.
Invalid installations and replacements leave the active theme unchanged.
Components use the runtime revision for caches, including across reinstallations.
Standalone store users supply their own invalidation. `ThemeRuntime` performs
explicit updates; `ThemeSession` optionally owns a file subscription.

## Semantic colors

`default_theme()` supplies the following roles in both light and dark modes.
They describe visual purpose independently of component recipes. Custom style
sets can override individual roles or inherit them from another set.

| Group | Tokens | Purpose |
| --- | --- | --- |
| Surfaces | `canvas`, `surface`, `muted_surface`, `surface_raised`, `surface_sunken` | Page background, content surfaces, quiet surfaces, elevated surfaces, and inset regions |
| Inverse | `surface_inverse`, `foreground_inverse` | A contrasting surface and its readable foreground |
| Foregrounds | `foreground`, `foreground_secondary`, `foreground_muted`, `foreground_disabled` | Primary content, secondary content, supporting content, and unavailable content |
| Edges | `edge`, `edge_subtle`, `edge_strong`, `divider` | Default, quiet, strong, and separating boundaries |
| Fills | `fill_hover`, `fill_pressed`, `fill_disabled` | Neutral interaction and unavailable fills |
| Contrast | `surface_contrast`, `edge_contrast` | High-luminance surfaces and their edges, including opaque or frosted handles |
| Roles | `primary`, `accent`, `success`, `warning`, `error`, `info` | Brand, secondary emphasis, and semantic status colors |
| Links | `link`, `link_hover`, `link_pressed` | Linked text in each interaction state |
| Selection | `selection`, `on_selection`, `selection_edge` | Selected background, content, and boundary |
| Other | `placeholder`, `focus`, `scrim`, `shadow` | Input hints, focus indicators, translucent occlusion, and shadows |

Each role has `<role>_hover`, `<role>_pressed`, `<role>_subtle`,
`<role>_foreground`, `<role>_edge`, and `on_<role>` companions. Use `on_<role>`
over the strong role fill; use `<role>_foreground` over the subtle fill. Strong
fills are not interchangeable with text colors, particularly in dark mode.

`divider` references `edge_subtle`; `placeholder` references `foreground_muted`.
`link` references `primary_foreground`, with hover and pressed colors derived
by mixing toward `foreground`. Selection roles reference the primary subtle,
foreground, and edge colors. These references follow scoped and inherited
overrides without copying colors. Any alias can also be replaced independently.
`scrim` and `shadow` include alpha; disabling content does not require reducing
the whole component's opacity.

## Material selection and files

`Theme.material` selects `MaterialMode::Plain` or `MaterialMode::Glass` globally.
The component recipe supplies geometry and its glass adaptation. Generic tokens
such as `glass_blur`, `glass_refraction`, `glass_thickness`, `glass_edge_light`, `glass_edge_color`, and `glass_frost`
control material parameters through the same theme file. Components
follow the theme by default. `.material_mode(mode)` fixes a component's material;
`set_material_mode(None, cx)` resumes following. Fixed controls unsubscribe from
material changes while continuing to receive color, metric, and typography
updates. Instance `Styled` patches remain the final override, including explicit
material parameters.

`ThemeConfig` combines the theme, active color mode, and selected style set in one
JSON document. `ThemeRuntime::install_file(path, cx)` installs it;
`reload_file(path, cx)` and `replace_json(source, cx)` atomically apply a complete
configuration to existing controls. Invalid files preserve the current colors,
material, mode, and style-set selection. `configuration()` returns a serializable
snapshot. `set_material_mode` also updates this configuration, so saved files
include programmatic changes. Material-only changes do not invalidate fixed
controls' style caches.

`ThemeRuntime` file reads are explicit and synchronous. Hosts can also read
asynchronously and call `replace_json` or `apply_config` with a loaded document.

## Theme directories and catalogs

`ThemeRepository::discover()` uses the nonempty `GPUI_FUSION_THEME` environment
variable as the complete theme root. Otherwise it uses the platform's configuration
directory followed by `org.gpui.fusion`. On Linux, this is under `XDG_CONFIG_HOME`,
or the user's `.config` directory when that variable is unavailable. An explicit
root can be supplied with `ThemeRepository::from_root`. Discovery does not create
directories or write configuration.

The root `index.toml` registers global themes. Each application can have a separate
`<app-id>/index.toml`. Application IDs are stable directory names, not window titles
or process IDs. Both indexes use the same `ThemeIndex` format:

| TOML field | Meaning |
| --- | --- |
| `default` | Optional registered theme ID used by followers |
| `themes.<id>.name` | Display name for a registered theme |
| `themes.<id>.file` | JSON document path relative to this index's directory |

Theme files contain `ThemeConfig`, including the theme, mode, and selected style
set. Nested relative paths are supported. Absolute paths and parent components
are rejected; native filesystem symlinks are followed normally. Unknown fields,
unregistered defaults, invalid documents, and unreadable files are errors.

`ThemeCatalog::open`, `index`, and `load` support application-owned theme pickers.
Loading a catalog does not select a theme or start a subscription. A catalog is a
snapshot; reopen it to refresh the list. Applications persist their own manual
selection separately from the catalog.

## Theme sessions

`ThemeSession::install(repository, source, cx)` loads the source and injects the
runtime before constructing components. Retain the returned entity while using
the source. `runtime()` gives access to its `ThemeRuntime`.

| `ThemeSource` | Selection |
| --- | --- |
| `Follow { app_id }` | Application catalog default, then global catalog default |
| `Application { app_id, theme_id }` | A specific theme from the application's catalog |
| `Global { theme_id }` | A specific theme from the global catalog |

A missing application index or an index without a default allows `Follow` to use
the global default. If neither catalog selects a default, `Follow` uses the built-in
theme. An invalid index or a selected theme that cannot load produces an error;
it does not silently select another theme. Explicit selections never fall back.

`set_source` changes the selection, and `reload` reads it again. Updates validate
the complete theme and current component recipes before replacing the active
configuration. Failed reloads preserve the last valid appearance and expose an
error through `ThemeSession::error`. Failed selection changes preserve the previous
source. Component instance styles, scopes, values, and focus remain intact. Direct
runtime changes to the mode or material last until the source is applied again.

Selection and synchronization are independent. The default `theme-watch` feature
provides `start_watching` and `stop_watching`; no subscription starts until requested.
Watching a pinned theme updates its files without following catalog default changes.
Subscriptions use native filesystem events, including atomic file replacement and
creation of missing theme directories, without periodic polling. Change-triggered
file reads run on the background executor. Unchanged effective configurations do
not notify components. Dropping the session or stopping its subscription cancels
pending automatic updates. Disable default features to omit the watcher dependency.

Run `cargo run -p gpui-fusion --example themes` to compare following and fixed-material
controls. The example uses application ID `org.gpui.fusion.themes`, lists indexed
themes at startup, and offers selection, reload, and subscription controls. It uses
`GPUI_FUSION_THEME` with the same root-directory semantics as the library.

## JSON and validation

The Rust data model is serializable as standard JSON. `Theme::from_json` parses
and validates the theme's declared modes, references, and inheritance.
`ComponentStyles::from_json` parses component recipes separately; `validate`
checks all recipe branches against a theme.
`StylePatch::from_json` parses a patch; call `validate` with its resolution context
and component part schema to validate all branches, including inactive states.
Unknown fields are rejected. Parse errors include line and column information;
resolution errors identify the failing property or token.

`Theme::json_schema` and `StylePatch::json_schema` generate JSON Schema from the
same model. Schema describes structural types; reference resolution and numeric
property constraints are checked at runtime. Expression and style traversal are
limited to 64 levels. Theme inheritance and token cycles are rejected.

## GPUI integration

`ComputedStyle::to_gpui` produces a fresh `gpui::StyleRefinement`. The supported
properties are width, height, text size, padding, gap, radius, border width and color,
background, foreground, and opacity. Length properties currently use logical
pixels. A cleared foreground leaves the fresh refinement unset so GPUI text
inheritance applies. Cleared backgrounds and borders use transparent colors;
cleared dimensions return to auto sizing.

`changes_from` classifies differences as layout or paint changes. Layout changes
also require paint. Theme resolution does not rebuild component state, schedule
frames, or manage caches. Consumers decide invalidation and reuse policy using
the computed differences and theme revision.

## Buttons

`Button::new` creates a persistent control inside `cx.new`. It takes a label,
window, and entity context. The theme and default style set come from the injected
`ThemeRuntime`. The
`button` recipe declares `label` and `icon` parts. `variant` selects a named
recipe variant; `scope` adds local token overrides. `ButtonPart` provides typed
part selectors through Fusion's `Styled` trait.

`default_theme()` supplies the `neutral` style set in `light` and `dark` modes.
Its base button uses a neutral surface and solid border. `primary`, `dashed`,
`text`, and `link` variants use ordinary painting. Surface-bearing buttons can
follow the theme material or select a fixed material; text and link buttons stay
surface-free. The `link` variant changes appearance only; applications
supply navigation through the activation callback.
`Material::Plain` explicitly removes an inherited material. Glass parameters
accept typed expressions, including theme tokens, and serialize with the recipe.
Material resolution failures are available through `Button::style_error`.

`on_click` receives mouse, keyboard, and accessibility activations. Disabling a
button clears pending visual press state and removes activation handlers and its
tab stop. Labels and monochrome SVG icons render above the material and remain
sharp. `icon` takes a GPUI asset path; the host supplies the asset source.

Material transitions interpolate blur, refraction, edge thickness, and frost.
Glass contours expand with separate horizontal and vertical damped springs on
hover, grow on press, and return on release or pointer leave. Directional edge
lighting follows the pointer. Foreground content, layout, and hit regions retain
their original geometry. Animations stop requesting frames on completion.
`transition_duration(Duration::ZERO)` disables material transitions and elastic
motion, and border flow. Other properties update immediately. Backdrop
sampling requires content painted earlier in the same window.

Run the component example with `cargo run -p gpui-fusion --example buttons`.

`border_dashed()` and `border_solid()` select border rendering independently of
width and color. The `border_style` declaration serializes as `dashed` or `solid`
and follows normal style-layer precedence.

## Gradient surfaces and borders

`bg_gradient` and `border_gradient` accept a `Gradient` with an angle and between
2 and 64 ordered stops. Each stop contains a position in `0..=1` and a typed color
expression, so gradients follow theme changes and local token scopes. Background
angles control the linear fill direction; border angles set the initial phase
around the contour. Gradient declarations serialize as JSON with the style.

Setting `bg` or `clear_bg` removes an inherited background gradient. Setting
`border_color` removes an inherited border gradient. Gradient setters replace the
corresponding solid declaration. State rules and style layers use the same
precedence as other properties.

`BorderMotion::Flow` advances the border gradient around the contour at the
configured positive `seconds_per_cycle`. Place it in a `pressed` rule for motion
only while pressed; `BorderMotion::Still` removes inherited motion. Disabled
buttons, zero-width borders, and controls without a border gradient do not request
flow frames. Setting the transition duration to zero disables flow as well.

Glass buttons paint their fill and border over the backdrop material, with labels
and icons above both. Use translucent gradient colors to retain visible backdrop
refraction. Plain buttons use the same fill and border declarations without a
glass pass.

The `buttons` example switches material, fill, border, shape, enabled state, and
theme on persistent preview controls. A dedicated optical control remains in the preview alongside the switchable
buttons; surface-free controls occupy a separate section. `Button::set_style` replaces the instance patch and
`Button::set_variant` changes its recipe variant without replacing the entity.


## Switches

`Switch::new` creates a persistent binary control with an accessible label and
initial checked value. It uses the shared `ThemeRuntime` and `switch` recipe.
`on_change` receives user changes; `set_checked` updates application state without
emitting this callback. Theme updates preserve the value and keyboard focus.

The default recipe paints a solid track and thumb. Glass mode uses a
white frosted thumb at rest and an expanded refractive lens while pressed or
travelling. Track color follows the animated thumb position, including during
dragging before the checked value is committed. The root style controls the track; `SwitchPart::Thumb` controls thumb
size, radius, fill, border, and material. A glass thumb uses its material's tint
and frost colors instead of a solid fill. Selected-state rules style the checked
state. Instance styles, scopes, and variants use normal Fusion precedence.

Click, Space, Enter, and accessibility activation toggle the value. Dragging
releases to the nearest end, including outside the track. Disabling or losing
focus cancels an active drag. `animated(false)` snaps directly to the target and
keeps the resting lens geometry. Settled controls do not schedule animation frames.

`content(unchecked, checked)` accepts `SwitchContent::Text` or an SVG asset path
through `SwitchContent::Icon`. Content is centered above the thumb material,
follows its movement, and changes at the midpoint. It remains sharp and retains
its size during lens expansion. Thumb foreground and text size style both text
and monochrome icons. The host provides SVG assets. The accessible label remains
independent of this decorative content.

Run `cargo run -p gpui-fusion --example switches` to compare plain and glass
controls, change theme, and exercise disabled states.

## Sliders

`Slider::new` creates a persistent horizontal numeric control using the `slider`
recipe. Its default range is 0..=100 with a step of 1. `range(SliderRange::new(min,
max, step)?)` sets finite increasing limits; `None` selects continuous values.
Steps are anchored at the minimum and both endpoints remain reachable. Initial
values are clamped and snapped to the configured range; nonfinite initial values
use the default minimum. Nonfinite `set_value` calls are ignored.

`on_change` receives distinct user values during dragging, track clicks, or
keyboard adjustment. `set_value` and `set_range` update application state without
emitting it. Disabling, losing focus, or an application value update ends the
current drag. Theme changes preserve the value and focus.

Arrow keys adjust one step, Page Up/Down adjust at least one step or ten percent
of the range, and Home/End select the limits. Continuous ranges use one percent
for keyboard increments. Accessibility exposes the label, numeric limits,
current value, and increment/decrement actions.

The root style controls the interaction area. `SliderPart::Track` controls rail
height and paint, `Fill` controls the filled rail paint, and `Thumb` controls thumb
size, radius, border, and material. Set the thumb part to `border(px(0.))`
to remove its outline, or combine `border` with `border_color` to customize it.
The optical material's edge highlight is independent of this painted outline.
Fill follows the animated position. Glass mode rests as a frosted thumb. Pressing expands it into a clear
refractive lens; releasing restores the frosted surface. A glass thumb uses
material tint and frost colors instead of its solid background. `animated(false)`
changes the position and pressed geometry immediately. Settled sliders do not
request animation frames; theme resolution is cached independently of the value.

Run `cargo run -p gpui-fusion --example sliders` to compare standard and glass
controls with continuous or stepped interaction and light/dark themes.

## Segmented controls

`Segmented::new(label, items, window, cx)` creates an equal-width radio group.
Each `Segment` has a unique nonempty value and an accessible label. `icon` adds
an SVG; `icon_only` hides the visual label while preserving its accessible name.
Individual items can be disabled. The first enabled item is initially selected;
empty and fully disabled groups have no selection and are excluded from tab order.

`set_selected` selects an enabled value without calling `on_change`. User clicks,
completed drags, arrow keys, Home, and End emit one callback when selection changes.
Keyboard navigation skips disabled items and wraps. Escape, focus loss, disabling,
or losing the pressed mouse button cancels a drag without committing it.

The `segmented` recipe declares `Item` and `Indicator` parts through `SegmentedPart`.
Root width, height, and padding determine the shared cell geometry. Material choice
follows the theme unless fixed with `material_mode`. Glass rests as a frosted pill
with crisp foreground content; holding or moving it reveals a refractive lens.
The indicator's foreground colors content inside the active lens. Its border can
supply a focus outline. `animated(false)` applies position and pressure immediately.
Settled controls do not request animation frames.

Run `cargo run -p gpui-fusion --example segmented` for text, icon, mixed-content,
and fixed-material controls with theme and material switching.

## Inputs

Create an `InputState` entity for the editable value, then an
`Input::new(&state, window, cx)` entity for the themed field. The state supports
single-line text, `password()`, and soft-wrapping `multiline()` input. Set
`aria_label` independently of the placeholder. `read_only(true)` retains
selection and copying; `disabled(true)` prevents interaction and tab focus.

The `input` recipe controls the surface through Fusion `Styled`. `InputPart`
provides `Placeholder` (text color), `Caret` (color, width, optional height),
`Selection` (background), and `Scrollbar` (background, hover/pressed background,
radius, minimum thumb height, focus border), `ActionButton` (built-in icon color, size, radius, and focus border), and `ResizeHandle` (grip color). The `invalid` variant uses error
colors. `rows` sets the multiline viewport height unless `h` is explicit.
`prefix` and `suffix` build adornments inside the field's focus boundary.

Colors and material follow the injected theme. `material_mode` fixes the field's
material while retaining theme colors; `set_material_mode(None, cx)` resumes
following it. Glass uses a frosted background beneath the text, selection, and
caret. The default glass field uses its optical edge without an additional outline;
focus increases edge lighting. `glass_edge_color` colors the optical edge independently
of the body tint, including in state and variant rules. The shared glass material
resolves `edge_color` from the `glass_edge_color` theme token by default. Explicit instance
border colors or gradients can provide an outline. The material's
`edge_light` value scales rim, reflection, and caustic lighting in `0..=1`.
Disabled fields use the neutral disabled surface and text colors.
Theme changes preserve the editor entity, focus, selection, and IME
composition. Resolved styles and unchanged text layouts are cached; the field
does not animate the material while idle.

`on_change` receives committed edits and programmatic `set_value` changes, but
not IME pre-edit updates. `value()` includes visible pre-edit text. Enter submits
single-line fields; multiline fields insert a newline and use Ctrl/Cmd+Enter to
submit. The state also emits `InputEvent` and configured software-keyboard
`InputActionEvent` events. Input bindings are installed once when a field is
created. While a multiline editor has focus, wheel and touchpad scrolling over
the field stays within the field, including at its scroll boundaries.

Multiline fields can opt into a resize grip with `resizable(true)`. `resize` selects
`InputResize::Vertical`, `Horizontal`, `Both`, or `Disabled`. `resize_limits` sets
the minimum and maximum outer dimensions in logical pixels. Dragging preserves the
value, selection, and composition. Disabled and single-line fields have no grip.
`set_resizable` changes availability without discarding the current size;
`reset_size` restores sizing from `rows` and the style. The grip is disabled by
default.

Password fields include an eye/eye-off button with mouse, keyboard, and accessibility
activation. `password_toggle(false)` omits it; `set_password_visible` changes it
programmatically. Revealing the value preserves the selection and composition.
Selected text can be copied while the password is visible, including in read-only
fields. Masked or disabled fields block copying. Password cutting, surrounding-text,
and history protections remain active.
Disabling the editor hides the value again.

Masked passwords disable desktop IME composition by default while allowing direct
key input and paste. `InputState::password_ime(true)` opts into composition while
masked; `set_password_ime(enabled, cx)` changes the policy at runtime. Revealed
passwords allow composition. Hiding a password with masked IME disabled, or
turning off masked IME during composition, cancels the pre-edit without committing
it. Read-only and disabled fields never enable IME.

`clearable(true)` adds an undoable clear action when editable text is nonempty.
`icon(side, InputIcon::new(path))` sets a leading or trailing SVG. Icons are
noninteractive by default; `InputIcon::on_click(label, callback)` makes an icon
an accessible button with mouse and keyboard activation. `search_icon(side)` is
a shortcut using the same icon slot and rendering: its action emits
`InputEvent::Submit` and invokes `on_submit` when composition is inactive.
`prefix`, `suffix`, and `icon` replace the content of their respective slot;
the last setter wins. Clear and password buttons remain independent.
All built-in buttons honor disabled state. Install `gpui_icons::IconAssets` in
the application's asset source to provide the bundled SVG assets.

Ctrl/Cmd+Z undoes and Ctrl/Cmd+Shift+Z or Ctrl/Cmd+Y redoes. A complete IME
composition is one edit. History is bounded to 100 entries and 1 MiB of text;
password fields retain no history. Programmatic
value replacement clears history. Switching to read-only or disabled cancels
unfinished composition without emitting a committed edit.

Run `cargo run -p gpui-fusion --example inputs` for theme/material switching,
multiline, password, read-only, disabled, and validation styling.
