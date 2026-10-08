use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn main(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "main takes no attribute arguments",
        )
        .to_compile_error()
        .into();
    }
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
    quote! {
        #function

        #[cfg(target_os = "android")]
        ::gpui_platform::__android_entry!(main);
    }
    .into()
}
