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
fn story_captures_rustdocs_and_static_control_metadata() {
    let input = quote! {
        /// A button story.
        ///
        /// It documents the disabled state.
        pub struct ButtonStory {
            #[storybook(control(label = "Disabled", description = "Prevents activation"))]
            disabled: bool,
            #[storybook(control(min = -1.0, max = 32.0, step = 0.5))]
            padding: f32,
            #[storybook(control(options = ["Primary", "Danger"]))]
            intent: ButtonIntent,
        }
    };

    let expanded = snapshot_tokens(story_impl(TokenStream2::new(), input));

    assert!(expanded.contains("StoryAutodoc::new"));
    assert!(expanded.contains("A button story.\\n\\nIt documents the disabled state."));
    assert!(expanded.contains("StaticControlKind::Checkbox"));
    assert!(expanded.contains("StaticControlKind::Range"));
    assert!(expanded.contains("StaticControlKind::Select"));
    assert!(expanded.contains("Some(- 1f64)"));
    assert!(expanded.contains("Some(32f64)"));
    assert!(expanded.contains("Some(0.5f64)"));
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
fn component_story_derive_with_scenarios_generates_story_scenarios_method() {
    let input = quote! {
        /// A component story with a reusable scenario.
        #[storybook(
            title = "Button",
            scenarios = ButtonChip::scenarios(),
        )]
        pub struct ButtonChip;
    };

    let expanded = component_story_impl(input);
    assert_snapshot!(
        "component_story_derive_with_scenarios_generates_story_scenarios_method",
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
fn story_controls_derive_registers_only_marked_fields() {
    let input = quote! {
        pub struct ButtonStory {
            #[storybook(control(label = "Disabled", category = "State"))]
            disabled: bool,
            #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
            padding: f32,
            #[storybook(control(options = ["Primary", "Danger"]))]
            intent: ButtonIntent,
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
            headline: gpui_kit::SharedString,
            #[storybook(control)]
            selected: bool,
            items: Vec<String>,
        }
    };

    assert_snapshot!(
        "component_story_controls_store_defaults_and_overlay_live_values",
        snapshot_tokens(component_story_impl(input))
    );
}

#[test]
fn unsupported_control_types_and_bounds_report_compile_errors() {
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
}

#[test]
fn control_metadata_reports_parser_errors() {
    assert_compile_error(
        story_controls_derive_impl(quote! {
            pub struct UnknownControlArgumentStory {
                #[storybook(control(unknown = "value"))]
                disabled: bool,
            }
        }),
        "Unknown field: `unknown`",
    );
    assert_compile_error(
        story_controls_derive_impl(quote! {
            pub struct DuplicateControlArgumentStory {
                #[storybook(control(label = "One", label = "Two"))]
                disabled: bool,
            }
        }),
        "Duplicate field `label`",
    );
    assert_compile_error(
        story_controls_derive_impl(quote! {
            pub struct EmptyControlOptionsStory {
                #[storybook(control(options = []))]
                intent: ButtonIntent,
            }
        }),
        "control options cannot be empty",
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
    assert_compile_error(expanded, "Duplicate field `title`");
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
        "Unknown field: `unknown`",
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
            &format!("Duplicate field `{name}`"),
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
            "Unknown field: `unknown`",
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
        assert_compile_error(substory_impl(input), &format!("Duplicate field `{name}`"));
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
