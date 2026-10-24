mod file_assets;

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use heck::{ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    Ident, ItemEnum, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Generates an enum from the files in a directory.
///
/// Paths are relative to the consuming crate's `CARGO_MANIFEST_DIR`.
///
/// ```ignore
/// #[file_enum(
///     path = "assets/icons",
///     ext = "svg",
///     rename_all = "PascalCase",
/// )]
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// pub enum Icon {}
/// ```
///
/// Each matching file directly inside `path` becomes a unit variant, and
/// `Display` returns the original file name including its extension. Extension
/// matching is case-sensitive.
///
/// Directory additions require touching the annotated source to trigger
/// expansion on stable Rust.
#[proc_macro_attribute]
pub fn file_enum(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as FileEnumArgs);
    let input = parse_macro_input!(item as ItemEnum);

    expand_asset_enum(args, input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates feature-selected embedded resources from explicitly configured directories.
#[proc_macro_attribute]
pub fn file_assets(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as FileEnumArgs);
    let input = parse_macro_input!(item as syn::ItemStruct);
    file_assets::expand(args, input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct FileEnumArgs {
    path: LitStr,
    groups: Vec<AssetGroup>,
    ext: LitStr,
    rename_all: RenameRule,
    rename_all_span: Span,
}

impl Parse for FileEnumArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut path = None;
        let mut groups: Vec<AssetGroup> = Vec::new();
        let mut ext = None;
        let mut rename_all = None;

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            if key == "group" {
                let content;
                syn::parenthesized!(content in input);
                groups.push(content.parse()?);
                if input.is_empty() {
                    break;
                }
                input.parse::<Token![,]>()?;
                continue;
            }
            input.parse::<Token![=]>()?;
            let value = input.parse::<LitStr>()?;

            match key.to_string().as_str() {
                "path" => set_once(&mut path, value, &key)?,
                "ext" => set_once(&mut ext, value, &key)?,
                "rename_all" => {
                    let span = value.span();
                    let rule = RenameRule::parse(&value)?;
                    set_once(&mut rename_all, (rule, span), &key)?;
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "unknown option; expected `path`, `group`, `ext`, or `rename_all`",
                    ));
                }
            }

            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        if path.is_some() == !groups.is_empty() {
            return Err(input.error("provide either `path` or one or more `group(...)` entries"));
        }
        let path = path.unwrap_or_else(|| groups[0].path.clone());
        let (rename_all, rename_all_span) = rename_all.unwrap_or((RenameRule::Pascal, path.span()));
        Ok(Self {
            path,
            groups,
            ext: ext.ok_or_else(|| input.error("missing required option `ext`"))?,
            rename_all,
            rename_all_span,
        })
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, key: &Ident) -> syn::Result<()> {
    if slot.replace(value).is_some() {
        return Err(syn::Error::new(
            key.span(),
            format!("duplicate option `{key}`"),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RenameRule {
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
}

impl RenameRule {
    fn parse(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "PascalCase" | "UpperCamelCase" => Ok(Self::Pascal),
            "camelCase" | "lowerCamelCase" => Ok(Self::Camel),
            "snake_case" => Ok(Self::Snake),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
            _ => Err(syn::Error::new(
                value.span(),
                "unsupported naming rule; expected `PascalCase`, `camelCase`, \
                 `snake_case`, or `SCREAMING_SNAKE_CASE`",
            )),
        }
    }

    fn apply(self, stem: &str) -> String {
        match self {
            Self::Pascal => stem.to_upper_camel_case(),
            Self::Camel => stem.to_lower_camel_case(),
            Self::Snake => stem.to_snake_case(),
            Self::ScreamingSnake => stem.to_shouty_snake_case(),
        }
    }
}

struct FileEntry {
    variant: Ident,
    file_name: String,
    asset_path: String,
    source: PathBuf,
    feature: Option<String>,
}

fn expand_asset_enum(
    args: FileEnumArgs,
    mut input: ItemEnum,
) -> syn::Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "file enums cannot have generic parameters",
        ));
    }
    if !input.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.variants,
            "file enum must be empty; variants are generated from the directory",
        ));
    }

    let assets = configured_assets(&args, true)?;
    if assets.is_empty() {
        return Err(syn::Error::new(
            args.path.span(),
            format!(
                "no files with extension `.{}` found in `{}`",
                normalized_extension(&args.ext)?,
                args.path.value()
            ),
        ));
    }

    input.variants = assets
        .iter()
        .map(|asset| -> syn::Variant {
            let variant = &asset.variant;
            let doc = match &asset.feature {
                Some(feature) => format!("Requires feature `{feature}` to bundle this resource."),
                None => "No feature requirement for embedding this resource.".into(),
            };
            syn::parse_quote!(#[doc = #doc] #variant)
        })
        .collect::<Punctuated<_, Token![,]>>();

    let enum_name = &input.ident;
    let variants: Vec<_> = assets.iter().map(|asset| &asset.variant).collect();
    let path_arms = assets.iter().map(|asset| {
        let variant = &asset.variant;
        let path = &asset.asset_path;
        quote!(Self::#variant => #path)
    });
    let feature_arms = assets.iter().map(|asset| {
        let variant = &asset.variant;
        match &asset.feature {
            Some(feature) => quote!(Self::#variant => Some(#feature)),
            None => quote!(Self::#variant => None),
        }
    });
    let display_arms = assets.iter().map(|asset| {
        let variant = &asset.variant;
        let file_name = &asset.file_name;
        quote!(Self::#variant => formatter.write_str(#file_name))
    });

    Ok(quote! {
        #input

        impl #enum_name {
            /// Every generated name, including resources not enabled for embedding.
            pub const ALL: &'static [Self] = &[#(Self::#variants),*];
            pub const fn path(&self) -> &'static str {
                match self { #(#path_arms),* }
            }
            pub const fn required_feature(&self) -> Option<&'static str> {
                match self { #(#feature_arms),* }
            }
        }

        impl ::core::fmt::Display for #enum_name {
            fn fmt(
                &self,
                formatter: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                match self {
                    #(#display_arms),*
                }
            }
        }

    })
}

fn collect_assets(args: &FileEnumArgs, enum_names: bool) -> syn::Result<Vec<FileEntry>> {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "`CARGO_MANIFEST_DIR` is unavailable while expanding asset enum",
        )
    })?;
    let configured_path = Path::new(&args.path.value()).to_path_buf();
    let directory = if configured_path.is_absolute() {
        configured_path
    } else {
        PathBuf::from(manifest_dir).join(configured_path)
    };
    let extension = normalized_extension(&args.ext)?;

    let entries = fs::read_dir(&directory).map_err(|error| {
        syn::Error::new(
            args.path.span(),
            format!(
                "failed to read asset directory `{}`: {error}",
                directory.display()
            ),
        )
    })?;

    let mut assets = Vec::new();
    let mut variants = HashMap::<String, String>::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            syn::Error::new(
                args.path.span(),
                format!(
                    "failed to read an entry in `{}`: {error}",
                    directory.display()
                ),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            syn::Error::new(
                args.path.span(),
                format!("failed to inspect `{}`: {error}", entry.path().display()),
            )
        })?;
        if !file_type.is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some(extension.as_str())
        {
            continue;
        }

        let file_name = entry.file_name().into_string().map_err(|_| {
            syn::Error::new(
                args.path.span(),
                format!("file name in `{}` is not valid UTF-8", directory.display()),
            )
        })?;
        let stem = entry
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                syn::Error::new(
                    args.path.span(),
                    format!("file `{file_name}` has no valid UTF-8 stem"),
                )
            })?
            .to_owned();
        let variant_name = args.rename_all.apply(&stem);
        let variant = if enum_names {
            syn::parse_str::<Ident>(&variant_name).map_err(|_| {
            syn::Error::new(
                args.rename_all_span(),
                format!(
                    "file `{file_name}` becomes `{variant_name}`, which is not a valid Rust enum variant"
                ),
            )
        })?
        } else {
            Ident::new("Resource", Span::call_site())
        };

        if enum_names
            && let Some(previous) = variants.insert(variant_name.clone(), file_name.clone())
        {
            return Err(syn::Error::new(
                args.rename_all_span(),
                format!(
                    "files `{previous}` and `{file_name}` both become enum variant `{variant_name}`"
                ),
            ));
        }

        assets.push(FileEntry {
            variant,
            asset_path: file_name.clone(),
            file_name,
            source: entry.path(),
            feature: None,
        });
    }

    assets.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(assets)
}

