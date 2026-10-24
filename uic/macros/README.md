# uic-macros

`uic-macros` provides general-purpose procedural macros that can be used
independently of GPUI.

## `Chainable`

`Chainable` generates consuming, same-name setter methods for a struct's named
fields. The generated method has the same visibility as its field and accepts
the exact field type:

```rust
use uic_macros::Chainable;

#[derive(Default, Chainable)]
struct Appearance {
    pub background: u32,
    pub foreground: u32,
    #[chain(skip)]
    cached_value: u32,
}

let appearance = Appearance::default()
    .background(0x171a24)
    .foreground(0xf4f4f7);
```

Use `#[chain(skip)]` for internal fields, validated values with hand-written
setters, or names that would conflict with an existing method. The derive works
with generic structs and preserves their `where` clauses.
