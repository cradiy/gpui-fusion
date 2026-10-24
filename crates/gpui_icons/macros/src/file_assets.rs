use crate::{FileEnumArgs, configured_assets};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{Fields, ItemStruct};

pub(super) fn expand(
    args: FileEnumArgs,
    input: ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    if !matches!(input.fields, Fields::Unit)
        || !input.generics.params.is_empty()
        || input.generics.where_clause.is_some()
    {
        return Err(syn::Error::new_spanned(
            input,
            "file_assets requires a non-generic unit struct",
        ));
    }
    let assets = configured_assets(&args, false)?;
    let name = &input.ident;
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    let mut paths = Vec::new();
    for asset in &assets {
        let (prefix, filename) = asset
            .asset_path
            .rsplit_once('/')
            .unwrap_or(("", &asset.asset_path));
        groups.entry(prefix).or_default().push((filename, asset));
        let guard = asset
            .feature
            .as_ref()
            .map(|feature| quote!(#[cfg(feature = #feature)]));
        let path = &asset.asset_path;
        paths.push(quote!(#guard #path));
    }
    let mut dispatch = Vec::new();
    let mut loaders = Vec::new();
    for (index, (prefix, entries)) in groups.into_iter().enumerate() {
        let function = format_ident!("load_group_{index}");
        dispatch.push(quote!(#prefix => Self::#function(filename)));
        let mut arms = Vec::new();
        for (filename, asset) in entries {
            let source = asset.source.to_string_lossy().into_owned();
            let enabled = quote!(#filename => Ok(Some(include_bytes!(#source))));
            if let Some(feature) = &asset.feature {
                let message = format!(
                    "resource `{}` is not bundled; enable feature `{feature}`",
                    asset.asset_path
                );
                arms.push(quote!(
                    #[cfg(feature = #feature)] #enabled,
                    #[cfg(not(feature = #feature))] #filename => Err(#message),
                ));
            } else {
                arms.push(quote!(#enabled,));
            }
        }
        loaders.push(quote!(
            fn #function(filename: &str) -> ::core::result::Result<Option<&'static [u8]>, &'static str> {
                match filename { #(#arms)* _ => Ok(None) }
            }
        ));
    }
    Ok(quote!(
        #input
        impl #name {
            /// Load bundled bytes, return None for unknown paths, or report a disabled feature.
            pub fn get(path: &str) -> ::core::result::Result<Option<&'static [u8]>, &'static str> {
                let (prefix, filename) = path.rsplit_once('/').unwrap_or(("", path));
                match prefix { #(#dispatch,)* _ => Ok(None) }
            }

            /// List paths whose resources are embedded in this build.
            pub fn iter() -> impl ::core::iter::Iterator<Item = &'static str> {
                const PATHS: &[&str] = &[#(#paths),*];
                PATHS.iter().copied()
            }

            #(#loaders)*
        }
    ))
}
