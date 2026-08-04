use stayhydated_xtask::trunk::{TrunkDemoBuildConfig, TrunkDemoPageConfig};

pub fn run() -> anyhow::Result<()> {
    let workspace_root = stayhydated_xtask::workspace_root_from_xtask_manifest()?;
    stayhydated_xtask::trunk::build(
        &TrunkDemoBuildConfig::builder()
            .workspace_root(workspace_root)
            .example_dir("examples/story")
            .output_dir("web/public/gpui-demo")
            .example_name("demo")
            .required_marker("gpui-storybook-example-story")
            .toolchain("nightly")
            .generated_page(
                TrunkDemoPageConfig::builder()
                    .title("gpui-storybook gallery demo")
                    .demo_name("gpui-storybook gallery")
                    .build(),
            )
            .build(),
    )
}
