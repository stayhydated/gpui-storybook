use std::{path::PathBuf, process::Command};

#[test]
fn duplicate_story_keys_fail_to_build() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = manifest_dir.join("tests/fixtures/duplicate-story-key");

    let output = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--quiet")
        .arg("--locked")
        // The nested fixture build verifies a linker diagnostic, not runtime
        // behavior. Keep the parent coverage instrumentation from leaking into
        // the child Cargo process and writing profiles beside fixture sources.
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_LLVM_COV")
        .env_remove("CARGO_LLVM_COV_BUILD_DIR")
        .env_remove("CARGO_LLVM_COV_SHOW_ENV")
        .env_remove("CARGO_LLVM_COV_TARGET_DIR")
        .env(
            "CARGO_TARGET_DIR",
            manifest_dir.join("../../target/duplicate-story-key-fixture"),
        )
        .env_remove("LLVM_PROFILE_FILE")
        .env_remove("RUSTDOCFLAGS")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("__CARGO_LLVM_COV_RUSTC_WRAPPER")
        .env_remove("__CARGO_LLVM_COV_RUSTC_WRAPPER_CRATE_NAMES")
        .env_remove("__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS")
        .current_dir(fixture)
        .output()
        .expect("fixture build should run");

    assert!(
        !output.status.success(),
        "duplicate story key fixture unexpectedly built successfully"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "__gpui_storybook_story_key__gpui-storybook-duplicate-fixture__DuplicateStory"
        ) || stderr.contains("multiple definition")
            || stderr.contains("defined multiple times"),
        "duplicate story key fixture failed without the expected duplicate symbol diagnostic:\n{stderr}"
    );
}
