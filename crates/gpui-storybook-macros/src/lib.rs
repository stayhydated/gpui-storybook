//! Proc macros for GPUI Storybook registration.
//!
//! The macros emit `inventory` submissions that target the public facade crate
//! under the `gpui_storybook` crate name. Direct users of this crate still need
//! the facade crate available at that path because generated code references
//! `gpui_storybook::__inventory`, `gpui_storybook::__registry`,
//! `gpui_storybook::StoryContainer`, and `gpui_storybook::Story`.
//!
//! `#[story]` registers an explicit story struct, preserving the input item and
//! appending a `StoryEntry` submission. The optional section can be a string
//! literal or enum variant; enum variants provide both a section label and a
//! `usize` ordering key.
//!
//! `#[derive(ComponentStory)]` supports non-generic structs and helper
//! attributes `title`, `description`, `section`, and `example`, plus
//! field-level `#[storybook(control...)]` metadata. It generates a hidden
//! wrapper view and registers the original component type name so
//! `disable_story = ["ComponentName"]` matches the public type the user wrote.
//! Macro-generated story entries also include a stable automation key in the
//! form `{crate-package-name}-{registered-story-name}` and an exported marker
//! that makes duplicate generated keys in the same package fail to build.
//!
//! `#[derive(StoryControls)]` generates typed metadata, reads, and setters for
//! explicitly marked fields. Boolean, numeric, text, `SharedString`, and
//! `Hsla` fields are inferred; enum-like fields provide string `options`.
//!
//! `#[derive(Substory)]` supports fieldless enums used with
//! `gpui_storybook::section(...)` or `gpui_storybook::StorySectionBase::new(...)`.
//! It generates stable capture keys from enum variant names while keeping
//! visible titles configurable with
//! `#[substory(title = "...")]`.
//!
//! `#[story_init]` registers a one-time setup function that the facade executes
//! during `gpui_storybook::init(...)`.

use heck::{ToKebabCase as _, ToTitleCase as _};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Expr, ExprArray, ExprLit, ExprPath, Field, Fields, ItemFn, ItemStruct, Lit,
    LitStr, Token, Type, meta::ParseNestedMeta, parse::Parse, parse::ParseStream,
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

#[derive(Default)]
struct SubstoryVariantArgs {
    title: Option<LitStr>,
    key: Option<LitStr>,
}

#[derive(Default)]
struct ControlFieldArgs {
    skip: bool,
    label: Option<LitStr>,
    description: Option<LitStr>,
    category: Option<LitStr>,
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
    options: Vec<LitStr>,
}

struct GeneratedControlField {
    ident: syn::Ident,
    ty: Type,
    key: String,
    label: String,
    description: String,
    category: String,
    kind: TokenStream2,
    min: TokenStream2,
    max: TokenStream2,
    step: TokenStream2,
    options: Vec<String>,
    choice: bool,
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
        }
    }
}

fn story_impl(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
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

fn validate_substory_key(key: &LitStr) -> syn::Result<()> {
    let value = key.value();
    let is_valid = !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-');

    if is_valid {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            key,
            "substory key must use lowercase ASCII letters, numbers, or `-`",
        ))
    }
}

fn parse_substory_variant_args(attrs: &[syn::Attribute]) -> syn::Result<SubstoryVariantArgs> {
    let mut args = SubstoryVariantArgs::default();

    for attr in attrs {
        if !attr.path().is_ident("substory") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("title") {
                let title: LitStr = meta.value()?.parse()?;
                if args.title.replace(title).is_some() {
                    return Err(duplicate_attr_error(&meta, "title"));
                }
                return Ok(());
            }

            if meta.path.is_ident("key") {
                let key: LitStr = meta.value()?.parse()?;
                validate_substory_key(&key)?;
                if args.key.replace(key).is_some() {
                    return Err(duplicate_attr_error(&meta, "key"));
                }
                return Ok(());
            }

            Err(meta.error("unsupported #[substory(...)] argument; expected `title` or `key`"))
        })?;
    }

    Ok(args)
}

