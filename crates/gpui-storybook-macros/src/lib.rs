use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemFn, ItemStruct, LitStr, Token, parse::Parse, parse::ParseStream};

enum SectionArg {
    StringLiteral(String),
    EnumVariant(syn::Path),
}

struct StoryArgs {
    section: Option<SectionArg>,
}

impl Parse for StoryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(StoryArgs { section: None });
        }

        // Try to parse as string literal first
        if input.peek(LitStr) {
            let section_lit: LitStr = input.parse()?;
            // Handle optional trailing comma
            let _ = input.parse::<Token![,]>();
            return Ok(StoryArgs {
                section: Some(SectionArg::StringLiteral(section_lit.value())),
            });
        }

        // Otherwise try to parse as path (enum variant)
        let path: syn::Path = input.parse()?;
        // Handle optional trailing comma
        let _ = input.parse::<Token![,]>();

        Ok(StoryArgs {
            section: Some(SectionArg::EnumVariant(path)),
        })
    }
}

fn story_impl(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let args: StoryArgs = syn::parse2(args).expect("failed to parse story macro arguments");
    let input_struct: ItemStruct = syn::parse2(input).expect("story macro expects a struct");
    let struct_name = &input_struct.ident;
    let struct_name_str = struct_name.to_string();

    let (section_value, section_order) = match &args.section {
        Some(SectionArg::StringLiteral(s)) => (quote! { Some(#s) }, quote! { None }),
        Some(SectionArg::EnumVariant(path)) => {
            // Extract just the variant name from the path (last segment)
            let variant_name = path
                .segments
                .last()
                .map(|seg| seg.ident.to_string())
                .unwrap_or_else(|| quote!(#path).to_string());
            (
                quote! { Some(#variant_name) },
                quote! { Some(#path as usize) },
            )
        },
        None => (quote! { None }, quote! { None }),
    };

    quote! {
        #input_struct

        gpui_storybook::__inventory::submit! {
            ::gpui_storybook::__registry::StoryEntry {
                name: #struct_name_str,
                section: #section_value,
                section_order: #section_order,
                create_fn: |window, cx| {
                    ::gpui_storybook::StoryContainer::panel::<#struct_name>(window, cx)
                },
                file: ::std::file!(),
                line: ::std::line!(),
            }
        }
    }
}

/// Attribute macro to register a story struct
///
/// Optionally accepts a section name as a string literal or enum variant:
/// ```ignore
/// // String literal (sorted alphabetically by section name)
/// #[story("Components")]
/// pub struct ButtonStory;
///
/// // Enum variant (sorted by enum discriminant order)
/// #[story(StorySection::Components)]
/// pub struct ButtonStory;
/// ```
#[proc_macro_attribute]
pub fn story(args: TokenStream, input: TokenStream) -> TokenStream {
    story_impl(args.into(), input.into()).into()
}

fn story_init_impl(_args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let input_fn: ItemFn = syn::parse2(input).expect("story_init macro expects a function");
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();

    quote! {
        #input_fn

        gpui_storybook::__inventory::submit! {
            ::gpui_storybook::__registry::InitEntry {
                init_fn: #fn_name,
                fn_name: #fn_name_str,
                file: ::std::file!(),
                line: ::std::line!(),
            }
        }
    }
}

/// Attribute macro to register an init function
#[proc_macro_attribute]
pub fn story_init(_args: TokenStream, input: TokenStream) -> TokenStream {
    story_init_impl(_args.into(), input.into()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use prettyplease::unparse;
    use quote::quote;

    fn snapshot_tokens(tokens: TokenStream2) -> String {
        let file =
            syn::parse2::<syn::File>(tokens).expect("generated code should be valid Rust syntax");
        unparse(&file)
    }

    #[test]
    fn story_generates_registry_entry() {
        let input = quote! {
            pub struct ButtonStory;
        };

        let expanded = story_impl(TokenStream2::new(), input);
        assert_snapshot!(
            "story_attribute_generates_registry_entry",
            snapshot_tokens(expanded)
        );
    }

    #[test]
    fn story_with_section_generates_registry_entry() {
        let args = quote! { "Components" };
        let input = quote! {
            pub struct ButtonStory;
        };

        let expanded = story_impl(args, input);
        assert_snapshot!(
            "story_attribute_with_section_generates_registry_entry",
            snapshot_tokens(expanded)
        );
    }

    #[test]
    fn story_init_generates_init_entry() {
        let input = quote! {
            pub fn setup() {}
        };

        let expanded = story_init_impl(TokenStream2::new(), input);
        assert_snapshot!(
            "story_init_attribute_generates_registry_entry",
            snapshot_tokens(expanded)
        );
    }
}
