use super::*;

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
    let args = SubstoryVariantArgs::from_attributes(attrs).map_err(syn::Error::from)?;
    if let Some(key) = &args.key {
        validate_substory_key(key)?;
    }
    Ok(args)
}

pub(super) fn substory_impl(input: TokenStream2) -> TokenStream2 {
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

            fn title(&self) -> ::gpui_kit::SharedString {
                match self {
                    #(#title_arms)*
                }
            }
        }
    }
}
