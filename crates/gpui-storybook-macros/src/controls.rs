use super::story::literal_f64;
use super::*;

fn parse_control_field_args(field: &Field) -> syn::Result<Option<ControlFieldArgs>> {
    let parsed = StorybookFieldArgs::from_attributes(&field.attrs).map_err(syn::Error::from)?;
    let Some(parsed) = parsed.control else {
        return Ok(None);
    };
    if parsed.options.as_ref().is_some_and(Vec::is_empty) {
        return Err(syn::Error::new_spanned(
            field,
            "control options cannot be empty",
        ));
    }

    Ok(Some(ControlFieldArgs {
        label: parsed.label,
        description: parsed.description,
        category: parsed.category,
        min: parsed.min.map(Into::into),
        max: parsed.max.map(Into::into),
        step: parsed.step.map(Into::into),
        options: parsed.options.unwrap_or_default(),
    }))
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

pub(super) fn generated_control_fields_for_fields(
    fields: &Fields,
) -> syn::Result<Vec<GeneratedControlField>> {
    let mut generated = Vec::new();
    let Fields::Named(fields) = fields else {
        let has_control = fields.iter().any(|field| {
            field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("storybook"))
        });
        if has_control {
            return Err(syn::Error::new_spanned(
                fields,
                "story controls require named struct fields",
            ));
        }
        return Ok(generated);
    };

    for field in &fields.named {
        let Some(args) = parse_control_field_args(field)? else {
            continue;
        };
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
                "unsupported story control type; leave the field unmarked or provide string `options` for an enum implementing Display and FromStr",
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
        let static_kind = if choice {
            quote! { ::gpui_storybook::StaticControlKind::Select }
        } else if numeric && (args.min.is_some() || args.max.is_some()) {
            quote! { ::gpui_storybook::StaticControlKind::Range }
        } else {
            match type_name.as_deref() {
                Some("bool") => quote! { ::gpui_storybook::StaticControlKind::Checkbox },
                Some(
                    "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "usize" | "f32"
                    | "f64",
                ) => quote! { ::gpui_storybook::StaticControlKind::Number },
                Some("String" | "SharedString") => {
                    quote! { ::gpui_storybook::StaticControlKind::Text }
                },
                Some("Hsla") => quote! { ::gpui_storybook::StaticControlKind::ColorPicker },
                _ => quote! { ::gpui_storybook::StaticControlKind::Custom("custom") },
            }
        };
        let static_min = args.min.as_ref().and_then(literal_f64);
        let static_max = args.max.as_ref().and_then(literal_f64);
        let static_step = args.step.as_ref().and_then(literal_f64);
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
            static_kind,
            min,
            max,
            step,
            static_min,
            static_max,
            static_step,
            choice,
        });
    }

    Ok(generated)
}

pub(super) fn generated_control_fields(
    input: &DeriveInput,
) -> syn::Result<Vec<GeneratedControlField>> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "StoryControls can only be derived for structs",
        ));
    };

    generated_control_fields_for_fields(&data.fields)
}

pub(super) fn story_controls_impl(
    type_ident: &syn::Ident,
    fields: &[GeneratedControlField],
) -> TokenStream2 {
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

pub(super) fn story_controls_derive_impl(input: TokenStream2) -> TokenStream2 {
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
