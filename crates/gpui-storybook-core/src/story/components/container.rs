use super::metadata::recreate_story;
use super::*;

pub fn story_group_klass(stories: &[Entity<StoryContainer>], cx: &App) -> SharedString {
    let mut klasses = stories
        .iter()
        .filter_map(|story| story.read(cx).story_klass.clone())
        .map(|klass| klass.to_string())
        .collect::<Vec<_>>();
    klasses.sort();

    format!("{}{}", STORY_GROUP_KLASS_PREFIX, klasses.join("|")).into()
}

pub fn parse_story_group_klass(story_klass: &str) -> Option<Vec<String>> {
    let members = story_klass.strip_prefix(STORY_GROUP_KLASS_PREFIX)?;
    Some(
        members
            .split('|')
            .filter(|member| !member.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

#[derive(Debug)]
pub enum ContainerEvent {
    Close,
    /// The concrete story entity and its runtime adapters were replaced.
    Recreated {
        /// Monotonic identity for the newly installed story instance.
        generation: u64,
    },
}

pub trait Story: Focusable + Render + StoryControls + Sized {
    fn klass() -> &'static str {
        let type_name = std::any::type_name::<Self>();
        type_name.rsplit("::").next().unwrap_or(type_name)
    }

    fn title(cx: &App) -> String;
    fn description(cx: &App) -> String {
        let _ = cx;
        "".to_owned()
    }
    fn closable() -> bool {
        true
    }
    fn zoomable() -> Option<PanelControl> {
        Some(PanelControl::default())
    }
    fn title_bg() -> Option<Hsla> {
        None
    }
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<Self>;

    /// Returns the story-root focus handle whose GPUI actions are exposed in
    /// the workbench.
    ///
    /// Track this handle on the element that installs the page or component
    /// action handlers. Keep it separate from [`Focusable::focus_handle`] when
    /// the story's primary interaction focus belongs to a nested control such
    /// as an input. Stories opt in explicitly so actions owned by child
    /// controls or the surrounding Storybook shell are never inferred as
    /// story actions.
    fn action_scope_focus_handle(&self, _cx: &App) -> Option<gpui_kit::FocusHandle> {
        None
    }

    /// Returns reusable, story-owned interaction scenarios.
    ///
    /// Each invocation is copied into the runtime story container and can be
    /// listed or run by the Storybook UI and automation integrations. The
    /// default implementation keeps existing stories scenario-free.
    fn scenarios() -> Vec<StoryScenario> {
        Vec::new()
    }

    fn on_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
        let _ = active;
        let _ = window;
        let _ = cx;
    }
    fn on_active_any(view: AnyView, active: bool, window: &mut Window, cx: &mut App)
    where
        Self: 'static,
    {
        if let Ok(story) = view.downcast::<Self>() {
            cx.update_entity(&story, |story, cx| {
                story.on_active(active, window, cx);
            });
        }
    }
}

impl EventEmitter<ContainerEvent> for StoryContainer {}

impl StoryContainer {
    pub fn new(_window: &mut Window, cx: &mut App) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            action_scope_focus_handle: None,
            name: "".into(),
            group: None,
            section: None,
            title_bg: None,
            description: "".into(),
            variants: Vec::new(),
            variant_group: None,
            scroll_handle: ScrollHandle::new(),
            story_scroll_handle: ScrollHandle::new(),
            width: None,
            height: None,
            tab_group: None,
            story: None,
            control_target: None,
            presentation: StoryPresentation::default(),
            responsive_size: None,
            canvas_bounds: None,
            canvas_stage_bounds: None,
            canvas_resize_drag: None,
            automation_size: None,
            workbench_state: None,
            story_klass: None,
            registration_metadata: None,
            story_key: None,
            story_name: None,
            crate_name: None,
            source_file: None,
            source_line: None,
            closable: true,
            is_active: false,
            zoomable: Some(PanelControl::default()),
            on_active: None,
            title_fn: None,
            description_fn: None,
            scenarios: Vec::new(),
            recreate: None,
            recreation_generation: 0,
        }
    }

    pub fn section(mut self, section: impl Into<SharedString>) -> Self {
        self.section = Some(section.into());
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn sidebar_group(&self) -> Option<SharedString> {
        self.group.clone().or(self.section.clone())
    }

    pub fn sidebar_section(&self) -> Option<SharedString> {
        match (&self.group, &self.section) {
            (Some(group), Some(section)) if group != section => Some(section.clone()),
            _ => None,
        }
    }

    pub fn panel<S: Story>(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let name = S::title(cx);
        let description = S::description(cx);
        let (story, control_target, focus_handle, action_scope_focus_handle) =
            recreate_story::<S>(window, cx);
        let story_klass = S::klass();
        let scenarios = S::scenarios();

        cx.new(|cx| {
            let mut story = Self::new(window, cx)
                .story(story, story_klass)
                .on_active(S::on_active_any);
            story.control_target = control_target;
            story.focus_handle = focus_handle;
            story.action_scope_focus_handle = action_scope_focus_handle;
            story.closable = S::closable();
            story.zoomable = S::zoomable();
            story.name = name.into();
            story.description = description.into();
            story.title_bg = S::title_bg();
            story.title_fn = Some(Box::new(S::title));
            story.description_fn = Some(Box::new(S::description));
            story.scenarios = scenarios;
            story.recreate = Some(recreate_story::<S>);
            story
        })
    }

    /// Creates a navigation descriptor for stories that share one visible title.
    ///
    /// Hosts render or open the selected concrete variant instead of mounting
    /// this descriptor as a panel.
    pub fn variant_group(
        name: impl Into<SharedString>,
        stories: Vec<Entity<StoryContainer>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = name.into();
        let story_klass = story_group_klass(&stories, cx);
        let description = format!("{} story variants", stories.len());
        let group = cx.new(|cx| {
            let mut story = Self::new(window, cx);
            story.name = name;
            story.description = description.into();
            story.story_klass = Some(story_klass);
            story.variants = stories.clone();
            story
        });
        let weak_group = group.downgrade();
        for story in stories {
            story.update(cx, |story, _| {
                story.variant_group = Some(weak_group.clone());
            });
        }
        group
    }

    pub(crate) fn variant_label(&self, cx: &App) -> String {
        let description = self.display_description(cx);
        if !description.is_empty() {
            return description;
        }

        self.story_name
            .as_ref()
            .or(self.story_klass.as_ref())
            .map(ToString::to_string)
            .unwrap_or_else(|| self.display_title(cx))
    }

    pub fn width(mut self, width: gpui_kit::Pixels) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: gpui_kit::Pixels) -> Self {
        self.height = Some(height);
        self
    }

    pub fn story(mut self, story: AnyView, story_klass: impl Into<SharedString>) -> Self {
        self.story = Some(story);
        self.story_klass = Some(story_klass.into());
        self
    }

    pub fn on_active(mut self, on_active: fn(AnyView, bool, &mut Window, &mut App)) -> Self {
        self.on_active = Some(on_active);
        self
    }

    /// Returns the controls for this concrete story instance.
    pub fn control_target(&self) -> Option<Rc<dyn ControlTarget>> {
        self.control_target.clone()
    }

    /// Returns the explicit story-root focus handle used by the Actions
    /// workbench, when the story exposes one.
    pub fn action_scope_focus_handle(&self) -> Option<gpui_kit::FocusHandle> {
        self.action_scope_focus_handle.clone()
    }

    /// Returns the immutable scenarios declared by this story type.
    pub fn scenarios(&self) -> &[StoryScenario] {
        &self.scenarios
    }

    pub(crate) const fn recreation_generation(&self) -> u64 {
        self.recreation_generation
    }

    /// Recreates the concrete story entity and all runtime adapters used by it.
    ///
    /// Scenario runs use this seam before applying their initial controls and
    /// dispatching their first step. The workbench's Actions and Scenarios reset
    /// commands use the same seam without dispatching anything. Recreating the
    /// entity is stronger than resetting controls: story-owned counters, input
    /// buffers, subscriptions, and other transient state return to the type's
    /// constructor defaults. Active stories receive an `on_active(false)`
    /// callback for the old entity and an `on_active(true)` callback for the
    /// replacement. The replacement primary focus handle, optional action-scope
    /// focus handle, and control target are installed atomically from the
    /// container's perspective. A successful replacement emits
    /// [`ContainerEvent::Recreated`] with a new generation after every runtime
    /// adapter points at the fresh instance.
    pub fn recreate_for_scenario(
        &mut self,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> bool {
        let Some(recreate) = self.recreate else {
            return false;
        };

        let on_active = self.on_active;
        if self.is_active
            && let Some(on_active) = on_active
            && let Some(story) = self.story.clone()
        {
            on_active(story, false, window, cx);
        }

        let (story, control_target, focus_handle, action_scope_focus_handle) = recreate(window, cx);
        self.story = Some(story.clone());
        self.control_target = control_target;
        self.focus_handle = focus_handle;
        self.action_scope_focus_handle = action_scope_focus_handle;
        self.story_scroll_handle = ScrollHandle::new();
        self.canvas_resize_drag = None;
        self.recreation_generation = self
            .recreation_generation
            .checked_add(1)
            .expect("story recreation generation exhausted");

        if self.is_active
            && let Some(on_active) = on_active
        {
            on_active(story, true, window, cx);
        }
        cx.emit(ContainerEvent::Recreated {
            generation: self.recreation_generation,
        });
        cx.notify();
        true
    }
}
