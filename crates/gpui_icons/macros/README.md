# GPUI icon macros

Procedural macros for typed resource names and embedded bytes. They have no GPUI
runtime dependency. Applications can import them through `gpui_icons` or depend
on `gpui-icons-macros` directly.

## `file_enum`

`file_enum` scans one directory at compile time and generates a unit variant for
each file with the requested extension:

```rust
use gpui_icons_macros::file_enum;

#[file_enum(
    path = "assets/icons",
    ext = "svg",
    rename_all = "PascalCase",
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {}
```

For files named `arrow-left.svg` and `volume-2.svg`, this creates
`Icon::ArrowLeft` and `Icon::Volume2`. The generated `Display` implementation
returns `arrow-left.svg` and `volume-2.svg`, including the extension.

`path` is resolved relative to the consuming crate's `CARGO_MANIFEST_DIR`.
`rename_all` is optional and defaults to `PascalCase`. Supported rules are:

- `PascalCase` or `UpperCamelCase`
- `camelCase` or `lowerCamelCase`
- `snake_case`
- `SCREAMING_SNAKE_CASE`

The enum must be empty and cannot have generic parameters. The macro reports
invalid Rust identifiers and collisions caused by name conversion as compile
errors.

Directory additions require touching the annotated source or rebuilding the consuming
crate on stable Rust. Enabled embedded files are tracked by `include_bytes!`.

## Explicit groups

Instead of `path`, supply one or more `group(...)` entries in either macro.
`ext` and `rename_all` remain top-level options.

| Group option | Meaning |
| --- | --- |
| `path = "..."` | Required directory, relative to the consuming crate root |
| `variant_prefix = "..."` | Prefix added to each generated enum member; empty by default |
| `asset_prefix = "..."` | Relative resource namespace, without a trailing slash; empty by default |
| `feature = "..."` | Optional Cargo feature controlling embedding |

Each group scans its direct files. Declare feature names in the consuming crate's
Cargo.toml. Features do not hide enum members: each member's documentation names
its requirement. `ALL`, `path()`, and `required_feature()` provide name metadata;
the latter returns `None` for unconditional resources. No SVG bytes are included
by `file_enum`.

Duplicate generated enum names and duplicate resource paths are rejected.
Different asset prefixes allow collections to contain identical file names.

## `file_assets`

Apply `file_assets` to a non-generic unit struct with `path` or explicit `group`
configuration. It does not require an enum or a variant prefix. The generated
struct provides:

- `get(path: &str) -> Result<Option<&'static [u8]>, &'static str>`: dispatches by
  resource prefix. Unknown paths return `None`; known disabled resources return
  an error naming the required feature.
- `iter() -> impl Iterator<Item = &'static str>`: lists enabled resource paths.

Only enabled resource bytes are embedded through `include_bytes!`. Both macros
can be used independently, including with different enum and embedding scopes.
Keep their asset prefixes consistent when using generated enum paths to load
resources. No GPUI or error-handling crate is required by the generated code.