fn substory_impl(input: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = match syn::parse2(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };

    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return syn::Error::new_spanned(input.ident, "Substory can only be derived for enums")
                .to_compile_error();
        },
    };

    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            input.generics,
            "Substory does not support generic enums yet",
        )
        .to_compile_error();
    }

    let enum_name = &input.ident;
    let mut key_arms = Vec::new();
    let mut title_arms = Vec::new();

    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(&variant.fields, "Substory variants must be fieldless")
                .to_compile_error();
        }

        let args = match parse_substory_variant_args(&variant.attrs) {
            Ok(args) => args,
            Err(err) => return err.to_compile_error(),
        };

        let variant_ident = &variant.ident;
        let default_key = variant_ident.to_string().to_kebab_case();
        let key = args.key.map_or(default_key, |key| key.value());
        let default_title = variant_ident.to_string().to_title_case();
        let title = args.title.map_or(default_title, |title| title.value());

        key_arms.push(quote! {
            Self::#variant_ident => #key,
        });
        title_arms.push(quote! {
            Self::#variant_ident => #title.into(),
        });
    }

    quote! {
        impl ::gpui_storybook::Substory for #enum_name {
            fn capture_key(&self) -> &'static str {
                match self {
                    #(#key_arms)*
                }
            }

            fn title(&self) -> ::gpui::SharedString {
                match self {
                    #(#title_arms)*
                }
            }
        }
    }
}

fn parse_control_field_args(field: &Field) -> syn::Result<Option<ControlFieldArgs>> {
    let mut control = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("storybook") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("control") {
                return Err(meta.error(
                    "unsupported field #[storybook(...)] argument; expected `control`",
                ));
            }
            if control.is_some() {
                return Err(duplicate_attr_error(&meta, "control"));
            }

            let mut args = ControlFieldArgs::default();
            if !meta.input.is_empty() {
                meta.parse_nested_meta(|nested| {
                    if nested.path.is_ident("skip") {
                        args.skip = true;
                        return Ok(());
                    }
                    if nested.path.is_ident("label") {
                        let value: LitStr = nested.value()?.parse()?;
                        if args.label.replace(value).is_some() {
                            return Err(duplicate_attr_error(&nested, "label"));
                        }
                        return Ok(());
                    }
                    if nested.path.is_ident("description") {
                        let value: LitStr = nested.value()?.parse()?;
                        if args.description.replace(value).is_some() {
                            return Err(duplicate_attr_error(&nested, "description"));
                        }
                        return Ok(());
                    }
                    if nested.path.is_ident("category") {
                        let value: LitStr = nested.value()?.parse()?;
                        if args.category.replace(value).is_some() {
                            return Err(duplicate_attr_error(&nested, "category"));
                        }
                        return Ok(());
                    }
                    if nested.path.is_ident("min") {
                        let value: Expr = nested.value()?.parse()?;
                        if args.min.replace(value).is_some() {
                            return Err(duplicate_attr_error(&nested, "min"));
                        }
                        return Ok(());
                    }
                    if nested.path.is_ident("max") {
                        let value: Expr = nested.value()?.parse()?;
                        if args.max.replace(value).is_some() {
                            return Err(duplicate_attr_error(&nested, "max"));
                        }
                        return Ok(());
                    }
                    if nested.path.is_ident("step") {
                        let value: Expr = nested.value()?.parse()?;
                        if args.step.replace(value).is_some() {
                            return Err(duplicate_attr_error(&nested, "step"));
                        }
                        return Ok(());
                    }
                    if nested.path.is_ident("options") {
                        if !args.options.is_empty() {
                            return Err(duplicate_attr_error(&nested, "options"));
                        }
                        let values: ExprArray = nested.value()?.parse()?;
                        for value in values.elems {
                            let Expr::Lit(ExprLit {
                                lit: Lit::Str(value),
                                ..
                            }) = value
                            else {
                                return Err(syn::Error::new_spanned(
                                    value,
                                    "control options must be string literals",
                                ));
                            };
                            args.options.push(value);
                        }
                        if args.options.is_empty() {
                            return Err(nested.error("control options cannot be empty"));
                        }
                        return Ok(());
                    }

                    Err(nested.error(
                        "unsupported control argument; expected `skip`, `label`, `description`, `category`, `min`, `max`, `step`, or `options`",
                    ))
                })?;
            }

            control = Some(args);
            Ok(())
        })?;
    }

    Ok(control)
}

