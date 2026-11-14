# Styles and themes

Import `gpui_fusion::prelude::Styled` for Fusion's symbolic style builders.
Components implement this trait by exposing their `StylePatch` and a typed
`Part` identifier. GPUI's `Styled` is used only by the final style adapter.

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
resolution applies base, variant, then instance. Across inherited style sets,
parent recipe layers precede child recipe layers; instance overrides remain last.
A requested variant may be inherited. Unknown variants and undeclared parts are
errors. Components must validate recipes against their actual supported parts
before accepting externally supplied part declarations.

Parts resolve independently from root properties. Repeated `part` builders
refine the existing patch. State rules may target parts, and parts may have their
own state rules. Nested parts are not supported. Input handling supplies the
active state set; style resolution does not change component behavior.

## Themes and local scopes

`Theme` contains named modes and named `StyleSet` values. Each style set can
inherit from one parent and register tokens and component recipes. A token's
mode-specific value precedes its default. A definition without either continues
lookup through outer scopes and parent style sets.

`ResolveContext` binds a theme, mode, and selected style set. `scoped` introduces
local token overrides and optionally selects another style set. Inner scopes
precede outer scopes. Inherited aliases resolve against the selected child's
overrides. Recipes are selected through `resolve_recipe`.

`ThemeStore` owns a validated theme and active mode. Successful changes increment
its revision. `replace_json` validates the replacement before changing either the
theme or revision, and rejects replacements that omit the active mode. Host
components must notify dependent views after a successful update and resolve
styles again; the store does not install GPUI observers or file watchers.

## JSON and validation

The Rust data model is serializable as standard JSON. `Theme::from_json` parses
and validates the theme's declared modes, references, inheritance, and recipes.
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
properties are width, height, padding, gap, radius, border width and color,
background, foreground, and opacity. Length properties currently use logical
pixels. A cleared foreground leaves the fresh refinement unset so GPUI text
inheritance applies. Cleared backgrounds and borders use transparent colors;
cleared dimensions return to auto sizing.

`changes_from` classifies differences as layout or paint changes. Layout changes
also require paint. Theme resolution does not rebuild component state, schedule
frames, or manage caches. Consumers decide invalidation and reuse policy using
the computed differences and theme revision.
