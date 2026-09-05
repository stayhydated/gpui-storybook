use super::*;

pub(super) fn install_live_automation(cx: &mut ::gpui_kit::App) {
    if gpui_storybook_core::automation::default_storybook_automation(cx).is_none() {
        gpui_storybook_core::automation::set_default_storybook_automation(
            cx,
            gpui_storybook_core::automation::StorybookAutomation::new(),
        );
    }
}

/// Returns the read-only saved and resolved preference snapshot after
/// initialization has begun.
///
/// The snapshot reports `Loading` until the readiness task completes. It stays
/// available in `Error` state when storage fails and fallback presentation is
/// active.
pub fn try_preference_state(cx: &::gpui_kit::App) -> Option<&PreferenceState> {
    gpui_storybook_core::preferences::try_state(cx)
}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
pub(super) fn start_mcp_automation(cx: &mut ::gpui_kit::App) {
    let automation = gpui_storybook_core::automation::default_storybook_automation(cx)
        .expect("gpui-storybook init should install live automation before MCP startup");

    if gpui_storybook_mcp::stdio_requested() {
        match gpui_storybook_mcp::start_stdio(automation.clone()) {
            Ok(completion) => {
                cx.spawn(async move |cx| {
                    let exit_code = match completion.await {
                        Ok(()) => 0,
                        Err(error) => {
                            eprintln!("gpui-storybook MCP stdio server failed: {error}");
                            1
                        },
                    };
                    cx.update(move |_cx| {
                        // A stdio session owns this process. Exit directly after
                        // transport completion so native window and renderer
                        // teardown cannot race the headless compositor cleanup.
                        exit_after_mcp_stdio(exit_code);
                    });
                })
                .detach();
            },
            Err(error) => {
                eprintln!("failed to start gpui-storybook MCP stdio server: {error}");
                cx.quit();
            },
        }
    }

    if let Err(error) = gpui_storybook_mcp::start_capture_session_from_env(automation) {
        eprintln!("failed to start storybook capture session: {error}");
    }
}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
fn exit_after_mcp_stdio(exit_code: i32) -> ! {
    // SAFETY: the stdio transport has completed and the automation session owns
    // this process. `_exit` avoids native platform and thread-local teardown.
    unsafe { libc::_exit(exit_code) }
}
