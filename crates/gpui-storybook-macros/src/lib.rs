use heck::ToTitleCase as _;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Expr, ExprLit, ExprPath, ItemFn, ItemStruct, Lit, LitStr, Token,
    meta::ParseNestedMeta, parse::Parse, parse::ParseStream,
};

enum SectionArg {
    StringLiteral(String),
    EnumVariant(syn::Path),
}

struct StoryArgs {
    section: Option<SectionArg>,
}

#[derive(Default)]
struct ComponentStoryArgs {
    title: Option<Expr>,
    description: Option<Expr>,
    section: Option<SectionArg>,
    example: Option<Expr>,
}

impl Parse for StoryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(StoryArgs { section: None });
        }

        if input.peek(LitStr) {
            let section_lit: LitStr = input.parse()?;
            let _ = input.parse::<Token![,]>();
            return Ok(StoryArgs {
                section: Some(SectionArg::StringLiteral(section_lit.value())),
            });
        }

        let path: syn::Path = input.parse()?;
        let _ = input.parse::<Token![,]>();

        Ok(StoryArgs {
            section: Some(SectionArg::EnumVariant(path)),
        })
    }
}

fn duplicate_attr_error(meta: &ParseNestedMeta<'_>, name: &str) -> syn::Error {
    meta.error(format!("duplicate `{name}` argument"))
}

fn parse_section_expr(expr: Expr) -> syn::Result<SectionArg> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(section_lit),
            ..
        }) => Ok(SectionArg::StringLiteral(section_lit.value())),
        Expr::Path(ExprPath { path, .. }) => Ok(SectionArg::EnumVariant(path)),
        _ => Err(syn::Error::new_spanned(
            expr,
            "`section` must be a string literal or enum variant path",
        )),
    }
}