fn normalized_extension(ext: &LitStr) -> syn::Result<String> {
    let value = ext.value();
    let extension = value.strip_prefix('.').unwrap_or(&value);
    if extension.is_empty() || extension.contains(['/', '\\']) {
        return Err(syn::Error::new(
            ext.span(),
            "`ext` must be a non-empty file extension without path separators",
        ));
    }

    Ok(extension.to_owned())
}

impl FileEnumArgs {
    fn rename_all_span(&self) -> Span {
        self.rename_all_span
    }
}

struct AssetGroup {
    path: LitStr,
    variant_prefix: String,
    asset_prefix: String,
    feature: Option<String>,
}

impl Parse for AssetGroup {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut path = None;
        let mut variant_prefix = None;
        let mut asset_prefix = None;
        let mut feature = None;
        while !input.is_empty() {
            let key = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;
            let value = input.parse::<LitStr>()?;
            match key.to_string().as_str() {
                "path" => set_once(&mut path, value, &key)?,
                "variant_prefix" => set_once(&mut variant_prefix, value.value(), &key)?,
                "asset_prefix" => set_once(&mut asset_prefix, value.value(), &key)?,
                "feature" => set_once(&mut feature, value.value(), &key)?,
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "expected path, variant_prefix, asset_prefix, or feature",
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            path: path.ok_or_else(|| input.error("group requires path"))?,
            variant_prefix: variant_prefix.unwrap_or_default(),
            asset_prefix: asset_prefix.unwrap_or_default(),
            feature,
        })
    }
}