fn control_type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn generated_control_fields(input: &DeriveInput) -> syn::Result<Vec<GeneratedControlField>> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "StoryControls can only be derived for structs",
        ));
    };

    let mut generated = Vec::new();
    let Fields::Named(fields) = &data.fields else {
        let has_control = data.fields.iter().any(|field| {
            field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("storybook"))
        });
        if has_control {
            return Err(syn::Error::new_spanned(
                &data.fields,
                "story controls require named struct fields",
            ));
        }
        return Ok(generated);
    };

    for field in &fields.named {
        let Some(args) = parse_control_field_args(field)? else {
            continue;
        };
        if args.skip {
            if args.label.is_some()
                || args.description.is_some()
                || args.category.is_some()
                || args.min.is_some()
                || args.max.is_some()
                || args.step.is_some()
                || !args.options.is_empty()
            {
                return Err(syn::Error::new_spanned(
                    field,
                    "`skip` cannot be combined with other control arguments",
                ));
            }
            continue;
        }

        let ident = field
            .ident
            .clone()
            .expect("named fields always have identifiers");
        let key = ident.to_string();
        let type_name = control_type_name(&field.ty);
        let numeric = matches!(
            type_name.as_deref(),
            Some(
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "usize"
                    | "f32"
                    | "f64"
            )
        );
        let supported = matches!(
            type_name.as_deref(),
            Some(
                "bool"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "String"
                    | "SharedString"
                    | "Hsla"
            )
        );
        let choice = !args.options.is_empty();
        if !supported && !choice {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "unsupported story control type; use `control(skip)` or provide string `options` for an enum implementing Display and FromStr",
            ));
        }
        if !numeric && (args.min.is_some() || args.max.is_some() || args.step.is_some()) {
            return Err(syn::Error::new_spanned(
                field,
                "`min`, `max`, and `step` are only supported by numeric controls",
            ));
        }
        if choice && (args.min.is_some() || args.max.is_some() || args.step.is_some()) {
            return Err(syn::Error::new_spanned(
                field,
                "select controls cannot also define numeric bounds",
            ));
        }

        let kind = if choice {
            quote! { ::gpui_storybook::ControlKind::Select }
        } else if numeric && (args.min.is_some() || args.max.is_some()) {
            quote! { ::gpui_storybook::ControlKind::Range }
        } else {
            let ty = &field.ty;
            quote! { <#ty as ::gpui_storybook::ControlValueField>::control_kind() }
        };
        let min = args
            .min
            .map_or_else(|| quote! { None }, |value| quote! { Some((#value) as f64) });
        let max = args
            .max
            .map_or_else(|| quote! { None }, |value| quote! { Some((#value) as f64) });
        let step = args
            .step
            .map_or_else(|| quote! { None }, |value| quote! { Some((#value) as f64) });

        generated.push(GeneratedControlField {
            ident,
            ty: field.ty.clone(),
            label: args
                .label
                .map_or_else(|| key.to_title_case(), |label| label.value()),
            description: args
                .description
                .map_or_else(String::new, |value| value.value()),
            category: args
                .category
                .map_or_else(|| "Properties".to_owned(), |value| value.value()),
            options: args
                .options
                .into_iter()
                .map(|value| value.value())
                .collect(),
            key,
            kind,
            min,
            max,
            step,
            choice,
        });
    }

    Ok(generated)
}

fn story_controls_impl(type_ident: &syn::Ident, fields: &[GeneratedControlField]) -> TokenStream2 {
    let specs = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let key = &field.key;
        let label = &field.label;
        let description = &field.description;
        let category = &field.category;
        let kind = &field.kind;
        let min = &field.min;
        let max = &field.max;
        let step = &field.step;
        let options = &field.options;
        let default = if field.choice {
            quote! { ::gpui_storybook::choice_control_value(&self.#ident) }
        } else {
            quote! {
                <#ty as ::gpui_storybook::ControlValueField>::to_control_value(&self.#ident)
            }
        };

        quote! {
            ::gpui_storybook::ControlSpec {
                key: #key.to_owned(),
                label: #label.to_owned(),
                description: #description.to_owned(),
                category: #category.to_owned(),
                kind: #kind,
                default: #default,
                bounds: ::gpui_storybook::ControlBounds {
                    min: #min,
                    max: #max,
                    step: #step,
                },
                options: vec![#(#options.to_owned()),*],
            }
        }
    });
    let value_arms = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let key = &field.key;
        let value = if field.choice {
            quote! { ::gpui_storybook::choice_control_value(&self.#ident) }
        } else {
            quote! {
                <#ty as ::gpui_storybook::ControlValueField>::to_control_value(&self.#ident)
            }
        };
        quote! { #key => Ok(#value), }
    });
    let setter_arms = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        let key = &field.key;
        if field.choice {
            let options = &field.options;
            quote! {
                #key => {
                    let options = vec![#(#options.to_owned()),*];
                    self.#ident = ::gpui_storybook::parse_choice_control_value::<#ty>(
                        #key,
                        value,
                        &options,
                    )?;
                    Ok(())
                },
            }
        } else {
            quote! {
                #key => {
                    self.#ident = <#ty as ::gpui_storybook::ControlValueField>::from_control_value(
                        #key,
                        value,
                    )?;
                    Ok(())
                },
            }
        }
    });

    quote! {
        impl ::gpui_storybook::StoryControls for #type_ident {
            fn control_specs(&self) -> ::std::vec::Vec<::gpui_storybook::ControlSpec> {
                vec![#(#specs),*]
            }

            fn control_value(
                &self,
                key: &str,
            ) -> ::std::result::Result<
                ::gpui_storybook::ControlValue,
                ::gpui_storybook::ControlError,
            > {
                match key {
                    #(#value_arms)*
                    _ => Err(::gpui_storybook::ControlError::UnknownControl {
                        key: key.to_owned(),
                    }),
                }
            }

            fn set_control_value(
                &mut self,
                key: &str,
                value: ::gpui_storybook::ControlValue,
            ) -> ::std::result::Result<(), ::gpui_storybook::ControlError> {
                match key {
                    #(#setter_arms)*
                    _ => Err(::gpui_storybook::ControlError::UnknownControl {
                        key: key.to_owned(),
                    }),
                }
            }
        }
    }
}

fn story_controls_derive_impl(input: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = match syn::parse2(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };
    if !input.generics.params.is_empty() {
        return syn::Error::new_spanned(
            input.generics,
            "StoryControls does not support generic structs",
        )
        .to_compile_error();
    }
    let fields = match generated_control_fields(&input) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error(),
    };

    story_controls_impl(&input.ident, &fields)
}

fn component_story_impl(input: TokenStream2) -> TokenStream2 {
    let input: DeriveInput = match syn::parse2(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };

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
    let control_fields = match generated_control_fields(&input) {
        Ok(fields) => fields,
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
    let wrapper_fields = control_fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! { #ident: #ty, }
    });
    let wrapper_initializers = control_fields.iter().map(|field| {
        let ident = &field.ident;
        quote! { #ident: example.#ident.clone(), }
    });
    let example_overlays = control_fields.iter().map(|field| {
        let ident = &field.ident;
        quote! { example.#ident = self.#ident.clone(); }
    });
    let view_example = (!control_fields.is_empty()).then(|| quote! { let example = #example; });
    let render_example = if control_fields.is_empty() {
        quote! { #example }
    } else {
        quote! {
            let mut example = #example;
            #(#example_overlays)*
            example
        }
    };
    let controls_impl = if control_fields.is_empty() {
        quote! { impl ::gpui_storybook::StoryControls for #wrapper_ident {} }
    } else {
        story_controls_impl(&wrapper_ident, &control_fields)
    };
    let registration = registration_tokens(
        quote! { #wrapper_ident },
        &struct_name_str,
        args.section.as_ref(),
    );

    quote! {
        struct #wrapper_ident {
            focus_handle: ::gpui::FocusHandle,
            #(#wrapper_fields)*
        }

        impl #wrapper_ident {
            fn view(_window: &mut ::gpui::Window, cx: &mut ::gpui::App) -> ::gpui::Entity<Self> {
                #view_example
                ::gpui::AppContext::new(cx, |cx| Self {
                    focus_handle: cx.focus_handle(),
                    #(#wrapper_initializers)*
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
                #render_example
            }
        }

        #controls_impl

        impl ::gpui_storybook::Story for #wrapper_ident {
            fn klass() -> &'static str {
                #struct_name_str
            }

            fn title(cx: &::gpui::App) -> ::std::string::String {
                let _ = cx;
                (#title).into()
            }

            fn description(cx: &::gpui::App) -> ::std::string::String {
                let _ = cx;
                (#description).into()
            }

            fn new_view(
                window: &mut ::gpui::Window,
                cx: &mut ::gpui::App,
            ) -> ::gpui::Entity<Self> {
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
/// #[derive(gpui_storybook::StoryControls)]
/// #[story("Components")]
/// pub struct ButtonStory;
///
/// // Enum variant (sorted by enum discriminant order)
/// #[derive(gpui_storybook::StoryControls)]
/// #[story(StorySection::Components)]
/// pub struct ButtonStory;
/// ```
#[proc_macro_attribute]
pub fn story(args: TokenStream, input: TokenStream) -> TokenStream {
    story_impl(args.into(), input.into()).into()
}

/// Derives typed access to fields marked with `#[storybook(control...)]`.
///
/// Supported fields are `bool`, integer and floating-point primitives,
/// `String`, `SharedString`, and `Hsla`. Enum-like fields can provide
/// `options = ["..."]` when they implement `Display` and `FromStr`.
///
/// ```ignore
/// #[derive(gpui_storybook::StoryControls)]
/// struct ButtonStory {
///     #[storybook(control)]
///     disabled: bool,
///     #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
///     padding: f32,
/// }
/// ```
#[proc_macro_derive(StoryControls, attributes(storybook))]
pub fn story_controls(input: TokenStream) -> TokenStream {
    story_controls_derive_impl(input.into()).into()
}

fn story_init_impl(_args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let input_fn: ItemFn = match syn::parse2(input) {
        Ok(input_fn) => input_fn,
        Err(err) => return err.to_compile_error(),
    };
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
/// Those expressions are emitted inside methods with `cx: &gpui::App` in scope.
///
/// ```ignore
/// #[derive(gpui_storybook::ComponentStory, gpui::IntoElement)]
/// #[storybook(
///     title = "Button",
///     section = StorySection::Components,
///     example = ButtonChip::example(),
/// )]
/// pub struct ButtonChip {
///     #[storybook(control(category = "Content"))]
///     label: gpui::SharedString,
/// }
/// ```
#[proc_macro_derive(ComponentStory, attributes(storybook))]
pub fn component_story(input: TokenStream) -> TokenStream {
    component_story_impl(input.into()).into()
}

/// Derive stable capture metadata for sub-story sections.
///
/// Variants become capture-addressable section descriptors that can be passed
/// to `gpui_storybook::section(...)` or `gpui_storybook::StorySectionBase::new(...)`.
/// By default, the route key is the variant name in kebab case and the visible
/// title is title case. Use
/// `#[substory(title = "...")]` to change the visible title without changing
/// the route key. Use `#[substory(key = "...")]` to set an explicit route key
/// independent of the variant name.
///
/// ```ignore
/// #[derive(gpui_storybook::Substory)]
/// enum ButtonSubstory {
///     NormalButton,
///     #[substory(title = "Button with Icon")]
///     ButtonWithIcon,
///     #[substory(key = "progress", title = "With Progress")]
///     WithProgress,
/// }
/// ```
#[proc_macro_derive(Substory, attributes(substory))]
pub fn substory(input: TokenStream) -> TokenStream {
    substory_impl(input.into()).into()
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

    fn assert_compile_error(tokens: TokenStream2, message: &str) {
        let tokens = tokens.to_string();
        assert!(
            tokens.contains("compile_error"),
            "expected compile error: {tokens}"
        );
        assert!(tokens.contains(message), "missing `{message}` in: {tokens}");
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
    fn story_with_enum_section_generates_ordered_registry_entry() {
        let expanded = story_impl(
            quote! { crate::StorySection::Components, },
            quote! { pub struct ButtonStory; },
        );
        let expanded = snapshot_tokens(expanded);
        let compact = expanded.split_whitespace().collect::<String>();

        assert!(expanded.contains("Some(\"Components\")"));
        assert!(compact.contains("Some(crate::StorySection::Componentsasusize)"));
    }

    #[test]
    fn malformed_story_arguments_report_compile_error() {
        assert_compile_error(
            story_impl(quote! { 42 }, quote! { pub struct ButtonStory; }),
            "expected identifier",
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
    fn story_controls_derive_generates_typed_metadata_and_setters() {
        let input = quote! {
            pub struct ButtonStory {
                #[storybook(control(label = "Disabled", category = "State"))]
                disabled: bool,
                #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
                padding: f32,
                #[storybook(control(options = ["Primary", "Danger"]))]
                intent: ButtonIntent,
                #[storybook(control(skip))]
                focus_handle: FocusHandle,
            }
        };

        assert_snapshot!(
            "story_controls_derive_generates_typed_metadata_and_setters",
            snapshot_tokens(story_controls_derive_impl(input))
        );
    }

    #[test]
    fn component_story_controls_store_defaults_and_overlay_live_values() {
        let input = quote! {
            #[storybook(example = WelcomeCard::example())]
            pub struct WelcomeCard {
                #[storybook(control(category = "Content"))]
                headline: gpui::SharedString,
                #[storybook(control)]
                selected: bool,
                #[storybook(control(skip))]
                items: Vec<String>,
            }
        };

        assert_snapshot!(
            "component_story_controls_store_defaults_and_overlay_live_values",
            snapshot_tokens(component_story_impl(input))
        );
    }

    #[test]
    fn explicitly_requested_unsupported_controls_report_compile_errors() {
        assert_compile_error(
            story_controls_derive_impl(quote! {
                pub struct UnsupportedStory {
                    #[storybook(control)]
                    items: Vec<String>,
                }
            }),
            "unsupported story control type",
        );
        assert_compile_error(
            story_controls_derive_impl(quote! {
                pub struct InvalidBoundStory {
                    #[storybook(control(min = 1.0))]
                    label: String,
                }
            }),
            "only supported by numeric controls",
        );
        assert_compile_error(
            story_controls_derive_impl(quote! {
                pub struct InvalidSkipStory {
                    #[storybook(control(skip, label = "Hidden"))]
                    label: String,
                }
            }),
            "`skip` cannot be combined",
        );
    }

    #[test]
    fn substory_derive_generates_stable_keys_and_titles() {
        let input = quote! {
            pub enum ButtonSubstory {
                NormalButton,
                #[substory(title = "Button with Icon")]
                ButtonWithIcon,
                #[substory(key = "progress", title = "With Progress")]
                WithProgress,
            }
        };

        let expanded = substory_impl(input);
        assert_snapshot!(
            "substory_derive_generates_stable_keys_and_titles",
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

    #[test]
    fn story_attribute_on_non_struct_reports_compile_error() {
        let input = quote! {
            pub fn button_story() {}
        };

        let expanded = story_impl(TokenStream2::new(), input);
        assert_compile_error(expanded, "expected `struct`");
    }

    #[test]
    fn component_story_duplicate_metadata_reports_compile_error() {
        let input = quote! {
            #[storybook(title = "Button", title = "Button Again")]
            pub struct ButtonChip;
        };

        let expanded = component_story_impl(input);
        assert_compile_error(expanded, "duplicate `title` argument");
    }

    #[test]
    fn component_story_rejects_unsupported_inputs() {
        assert_compile_error(
            component_story_impl(quote! { pub enum ButtonChip { Default } }),
            "ComponentStory can only be derived for structs",
        );
        assert_compile_error(
            component_story_impl(quote! { pub struct ButtonChip<T>(T); }),
            "ComponentStory does not support generic structs yet",
        );
        assert_compile_error(
            component_story_impl(quote! {
                #[storybook(section = 42)]
                pub struct ButtonChip;
            }),
            "`section` must be a string literal or enum variant path",
        );
        assert_compile_error(
            component_story_impl(quote! {
                #[storybook(unknown = "value")]
                pub struct ButtonChip;
            }),
            "unsupported #[storybook(...)] argument",
        );
    }

    #[test]
    fn component_story_rejects_each_duplicate_metadata_field() {
        for (input, name) in [
            (
                quote! {
                    #[storybook(description = "one", description = "two")]
                    pub struct ButtonChip;
                },
                "description",
            ),
            (
                quote! {
                    #[storybook(section = "one", section = "two")]
                    pub struct ButtonChip;
                },
                "section",
            ),
            (
                quote! {
                    #[storybook(example = one(), example = two())]
                    pub struct ButtonChip;
                },
                "example",
            ),
        ] {
            assert_compile_error(
                component_story_impl(input),
                &format!("duplicate `{name}` argument"),
            );
        }
    }

    #[test]
    fn unrelated_attributes_do_not_change_component_defaults() {
        let expanded = component_story_impl(quote! {
            #[derive(Clone)]
            pub struct MenuButtonStory;
        });
        let expanded = snapshot_tokens(expanded);

        assert!(expanded.contains("\"Menu Button\""));
        assert!(expanded.contains("<MenuButtonStory as ::std::default::Default>::default()"));
    }

    #[test]
    fn substory_rejects_unsupported_inputs_and_metadata() {
        for (input, message) in [
            (
                quote! { pub struct ButtonSubstory; },
                "Substory can only be derived for enums",
            ),
            (
                quote! { pub enum ButtonSubstory<T> { Default(T) } },
                "Substory does not support generic enums yet",
            ),
            (
                quote! { pub enum ButtonSubstory { Default(String) } },
                "Substory variants must be fieldless",
            ),
            (
                quote! {
                    pub enum ButtonSubstory {
                        #[substory(unknown = "value")]
                        Default,
                    }
                },
                "unsupported #[substory(...)] argument",
            ),
            (
                quote! {
                    pub enum ButtonSubstory {
                        #[substory(key = "Not Stable")]
                        Default,
                    }
                },
                "substory key must use lowercase ASCII letters, numbers, or `-`",
            ),
            (
                quote! {
                    pub enum ButtonSubstory {
                        #[substory(key = "")]
                        Default,
                    }
                },
                "substory key must use lowercase ASCII letters, numbers, or `-`",
            ),
        ] {
            assert_compile_error(substory_impl(input), message);
        }
    }

    #[test]
    fn substory_rejects_duplicate_metadata_fields() {
        for (input, name) in [
            (
                quote! {
                    pub enum ButtonSubstory {
                        #[substory(title = "One", title = "Two")]
                        Default,
                    }
                },
                "title",
            ),
            (
                quote! {
                    pub enum ButtonSubstory {
                        #[substory(key = "one", key = "two")]
                        Default,
                    }
                },
                "key",
            ),
        ] {
            assert_compile_error(
                substory_impl(input),
                &format!("duplicate `{name}` argument"),
            );
        }
    }

    #[test]
    fn malformed_substory_input_reports_compile_error() {
        assert_compile_error(substory_impl(quote! { impl }), "expected");
    }

    #[test]
    fn story_init_attribute_on_non_function_reports_compile_error() {
        let input = quote! {
            pub struct Setup;
        };

        let expanded = story_init_impl(TokenStream2::new(), input);
        assert_compile_error(expanded, "expected `fn`");
    }
}
