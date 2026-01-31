# Architecture

## Purpose
`gpui-storybook-core` implements the storybook UI runtime: the gallery, story panels, theming, i18n wiring, and asset handling.

## Module map
- `story`: Core story runtime (containers, sections, app state, window creation, and theme handling).
- `gallery`: Sidebar + active story rendering with search and section grouping.
- `title_bar`: Custom title bar with app menu and appearance controls.
- `app_menus`: Menu construction for theme and language switching.
- `actions`: Action types used by menus and UI.
- `language` and `locale`: Language trait and locale manager wiring.
- `i18n`: Embedded es-fluent manager and locale switching.
- `assets`: Asset source that merges local assets with gpui-component icons.

## Startup flow
1. `story::init` initializes i18n, gpui-component, AppState, theme handling, menus, and key bindings.
2. `create_new_window` opens the storybook window and builds a `StoryRoot` containing the title bar and the provided view (typically `Gallery`).
3. The window is activated and configured with title, bounds, and platform-specific settings.

## Story rendering
- `Story` defines how a story constructs its view and metadata.
- `StoryContainer` wraps story views into a `Panel` implementation, stores metadata (title, description, section), and forwards active/visible state.
- `StorySection` is a helper element for grouping story content in the UI.
- `Gallery` owns the list of story containers, groups them by section, and renders the active story.

## Theming and persistence
- `story::themes` loads theme name + scrollbar visibility from `target/state.json` and persists those changes (font size and radius are session-only).
- `ThemeRegistry` watches the local theme directory in debug builds and applies updates.
- `title_bar::FontSizeSelector` exposes font size, radius, and scrollbar controls.

## Localization
- `Language` is a bound trait for locale enums.
- `LocaleManager` implements `LocaleStore` to expose available locales and update the current locale.
- `i18n::change_locale` bridges into es-fluent and updates gpui-component locale settings.

## Assets
- `Assets` implements `gpui::AssetSource`.
- Local assets are embedded from `assets/` (themes and i18n), while `icons/` are delegated to `gpui-component` assets.
