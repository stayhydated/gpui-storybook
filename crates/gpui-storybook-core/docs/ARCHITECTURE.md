# Architecture

## Purpose

`gpui-storybook-core` implements the storybook UI runtime: the gallery, dock workspace, story panels, theming, i18n wiring, and asset handling.

## Module map

- `story`: Core story runtime (containers, sections, app state, window creation, and theme handling).
- `dock_gallery`: Optional dock workspace and persisted layout support behind the `dock` feature.
- `gallery`: Sidebar + active story rendering with search and top-level group/section grouping.
- `title_bar`: Custom title bar with app menu and appearance controls.
- `app_menus`: Menu construction for theme and language switching.
- `actions`: Action types used by menus and UI.
- `language` and `locale`: Language trait and locale manager wiring.
- `i18n`: Embedded es-fluent manager and locale switching.
- `assets`: Asset source that merges local assets with gpui-component icons.
- `storybook_window_ui`: Hooks for adding custom app-menu and title-bar items.
- `window_view`: Marker traits for views embedded in standard or dock windows.
- `registry`: Inventory entry types shared with the facade and macros crates.

## Startup flow

1. `story::init` initializes i18n, gpui-component, AppState, theme handling, menus, and key bindings.
1. `create_new_window` and `create_new_window_with_ui` open the storybook window and build a `StoryRoot` containing the title bar and the provided view (typically `Gallery`).
1. The window is activated and configured with title, bounds, and platform-specific settings.

## Story rendering

- `Story` defines how a story constructs its view and metadata.
- `StoryContainer` wraps story views into a `Panel` implementation, stores metadata (title, description, top-level group, section), and forwards active/visible state.
- `StorySection` is a helper element for grouping story content in the UI.
- `Gallery` owns the list of story containers, groups them by top-level crate group when present (falling back to section when no group exists), and renders the active story.
- `dock_gallery` reuses the same `StoryContainer` data, exposes stories as dock panels, and uses the same top-level group plus optional section structure in its sidebar.

## Theming and persistence

- `story::themes` loads theme name + scrollbar visibility from `target/state.json` and persists those changes (font size and radius are session-only).
- `ThemeRegistry` watches the local theme directory in debug builds and applies updates.
- `title_bar::FontSizeSelector` exposes font size, radius, and scrollbar controls.
- `dock_gallery` persists dock layout state to `target/storybook-docks.json` in debug builds and `storybook-docks.json` otherwise.

## Localization

- `Language` is a bound trait for locale enums.
- `LocaleManager` implements `LocaleStore` to expose available locales and update the current locale.
- `i18n::change_locale` bridges into es-fluent and updates gpui-component locale settings.

## Assets

- `Assets` implements `gpui::AssetSource`.
- Local assets are embedded from `assets/` (themes and i18n), while `icons/` are delegated to `gpui-component` assets.
