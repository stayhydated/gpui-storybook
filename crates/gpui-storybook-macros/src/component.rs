use super::*;
use super::{
    controls::{generated_control_fields, story_controls_impl},
    story::{autodoc_tokens, registration_tokens, rustdoc_from_attrs},
};

fn parse_component_story_args(input: &DeriveInput) -> syn::Result<ComponentStoryArgs> {
    let parsed =
        ParsedComponentStoryArgs::from_attributes(&input.attrs).map_err(syn::Error::from)?;
    Ok(ComponentStoryArgs {
        title: parsed.title.map(Into::into),
        description: parsed.description.map(Into::into),
        section: parsed.section,
        example: parsed.example.map(Into::into),
        scenarios: parsed.scenarios.map(Into::into),
    })
}

fn default_component_title(struct_name: &str) -> String {
    struct_name.trim_end_matches("Story").to_title_case()
}

pub(super) fn component_story_impl(input: TokenStream2) -> TokenStream2 {
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
    let autodoc = autodoc_tokens(&rustdoc_from_attrs(&input.attrs), &control_fields);
    let scenarios = args.scenarios.map(|expression| {
        quote! {
            fn scenarios() -> ::std::vec::Vec<::gpui_storybook::StoryScenario> {
                (#expression)
            }
        }
    });
    let registration = registration_tokens(
        quote! { #wrapper_ident },
        &struct_name_str,
        args.section.as_ref(),
        autodoc,
    );

    quote! {
        struct #wrapper_ident {
            focus_handle: ::gpui_kit::FocusHandle,
            #(#wrapper_fields)*
        }

        impl #wrapper_ident {
            fn view(_window: &mut ::gpui_kit::Window, cx: &mut ::gpui_kit::App) -> ::gpui_kit::Entity<Self> {
                #view_example
                ::gpui_kit::AppContext::new(cx, |cx| Self {
                    focus_handle: cx.focus_handle(),
                    #(#wrapper_initializers)*
                })
            }
        }

        impl ::gpui_kit::Focusable for #wrapper_ident {
            fn focus_handle(&self, _cx: &::gpui_kit::App) -> ::gpui_kit::FocusHandle {
                self.focus_handle.clone()
            }
        }

        impl ::gpui_kit::Render for #wrapper_ident {
            fn render(
                &mut self,
                window: &mut ::gpui_kit::Window,
                cx: &mut ::gpui_kit::Context<Self>,
            ) -> impl ::gpui_kit::IntoElement {
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

            fn title(cx: &::gpui_kit::App) -> ::std::string::String {
                let _ = cx;
                (#title).into()
            }

            fn description(cx: &::gpui_kit::App) -> ::std::string::String {
                let _ = cx;
                (#description).into()
            }

            fn new_view(
                window: &mut ::gpui_kit::Window,
                cx: &mut ::gpui_kit::App,
            ) -> ::gpui_kit::Entity<Self> {
                Self::view(window, cx)
            }

            #scenarios
        }

        #registration
    }
}
