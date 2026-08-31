use super::*;

pub(super) fn story_init_impl(_args: TokenStream2, input: TokenStream2) -> TokenStream2 {
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
