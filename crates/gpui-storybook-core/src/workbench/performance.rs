use super::*;

impl StoryWorkbench {
    #[cfg(feature = "performance")]
    pub(super) fn render_performance(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        struct Metric {
            label: &'static str,
            samples: u64,
            p50_ms: f64,
            p95_ms: f64,
            p99_ms: f64,
            max_ms: f64,
        }

        macro_rules! duration_metric {
            ($label:literal, $histogram:expr) => {{
                let histogram = &$histogram;
                Metric {
                    label: $label,
                    samples: histogram.len(),
                    p50_ms: histogram.value_at_quantile(0.50) as f64 / 1_000_000.0,
                    p95_ms: histogram.value_at_quantile(0.95) as f64 / 1_000_000.0,
                    p99_ms: histogram.value_at_quantile(0.99) as f64 / 1_000_000.0,
                    max_ms: histogram.max() as f64 / 1_000_000.0,
                }
            }};
        }

        let frames = window.frame_duration_snapshot();
        let input = window.input_latency_snapshot();
        let metrics = [
            duration_metric!("Draw duration", frames.draw_duration_histogram),
            duration_metric!("Dirty to present", frames.dirty_to_present_histogram),
            duration_metric!("Present interval", frames.present_interval_histogram),
            duration_metric!("Input to frame", input.latency_histogram),
        ];
        let overlay_mode = format!("{:?}", window.debug_frame_overlay_mode());

        v_flex()
            .id("workbench-performance")
            .p_4()
            .gap_3()
            .child(
                h_flex().justify_end().gap_2().child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("performance-refresh")
                                .label("Refresh")
                                .xsmall()
                                .on_click(|_, window, _| {
                                    window.refresh();
                                }),
                        )
                        .child(
                            Button::new("performance-cycle-overlay")
                                .label(format!("Overlay: {overlay_mode}"))
                                .xsmall()
                                .on_click(|_, window, _| {
                                    window.cycle_debug_frame_overlay_mode();
                                }),
                        ),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "Input events dropped during draw: {}",
                        input.mid_draw_events_dropped
                    )),
            )
            .children(metrics.into_iter().map(|metric| {
                v_flex()
                    .gap_1()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .justify_between()
                            .child(metric.label)
                            .child(format!("{} samples", metric.samples)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "p50 {:.2} ms  ·  p95 {:.2} ms  ·  p99 {:.2} ms  ·  max {:.2} ms",
                                metric.p50_ms, metric.p95_ms, metric.p99_ms, metric.max_ms
                            )),
                    )
            }))
            .into_any_element()
    }
}
