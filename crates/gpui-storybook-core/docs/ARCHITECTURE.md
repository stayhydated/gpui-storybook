# Architecture: gpui-storybook-core

## Purpose

`gpui-storybook-core` implements the runtime shell behind `gpui-storybook`.

It owns:

- window creation and title-bar composition
- the gallery UI
- the optional dock workspace
- story container/panel behavior
- theme persistence and theme watching
- locale switching and embedded assets
- inventory entry types shared with the facade and macro crates

## Module boundaries

- `story`: runtime contract and shell bootstrapping
  - `components.rs`: `Story`, `StoryContainer`, `StoryState`, section helpers, and panel behavior
  - `window.rs`: `create_new_window` and `create_new_window_with_ui`
  - `init.rs`: core runtime startup
  - `state.rs`: global app state such as invisible panel tracking
  - `themes.rs`: theme restore/persist and debug-time watch mode
- `gallery`: searchable sidebar plus active story display
- `dock_gallery`: feature-gated dock workspace, sidebar panel, panel registry, and layout persistence
- `title_bar`: shared title-bar UI and controls
- `app_menus` and `actions`: menu/actions surface consumed by the shell
- `language`, `locale`, and `i18n`: locale abstraction and bridge into `es-fluent` plus `gpui-component`
- `assets`: embedded themes/i18n assets plus delegated icon loading
- `storybook_window_ui`: customization hooks for app-menu and title-bar content
- `window_view`: marker traits for views mounted in the standard or dock windows
- `registry`: shared `StoryEntry` and `InitEntry` inventory item definitions

## Startup flow

`gpui_storybook_core::story::init` is the runtime bootstrap sequence used by the facade:

1. initialize embedded i18n through `es_fluent_manager_embedded`
1. initialize `gpui-component`
1. install `AppState`
1. restore persisted theme state and register theme actions
1. bind common actions such as `/` for search and `cmd-q` for quit
1. install the base application menus and activate the app

The facade supplies the language-specific `LocaleStore` before calling this function.

## Story lifecycle

`Story` is the core runtime contract. `StoryContainer::panel::<S>` turns a story type into a panel entity by:

1. calling `S::new_view(window, cx)`
1. storing `S::klass()` as `story_klass`
1. capturing `S::title` and `S::description` as deferred metadata functions
1. wrapping the rendered view in panel chrome used by both gallery and dock modes

The resulting `StoryContainer` carries both runtime metadata (`group`, `section`, `story_klass`) and panel behavior (`Panel`, `PanelView`, focus handling, visibility).

## Gallery runtime

`gallery::Gallery` is the simple storybook shell:

- it owns the full story list
- it keeps search state in a `gpui_component::input::InputState`
- filtering matches story title, top-level group, and section
- the sidebar groups stories by `group`, falling back to `section` when no group exists
- the active story is rendered from the filtered list while preserving indices into the original story vector

This mode is stateless beyond the currently selected story and the search field.

## Dock runtime

`dock_gallery` reuses `StoryContainer` values but adds persistence and re-instantiation semantics.

Key internal pieces:

- `STORY_PANELS`: per-dock-area map of mounted `StoryContainer` weak refs keyed by `story_klass`
- `STORY_SEEDS`: per-dock-area registry of seed metadata used to recreate stories from layout state
- `DockLayoutStore`: JSON sanitize/load/save layer for `DockAreaState`

Important behavior:

- layout state is saved to `target/storybook-docks.json` in debug builds and `storybook-docks.json` otherwise
- saved state is sanitized before load/save so null tab-panel payloads from upstream dock state do not break reloads
- opening a story first tries `reveal_story_panel`; if the panel is not mounted, the dock adds it back from the seed registry
- the sidebar panel is itself a dock panel and uses `StorySidebarItem` plus `StoryDrag` from `gpui-storybook-components`

## Window shell composition

`create_new_window_with_ui` is the central shell constructor. It:

- opens a GPUI window with storybook defaults
- wraps the caller-provided view in `StorybookWindow<V>`
- mounts the shared title bar
- applies optional menu/title-bar builders from `StorybookWindowUi`

The dock entry point follows the same pattern but mounts `StoryWorkspace` instead of a simple view.

## Theme and locale behavior

Theme state is persisted to `target/state.json` through `story::themes`.

Internal rules:

- the selected theme and scrollbar visibility are restored on startup
- debug builds watch `assets/themes` and re-apply the selected theme on file changes
- font size and radius controls live in runtime state but only theme name and scrollbar visibility are persisted

Locale behavior is split across modules:

- `language.rs` defines the trait bound expected from app language enums
- `locale.rs` adapts that enum into a `LocaleStore`
- `i18n.rs` delegates locale changes into `es_fluent_manager_embedded`
- `LocaleManager::set_current_locale` also updates `gpui_component::set_locale`

## Dependency edges

- `gpui-component` supplies the bulk of shell UI, sidebar, dock, theming, and controls
- `es-fluent` and `es-fluent-manager-embedded` supply localization lookup and embedded locale data
- `gpui-storybook-components` supplies dock-sidebar-specific primitives
- `inventory` is only used for shared registry types; discovery itself happens in the facade crate