fn section_tokens(section: Option<&SectionArg>) -> (TokenStream2, TokenStream2) {
    match section {
        Some(SectionArg::StringLiteral(section)) => (quote! { Some(#section) }, quote! { None }),
        Some(SectionArg::EnumVariant(path)) => {
            let variant_name = path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_else(|| quote!(#path).to_string());
            (
                quote! { Some(#variant_name) },
                quote! { Some(#path as usize) },
            )
        },
        None => (quote! { None }, quote! { None }),
    }
}

fn registration_tokens(
    story_type: TokenStream2,
    entry_name: &str,
    section: Option<&SectionArg>,
) -> TokenStream2 {
    let (section_value, section_order) = section_tokens(section);

    quote! {
        gpui_storybook::__inventory::submit! {
            ::gpui_storybook::__registry::StoryEntry {
                name: #entry_name,
                section: #section_value,
                section_order: #section_order,
                create_fn: |window, cx| {
                    ::gpui_storybook::StoryContainer::panel::<#story_type>(window, cx)
                },
                crate_name: ::std::env!("CARGO_PKG_NAME"),
                crate_dir: ::std::env!("CARGO_MANIFEST_DIR"),
                file: ::std::file!(),
                line: ::std::line!(),
            }
        }
    }
}

fn story_impl(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let args: StoryArgs = syn::parse2(args).expect("failed to parse story macro arguments");
    let input_struct: ItemStruct = syn::parse2(input).expect("story macro expects a struct");
    let struct_name = &input_struct.ident;
    let struct_name_str = struct_name.to_string();
    let registration = registration_tokens(
        quote! { #struct_name },
        &struct_name_str,
        args.section.as_ref(),
    );

    quote! {
        #input_struct

        #registration
    }
}

fn parse_component_story_args(input: &DeriveInput) -> syn::Result<ComponentStoryArgs> {
    let mut args = ComponentStoryArgs::default();

    for attr in &input.attrs {
        if !attr.path().is_ident("storybook") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("title") {
                let title: Expr = meta.value()?.parse()?;
                if args.title.replace(title).is_some() {
                    return Err(duplicate_attr_error(&meta, "title"));
                }
                return Ok(());
            }

            if meta.path.is_ident("description") {
                let description: Expr = meta.value()?.parse()?;
                if args.description.replace(description).is_some() {
                    return Err(duplicate_attr_error(&meta, "description"));
                }
                return Ok(());
            }

            if meta.path.is_ident("section") {
                let expr: Expr = meta.value()?.parse()?;
                let section = parse_section_expr(expr)?;
                if args.section.replace(section).is_some() {
                    return Err(duplicate_attr_error(&meta, "section"));
                }
                return Ok(());
            }

            if meta.path.is_ident("example") {
                let expr: Expr = meta.value()?.parse()?;
                if args.example.replace(expr).is_some() {
                    return Err(duplicate_attr_error(&meta, "example"));
                }
                return Ok(());
            }

            Err(meta.error(
                "unsupported #[storybook(...)] argument; expected `title`, `description`, `section`, or `example`",
            ))
        })?;
    }

    Ok(args)
}

fn default_component_title(struct_name: &str) -> String {
    struct_name.trim_end_matches("Story").to_title_case()
}

fn component_story_impl(input: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = syn::parse2(input).expect("ComponentStory derive expects a type");

    if !matches!(input.data, Data::Struct(_)) {
        return syn::Error::new_spanned(
            input.ident,
            "ComponentStory can only be derived for structs",
        )
        .to_compile_error();
    }

    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            input.generics,
            "ComponentStory does not support generic structs yet",
        )
        .to_compile_error();
    }

    let args = match parse_component_story_args(&input) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };

    let struct_name = &input.ident;
    let struct_name_str = struct_name.to_string();
    let default_title = default_component_title(&struct_name_str);
    let title = args
        .title
        .unwrap_or_else(|| syn::parse_quote!(#default_title));
    let description = args.description.unwrap_or_else(|| syn::parse_quote!(""));
    let example = args.example.unwrap_or_else(|| {
        syn::parse_quote! {
            <#struct_name as ::std::default::Default>::default()
        }
    });
    let wrapper_ident = format_ident!("__{}ComponentStoryView", struct_name);
    let registration = registration_tokens(
        quote! { #wrapper_ident },
        &struct_name_str,
        args.section.as_ref(),
    );

    quote! {
        struct #wrapper_ident {
            focus_handle: ::gpui::FocusHandle,
        }

        impl #wrapper_ident {
            fn view(_window: &mut ::gpui::Window, cx: &mut ::gpui::App) -> ::gpui::Entity<Self> {
                ::gpui::AppContext::new(cx, |cx| Self {
                    focus_handle: cx.focus_handle(),
                })
            }
        }

        impl ::gpui::Focusable for #wrapper_ident {
            fn focus_handle(&self, _cx: &::gpui::App) -> ::gpui::FocusHandle {
                self.focus_handle.clone()
            }
        }

        impl ::gpui::Render for #wrapper_ident {
            fn render(
                &mut self,
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::Context<Self>,
            ) -> impl ::gpui::IntoElement {
                let _ = &self.focus_handle;
                let _ = window;
                let _ = cx;
                #example
            }
        }

        impl ::gpui_storybook::Story for #wrapper_ident {
            fn klass() -> &'static str {
                #struct_name_str
            }

            fn title() -> ::std::string::String {
                (#title).into()
            }

            fn description() -> ::std::string::String {
                (#description).into()
            }

            fn new_view(
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::App,
            ) -> ::gpui::Entity<impl ::gpui::Render + ::gpui::Focusable> {
                Self::view(window, cx)
            }
        }

        #registration
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

/// Derive macro that registers a component by generating an internal story wrapper.
///
/// The component stays component-focused. The macro creates the `Story`, `Render`, and
/// `Focusable` wrapper that storybook needs.
///
/// By default the wrapper renders `<Self as Default>::default()`. Use `example = ...`
/// when the component needs a custom constructor or builder configuration. `title` and
/// `description` accept expressions that evaluate into `String`, not only string literals.
///
/// ```ignore
/// #[derive(gpui_storybook::ComponentStory, gpui::IntoElement)]
/// #[storybook(
///     title = "Button",
///     section = StorySection::Components,
///     example = ButtonChip::example(),
/// )]
/// pub struct ButtonChip;
/// ```
#[proc_macro_derive(ComponentStory, attributes(storybook))]
pub fn component_story(input: TokenStream) -> TokenStream {
    component_story_impl(input.into()).into()
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
    fn component_story_derive_generates_wrapper_story_and_registry_entry() {
        let input = quote! {
            #[storybook(section = "Components")]
            pub struct ButtonChip;
        };

        let expanded = component_story_impl(input);
        assert_snapshot!(
            "component_story_derive_generates_wrapper_story_and_registry_entry",
            snapshot_tokens(expanded)
        );
    }

    #[test]
    fn component_story_derive_with_metadata_generates_wrapper_story_and_registry_entry() {
        let input = quote! {
            #[storybook(
                title = "Button",
                description = "Interactive buttons",
                section = crate::StorySection::Components,
                example = ButtonChip::example(),
            )]
            pub struct ButtonChip;
        };

        let expanded = component_story_impl(input);
        assert_snapshot!(
            "component_story_derive_with_metadata_generates_wrapper_story_and_registry_entry",
            snapshot_tokens(expanded)
        );
    }

    #[test]
    fn component_story_derive_with_string_expressions_generates_wrapper_story_and_registry_entry() {
        let input = quote! {
            #[storybook(
                title = ::std::string::String::from("Button"),
                description = ["Interactive", " buttons"].concat(),
                section = crate::StorySection::Components,
                example = ButtonChip::example(),
            )]
            pub struct ButtonChip;
        };

        let expanded = component_story_impl(input);
        assert_snapshot!(
            "component_story_derive_with_string_expressions_generates_wrapper_story_and_registry_entry",
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
