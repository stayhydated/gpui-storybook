mod stories;

use es_fluent::{EsFluent, ToFluentString as _};
use es_fluent_lang::{LanguageIdentifier, es_fluent_language};
use gpui::Application;
use gpui_storybook::{Assets, Gallery};
use strum::{EnumIter, IntoEnumIterator as _};

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

fn main() {
    let app = Application::new().with_assets(Assets);
    let name_arg = std::env::args().nth(1);

    app.run(move |app_cx| {
        gpui_component::init(app_cx);
        gpui_storybook::init_with_language(Languages::default(), app_cx);
        gpui_storybook::change_locale(Languages::default());
        app_cx.activate(true);

        gpui_storybook::create_new_window::<Languages, _, _>(
            &format!("{} - Stories", env!("CARGO_PKG_NAME")),
            move |window, cx| {
                let all_stories = gpui_storybook::generate_stories(window, cx);

                Gallery::view(all_stories, name_arg.as_deref(), window, cx)
            },
            app_cx,
        );
    });
}
