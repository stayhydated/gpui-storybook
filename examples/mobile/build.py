#!/usr/bin/env python3
"""Build the Rust GPUI library and pinned Jetpack Compose Android application."""
import argparse
import os
from pathlib import Path
import subprocess
import shutil
import tomllib

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument("--sdk", type=Path, default=os.environ.get("ANDROID_HOME"))
parser.add_argument("--ndk", type=Path, default=os.environ.get("ANDROID_NDK_HOME"))
parser.add_argument("--build-tools", default="36.0.0")
parser.add_argument("--platform", default="36")
parser.add_argument("--abi", choices=["x86_64", "arm64-v8a"], default="x86_64")
parser.add_argument("--automation", action="store_true")
parser.add_argument("--profile", default="mobile")
parser.add_argument("--write-gradle-locks", action="store_true", help="Refresh dependency locks and SHA-256 verification metadata")
args = parser.parse_args()
if not args.sdk or not args.ndk:
    parser.error("provide --sdk/--ndk or ANDROID_HOME/ANDROID_NDK_HOME")
tools = args.sdk / "build-tools" / args.build_tools
target, clang = {
    "x86_64": ("x86_64-linux-android", "x86_64-linux-android31-clang"),
    "arm64-v8a": ("aarch64-linux-android", "aarch64-linux-android31-clang"),
}[args.abi]
host = "linux-x86_64" if os.uname().sysname == "Linux" else "darwin-x86_64"
bin_dir = args.ndk / "toolchains" / "llvm" / "prebuilt" / host / "bin"
env = dict(os.environ)
env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(bin_dir / clang)
env[f"CC_{target.replace('-', '_')}"] = str(bin_dir / clang)
env[f"CXX_{target.replace('-', '_')}"] = str(bin_dir / f"{clang}++")
env[f"AR_{target.replace('-', '_')}"] = str(bin_dir / "llvm-ar")
def run(*command, cwd=ROOT):
    subprocess.run([str(value) for value in command], cwd=cwd, env=env, check=True)

build = ROOT / "target" / "mobile-example" / args.abi
build.mkdir(parents=True, exist_ok=True)
cargo = ["cargo", "build", "-p", "gpui-storybook-example-mobile", "--locked", "--target", target, "--profile", args.profile]
if args.automation:
    cargo += ["--features", "automation"]
run(*cargo)
library = ROOT / "target" / target / ("debug" if args.profile == "dev" else args.profile) / "libgpui_storybook_example_mobile.so"
jni = build / "jni" / args.abi
jni.mkdir(parents=True, exist_ok=True)
shutil.copyfile(library, jni / library.name)
licenses = build / "assets/licenses/gpui-mobile"
licenses.mkdir(parents=True, exist_ok=True)
for name in ("LICENSE-APACHE", "NOTICE"):
    shutil.copyfile(HERE / "android" / name, licenses / name)
version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
env["ANDROID_HOME"] = str(args.sdk.resolve())
env["ANDROID_SDK_ROOT"] = str(args.sdk.resolve())
gradle = [HERE / "android/gradlew", "--no-daemon", ":app:assembleDebug",
          f"-PstorybookBuildRoot={build / 'gradle'}", f"-PstorybookJni={build / 'jni'}",
          f"-PstorybookAssets={build / 'assets'}", f"-PstorybookAbi={args.abi}",
          f"-PstorybookPlatform={args.platform}", f"-PstorybookBuildTools={args.build_tools}",
          f"-PstorybookVersion={version}"]
if args.write_gradle_locks:
    gradle += ["--write-locks", "--write-verification-metadata", "sha256"]
run(*gradle, cwd=HERE / "android")
unsigned = build / "gradle/app/outputs/apk/debug/app-debug.apk"
apk = build / "storybook.apk"
key = ROOT / "target" / "mobile-example" / "debug.keystore"
if not key.exists():
    run("keytool", "-genkeypair", "-keystore", key, "-storepass", "android", "-keypass", "android",
        "-alias", "androiddebugkey", "-dname", "CN=Storybook Example", "-keyalg", "RSA", "-validity", "3650")
run(tools / "apksigner", "sign", "--ks", key, "--ks-pass", "pass:android", "--out", apk, unsigned)
run(tools / "apksigner", "verify", apk)
print(apk)
