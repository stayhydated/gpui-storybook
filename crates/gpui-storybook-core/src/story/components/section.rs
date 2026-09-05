use super::*;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ShowPanelInfo;

/// Stable descriptor for a capture-addressable section inside a story.
///
/// Derive this with `#[derive(gpui_storybook::Substory)]` on a fieldless enum,
/// then pass variants to [`section`] or [`StorySectionBase::new`] so capture
/// routes use stable enum-derived keys instead of display-title slugs.
pub trait Substory: 'static {
    /// Stable route segment used in `story-key/substory-key` capture routes.
    fn capture_key(&self) -> &'static str;

    /// Visible section title shown in the story UI.
    fn title(&self) -> SharedString;
}

/// Input accepted by [`section`] and [`StorySectionBase::new`] for visible
/// titles and stable capture keys.
#[derive(Clone, Debug)]
pub struct StorySectionTitle {
    title: SharedString,
    pub(super) capture_key: Option<SharedString>,
}

impl StorySectionTitle {
    /// Create a section whose capture key is derived from the visible title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            capture_key: None,
        }
    }

    /// Create a section with an explicit stable capture key.
    pub fn with_capture_key(
        capture_key: impl Into<SharedString>,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            capture_key: Some(capture_key.into()),
        }
    }

    /// Split the descriptor into its visible title and optional capture key.
    pub fn into_parts(self) -> (SharedString, Option<SharedString>) {
        (self.title, self.capture_key)
    }
}

impl From<&str> for StorySectionTitle {
    fn from(title: &str) -> Self {
        Self::new(title)
    }
}

impl From<String> for StorySectionTitle {
    fn from(title: String) -> Self {
        Self::new(title)
    }
}

impl From<SharedString> for StorySectionTitle {
    fn from(title: SharedString) -> Self {
        Self::new(title)
    }
}

impl<T: Substory> From<T> for StorySectionTitle {
    fn from(substory: T) -> Self {
        Self::with_capture_key(substory.capture_key(), substory.title())
    }
}

/// Base capture metadata for a user-defined story section component.
///
/// Store this inside a custom section component, render the component with the
/// app's own layout and chrome, then call [`capture`](Self::capture) with the
/// rendered element from `RenderOnce`. The styled [`section`] helper uses this
/// same base type internally.
#[derive(Clone, Debug)]
pub struct StorySectionBase {
    title: SharedString,
    capture_key: Option<SharedString>,
}

impl StorySectionBase {
    /// Create capture metadata from a visible title, explicit section title, or
    /// `#[derive(Substory)]` enum variant.
    pub fn new(title: impl Into<StorySectionTitle>) -> Self {
        let (title, capture_key) = title.into().into_parts();

        Self { title, capture_key }
    }

    /// Visible title supplied for this section.
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Explicit stable capture key, when one was supplied by a `Substory`
    /// variant or [`StorySectionTitle::with_capture_key`].
    pub fn capture_key(&self) -> Option<&SharedString> {
        self.capture_key.as_ref()
    }

    /// Wrap a rendered custom section in the capture marker.
    pub fn capture(self, child: impl IntoElement) -> AnyElement {
        if let Some(capture_key) = self.capture_key {
            capture_substory_with_key(capture_key, child).into_any_element()
        } else {
            capture_substory(self.title, child).into_any_element()
        }
    }
}

#[derive(IntoElement)]
pub struct StorySection {
    pub(super) capture: StorySectionBase,
    base: Div,
    pub(super) sub_title: Vec<AnyElement>,
    pub(super) children: Vec<AnyElement>,
}

impl StorySection {
    pub fn sub_title(mut self, sub_title: impl IntoElement) -> Self {
        self.sub_title.push(sub_title.into_any_element());
        self
    }

    #[allow(unused)]
    pub fn max_w_md(mut self) -> Self {
        self.base = self.base.max_w(rems(48.));
        self
    }

    #[allow(unused)]
    pub fn max_w_lg(mut self) -> Self {
        self.base = self.base.max_w(rems(64.));
        self
    }

    #[allow(unused)]
    pub fn max_w_xl(mut self) -> Self {
        self.base = self.base.max_w(rems(80.));
        self
    }

    #[allow(unused)]
    pub fn max_w_2xl(mut self) -> Self {
        self.base = self.base.max_w(rems(96.));
        self
    }
}

impl ParentElement for StorySection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for StorySection {
    fn style(&mut self) -> &mut gpui_kit::StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for StorySection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let capture = self.capture;
        let title = capture.title().clone();
        let group = GroupBox::new()
            .id(title.clone())
            .outline()
            .title(
                h_flex()
                    .justify_between()
                    .w_full()
                    .gap_4()
                    .child(title)
                    .children(self.sub_title),
            )
            .content_style(
                StyleRefinement::default()
                    .rounded(cx.theme().radius_lg)
                    .overflow_x_hidden()
                    .items_center()
                    .justify_center(),
            )
            .child(self.base.children(self.children));

        capture.capture(group)
    }
}

pub fn section(title: impl Into<StorySectionTitle>) -> StorySection {
    StorySection {
        capture: StorySectionBase::new(title),
        sub_title: vec![],
        base: h_flex()
            .flex_wrap()
            .justify_center()
            .items_center()
            .w_full()
            .gap_4(),
        children: vec![],
    }
}
