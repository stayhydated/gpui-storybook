use super::*;
use gpui::{AppContext as _, Styled as _};
use gpui_storybook_automation::wire::HostReadinessIssue;

/// Consumer fixture implements navigation and availability, with no control,
/// action, presentation, or fixture boilerplate.
struct NavigationRoot {
    route: &'static str,
    revision: u64,
    available: bool,
}
impl Render for NavigationRoot {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        capture_story_view_with_scroll(self.route, None, gpui::div().size_full())
    }
}
impl EmbeddedRoot for NavigationRoot {
    fn active_route(&self, _: &App) -> String {
        self.route.to_owned()
    }
    fn revision(&self, _: &App) -> u64 {
        self.revision
    }
    fn readiness(&self, _: &App) -> Result<(), HostReadinessIssue> {
        if self.available {
            Ok(())
        } else {
            Err(HostReadinessIssue::ApplicationUnavailable {
                message: "account content is hidden".to_owned(),
            })
        }
    }
    fn select_route(
        &mut self,
        route: &str,
        _: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        self.route = match route {
            "account" => "account",
            "scanner" => "scanner",
            _ => {
                return Err(StorybookAutomationError::StoryNotFound {
                    key: route.to_owned(),
                });
            },
        };
        self.revision += 1;
        cx.notify();
        Ok(())
    }
}

#[gpui_kit::test]
fn minimal_consumer_exposes_navigation_and_optional_defaults(cx: &mut gpui_kit::TestAppContext) {
    let (window, root) = cx.update(|cx| {
        let mut root = None;
        let window = cx
            .open_window(Default::default(), |_, cx| {
                let view = cx.new(|_| NavigationRoot {
                    route: "account",
                    revision: 0,
                    available: false,
                });
                root = Some(view.clone());
                view
            })
            .unwrap();
        (window, root.unwrap())
    });
    cx.update_window(window.into(), |_, window, cx| {
        let attachment = GpuiHostAttachment::attach(
            &root,
            vec![
                crate::EmbeddedRoute::new("account", "Account").into(),
                crate::EmbeddedRoute::new("scanner", "Scanner").into(),
            ],
            AutomationCapabilities::new([
                AutomationCapability::Navigation,
                AutomationCapability::Controls,
            ]),
            Rc::new(NoCaptureProvider),
            window,
            cx,
        )
        .unwrap();
        assert!(matches!(
            attachment.check_readiness(window, cx),
            Err(StorybookAutomationError::HostNotReady {
                issue: HostReadinessIssue::ApplicationUnavailable { .. }
            })
        ));
        root.update(cx, |root, _| root.available = true);
        assert_eq!(attachment.check_readiness(window, cx), Ok(()));
        attachment.open_story("scanner", window, cx).unwrap();
        let current = attachment.current_story(window, cx).unwrap();
        assert_eq!(current.story.unwrap().key, "scanner");
        assert!(current.revision > 0);
        assert!(
            attachment
                .read_controls(window, cx)
                .unwrap()
                .controls
                .is_empty()
        );
        assert_eq!(
            root.read(cx).read_control("unknown", cx),
            Err(ControlError::UnknownControl {
                key: "unknown".to_owned()
            })
        );
    })
    .unwrap();
}
