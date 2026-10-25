use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, Ident, ItemFn, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

#[derive(Default)]
struct MainArgs {
    android_log_level: Option<Expr>,
}

impl Parse for MainArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = Self::default();
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            if key != "android_log_level" {
                return Err(syn::Error::new_spanned(key, "expected android_log_level"));
            }
            if options.android_log_level.is_some() {
                return Err(syn::Error::new_spanned(key, "duplicate android_log_level"));
            }
            input.parse::<Token![=]>()?;
            options.android_log_level = Some(input.parse()?);
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(options)
    }
}

pub fn main(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as MainArgs);
    let function = parse_macro_input!(input as ItemFn);
    let signature = &function.sig;
    if signature.ident != "main"
        || !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.constness.is_some()
        || signature.abi.is_some()
    {
        return syn::Error::new_spanned(signature, "expected a synchronous fn main()")
            .to_compile_error()
            .into();
    }
    let loader = match args.android_log_level {
        Some(level) => quote! { ::gpui_platform::__android_entry!(main, #level); },
        None => quote! { ::gpui_platform::__android_entry!(main); },
    };
    quote! {
        #function

        #[cfg(target_os = "android")]
        #loader
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::MainArgs;

    #[test]
    fn logging_options_accept_expressions_and_reject_ambiguous_configuration() {
        assert!(
            syn::parse_str::<MainArgs>("")
                .unwrap()
                .android_log_level
                .is_none()
        );
        assert!(
            syn::parse_str::<MainArgs>("android_log_level = configured_level(),")
                .unwrap()
                .android_log_level
                .is_some()
        );
        assert!(
            syn::parse_str::<MainArgs>("android_log_level = a, android_log_level = b").is_err()
        );
        assert!(syn::parse_str::<MainArgs>("other = a").is_err());
    }
}
