use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{ItemFn, ItemStruct};

fn story_impl(_args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let input_struct: ItemStruct = syn::parse2(input).expect("story macro expects a struct");
    let struct_name = &input_struct.ident;
    let struct_name_str = struct_name.to_string();

    quote! {
        #input_struct

        gpui_storybook::__inventory::submit! {
            ::gpui_storybook::__registry::StoryEntry {
                name: #struct_name_str,
                create_fn: |window, cx| {
                    ::gpui_storybook::StoryContainer::panel::<#struct_name>(window, cx)
                },
            }
        }
    }
}

/// Attribute macro to register a story struct
#[proc_macro_attribute]
pub fn story(_args: TokenStream, input: TokenStream) -> TokenStream {
    story_impl(_args.into(), input.into()).into()
}

fn story_init_impl(_args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let input_fn: ItemFn = syn::parse2(input).expect("story_init macro expects a function");
    let fn_name = &input_fn.sig.ident;

    quote! {
        #input_fn

        inventory::submit! {
            ::gpui_storybook::__registry::InitEntry {
                init_fn: #fn_name,
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
