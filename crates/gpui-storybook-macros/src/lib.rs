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
//! attributes `title`, `description`, `section`, `example`, and `scenarios`, plus
//! field-level `#[storybook(control...)]` metadata. It generates a hidden
//! wrapper view and registers the original component type name so
//! `disable_story = ["ComponentName"]` matches the public type the user wrote.
//! Macro-generated story entries also include a stable automation key in the
//! form `{crate-package-name}-{registered-story-name}` and an exported marker
//! that makes duplicate generated keys in the same package fail to build. Rustdoc
//! comments and static control shape metadata are captured for the static
//! catalog; localized titles and runtime defaults remain live-story values.
//!
//! `#[derive(StoryControls)]` generates typed metadata, reads, and setters for
//! explicitly marked fields. It infers `bool`, `i8` through `i64`, `isize`,
//! `u8` through `u32`, `usize`, `f32`, `f64`, `String`, `SharedString`, and
//! `Hsla`; enum-like fields provide string `options`.
//!
//! `#[derive(Substory)]` supports fieldless enums used with
//! `gpui_storybook::section(...)` or `gpui_storybook::StorySectionBase::new(...)`.
//! It generates stable capture keys from enum variant names while keeping
//! visible titles configurable with
//! `#[substory(title = "...")]`.
//!
//! `#[story_init]` registers a one-time setup function that the facade executes
//! during `gpui_storybook::init(...)`.

use darling::{FromAttributes as _, util::PreservedStrExpr};
use heck::{ToKebabCase as _, ToTitleCase as _};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Expr, ExprLit, ExprPath, Field, Fields, ItemFn, ItemStruct, Lit, LitStr,
    Token, Type, parse::Parse, parse::ParseStream,
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
    scenarios: Option<Expr>,
}

#[derive(Default, darling::FromAttributes)]
#[darling(attributes(storybook))]
struct ParsedComponentStoryArgs {
    title: Option<PreservedStrExpr>,
    description: Option<PreservedStrExpr>,
    section: Option<SectionArg>,
    example: Option<PreservedStrExpr>,
    scenarios: Option<PreservedStrExpr>,
}

#[derive(Default, darling::FromAttributes)]
#[darling(attributes(substory))]
struct SubstoryVariantArgs {
    title: Option<LitStr>,
    key: Option<LitStr>,
}

#[derive(Default)]
struct ControlFieldArgs {
    label: Option<LitStr>,
    description: Option<LitStr>,
    category: Option<LitStr>,
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
    options: Vec<LitStr>,
}

#[derive(Default, darling::FromMeta)]
#[darling(default, from_word = || Ok(Self::default()))]
struct ParsedControlFieldArgs {
    label: Option<LitStr>,
    description: Option<LitStr>,
    category: Option<LitStr>,
    min: Option<PreservedStrExpr>,
    max: Option<PreservedStrExpr>,
    step: Option<PreservedStrExpr>,
    options: Option<Vec<LitStr>>,
}

#[derive(Default, darling::FromAttributes)]
#[darling(attributes(storybook))]
struct StorybookFieldArgs {
    control: Option<ParsedControlFieldArgs>,
}

struct GeneratedControlField {
    ident: syn::Ident,
    ty: Type,
    key: String,
    label: String,
    description: String,
    category: String,
    kind: TokenStream2,
    static_kind: TokenStream2,
    min: TokenStream2,
    max: TokenStream2,
    step: TokenStream2,
    static_min: Option<f64>,
    static_max: Option<f64>,
    static_step: Option<f64>,
    options: Vec<String>,
    choice: bool,
}

mod component;
mod controls;
mod init;
mod story;
mod substory;

use component::component_story_impl;
use controls::story_controls_derive_impl;
use init::story_init_impl;
use story::story_impl;
use substory::substory_impl;

/// Attribute macro to register a story struct
///
/// Optionally accepts a section name as a string literal or enum variant:
///
/// Rustdoc comments and marked control fields are captured in the static
/// registration catalog. The stable key and registered name remain separate
/// from any localized runtime title.
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
/// Supported fields are `bool`, `i8` through `i64`, `isize`, `u8` through
/// `u32`, `usize`, `f32`, `f64`, `String`, `SharedString`, and `Hsla`.
/// Enum-like fields can provide `options = ["..."]` when they implement
/// `Display` and `FromStr`.
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

/// Derive macro that registers a component by generating an internal story wrapper.
///
/// The component stays component-focused. The macro creates the `Story`, `Render`, and
/// `Focusable` wrapper that storybook needs.
///
/// By default the wrapper renders `<Self as Default>::default()`. Use `example = ...`
/// when the component needs a custom constructor or builder configuration. `title` and
/// `description` accept expressions that evaluate into `String`, not only string literals.
/// Those expressions are emitted inside methods with `cx: &gpui_kit::App` in scope. An optional
/// `scenarios = ...` expression evaluates to `Vec<gpui_storybook::StoryScenario>` and is
/// copied into the runtime story container for automation.
///
/// ```ignore
/// #[derive(gpui_storybook::ComponentStory, gpui_kit::IntoElement)]
/// #[storybook(
///     title = "Button",
///     section = StorySection::Components,
///     example = ButtonChip::example(),
///     scenarios = ButtonChip::scenarios(),
/// )]
/// pub struct ButtonChip {
///     #[storybook(control(category = "Content"))]
///     label: gpui_kit::SharedString,
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
mod tests;
