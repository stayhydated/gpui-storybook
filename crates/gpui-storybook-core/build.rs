use std::{env, path::PathBuf};

const COMPONENT_ICONS_DIR_ENV: &str = "DEP_GPUI_COMPONENT_DEFAULT_ICONS_ICONS_DIR";
const WEB_FONT_FILES: [&str; 3] = [
    "NotoSansSC-Regular-subset.ttf",
    "NotoEmoji-Regular.ttf",
    "JetBrainsMono-Regular.ttf",
];

fn main() {
    es_fluent_build::track_i18n_assets();
    export_gpui_component_web_fonts();
}

fn export_gpui_component_web_fonts() {
    let icons_dir = PathBuf::from(
        env::var_os(COMPONENT_ICONS_DIR_ENV)
            .expect("gpui-component-assets should publish its icons directory"),
    );
    let component_root = icons_dir
        .ancestors()
        .nth(4)
        .expect("gpui-component-assets should live in the gpui-component workspace");
    let fonts_dir = component_root.join("crates/story-web/fonts");

    for font_file in WEB_FONT_FILES {
        let font_path = fonts_dir.join(font_file);
        assert!(
            font_path.is_file(),
            "expected gpui-component web font at {}",
            font_path.display()
        );
        println!("cargo:rerun-if-changed={}", font_path.display());
    }

    println!("cargo:rerun-if-env-changed={COMPONENT_ICONS_DIR_ENV}");
    println!(
        "cargo:rustc-env=GPUI_COMPONENT_STORY_FONTS_DIR={}",
        fonts_dir.display()
    );
}