fn configured_assets(args: &FileEnumArgs, enum_names: bool) -> syn::Result<Vec<FileEntry>> {
    if args.groups.is_empty() {
        return collect_assets(args, enum_names);
    }
    let mut result = Vec::new();
    let mut variants = HashMap::new();
    let mut paths = HashMap::new();
    for group in &args.groups {
        let span = group.path.span();
        if !group.asset_prefix.is_empty()
            && group
                .asset_prefix
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\\'))
        {
            return Err(syn::Error::new(
                span,
                "asset_prefix must be a relative slash-separated path",
            ));
        }
        if group
            .feature
            .as_ref()
            .is_some_and(|feature| feature.is_empty())
        {
            return Err(syn::Error::new(span, "feature must not be empty"));
        }
        let entries = collect_assets(
            &FileEnumArgs {
                path: group.path.clone(),
                groups: Vec::new(),
                ext: args.ext.clone(),
                rename_all: args.rename_all,
                rename_all_span: args.rename_all_span,
            },
            enum_names,
        )?;
        if entries.is_empty() {
            return Err(syn::Error::new(
                span,
                format!("no matching resources in {}", group.path.value()),
            ));
        }
        for mut entry in entries {
            if enum_names {
                let name = format!("{}{}", group.variant_prefix, entry.variant);
                entry.variant = syn::parse_str(&name)
                    .map_err(|_| syn::Error::new(span, format!("invalid variant {name}")))?;
                if variants
                    .insert(name.clone(), entry.source.clone())
                    .is_some()
                {
                    return Err(syn::Error::new(span, format!("duplicate variant {name}")));
                }
            }
            if !group.asset_prefix.is_empty() {
                entry.asset_path = format!("{}/{}", group.asset_prefix, entry.file_name);
            }
            entry.feature = group.feature.clone();
            if paths
                .insert(entry.asset_path.clone(), entry.source.clone())
                .is_some()
            {
                return Err(syn::Error::new(
                    span,
                    format!("duplicate resource path {}", entry.asset_path),
                ));
            }
            result.push(entry);
        }
    }
    Ok(result)
}
