use super::controls::generated_control_fields_for_fields;
use super::*;

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

impl darling::FromMeta for SectionArg {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        parse_section_expr(expr.clone()).map_err(darling::Error::from)
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

pub(super) fn rustdoc_from_attrs(attrs: &[syn::Attribute]) -> String {
    let mut lines = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            let syn::Meta::NameValue(meta) = &attr.meta else {
                return None;
            };
            let Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            }) = &meta.value
            else {
                return None;
            };
            let value = value.value();
            Some(value.strip_prefix(' ').unwrap_or(value.as_str()).to_owned())
        })
        .collect::<Vec<_>>();

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

pub(super) fn literal_f64(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => value.base10_parse().ok(),
        Expr::Lit(ExprLit {
            lit: Lit::Float(value),
            ..
        }) => value.base10_parse().ok(),
        Expr::Unary(unary) if matches!(&unary.op, syn::UnOp::Neg(_)) => {
            literal_f64(&unary.expr).map(|value| -value)
        },
        _ => None,
    }
}

fn static_control_tokens(fields: &[GeneratedControlField]) -> TokenStream2 {
    let controls = fields.iter().map(|field| {
        let key = &field.key;
        let label = &field.label;
        let description = &field.description;
        let category = &field.category;
        let kind = &field.static_kind;
        let min = field
            .static_min
            .map_or_else(|| quote! { None }, |value| quote! { Some(#value) });
        let max = field
            .static_max
            .map_or_else(|| quote! { None }, |value| quote! { Some(#value) });
        let step = field
            .static_step
            .map_or_else(|| quote! { None }, |value| quote! { Some(#value) });
        let options = &field.options;

        quote! {
            ::gpui_storybook::StaticControlSpec::new(
                #key,
                #label,
                #description,
                #category,
                #kind,
                ::gpui_storybook::ControlBounds {
                    min: #min,
                    max: #max,
                    step: #step,
                },
                &[#(#options),*],
            )
        }
    });

    quote! { &[#(#controls),*] }
}

pub(super) fn autodoc_tokens(docs: &str, fields: &[GeneratedControlField]) -> TokenStream2 {
    let controls = static_control_tokens(fields);
    quote! {
        ::gpui_storybook::__registry::StoryAutodoc::new(#docs, #controls)
    }
}

pub(super) fn registration_tokens(
    story_type: TokenStream2,
    entry_name: &str,
    section: Option<&SectionArg>,
    autodoc: TokenStream2,
) -> TokenStream2 {
    let (section_value, section_order) = section_tokens(section);
    let marker_ident = format_ident!("__gpui_storybook_story_key_marker_{entry_name}");

    quote! {
        #[doc(hidden)]
        #[used]
        #[unsafe(export_name = concat!(
            "__gpui_storybook_story_key__",
            env!("CARGO_PKG_NAME"),
            "__",
            #entry_name,
        ))]
        static #marker_ident: u8 = 0;

        gpui_storybook::__inventory::submit! {
            ::gpui_storybook::__registry::StoryEntry::new(
                concat!(::std::env!("CARGO_PKG_NAME"), "-", #entry_name),
                #entry_name,
                #section_value,
                #section_order,
                |window, cx| {
                    ::gpui_storybook::StoryContainer::panel::<#story_type>(window, cx)
                },
                ::gpui_storybook::__registry::StoryRegistrationSource::new(
                    ::std::env!("CARGO_PKG_NAME"),
                    ::std::env!("CARGO_MANIFEST_DIR"),
                    ::std::file!(),
                    ::std::line!(),
                ),
            )
            .with_autodoc(#autodoc)
        }
    }
}

pub(super) fn story_impl(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let args: StoryArgs = match syn::parse2(args) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };
    let input_struct: ItemStruct = match syn::parse2(input) {
        Ok(input_struct) => input_struct,
        Err(err) => return err.to_compile_error(),
    };
    let struct_name = &input_struct.ident;
    let struct_name_str = struct_name.to_string();
    let control_fields =
        generated_control_fields_for_fields(&input_struct.fields).unwrap_or_default();
    let autodoc = autodoc_tokens(&rustdoc_from_attrs(&input_struct.attrs), &control_fields);
    let registration = registration_tokens(
        quote! { #struct_name },
        &struct_name_str,
        args.section.as_ref(),
        autodoc,
    );

    quote! {
        #input_struct

        #registration
    }
}
