mod chainable;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Generates consuming, same-name setter methods for every named field.
/// Use `#[chain(skip)]` on fields that should not receive a generated method.
/// Setter visibility matches the corresponding field visibility.
#[proc_macro_derive(Chainable, attributes(chain))]
pub fn derive_chainable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    chainable::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
