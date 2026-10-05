#!/usr/bin/env python3
"""Build the dependency-free native Android shell with the selected SDK and NDK."""
import argparse
import os
from pathlib import Path
import subprocess
import zipfile

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
args = parser.parse_args()
if not args.sdk or not args.ndk:
    parser.error("provide --sdk/--ndk or ANDROID_HOME/ANDROID_NDK_HOME")
tools = args.sdk / "build-tools" / args.build_tools
android = args.sdk / "platforms" / f"android-{args.platform}" / "android.jar"
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
def run(*command):
    subprocess.run([str(value) for value in command], cwd=ROOT, env=env, check=True)

build = ROOT / "target" / "mobile-example" / args.abi
classes, dex = build / "classes", build / "dex"
classes.mkdir(parents=True, exist_ok=True)
dex.mkdir(parents=True, exist_ok=True)
cargo = ["cargo", "build", "-p", "gpui-storybook-example-mobile", "--locked", "--target", target, "--profile", args.profile]
if args.automation:
    cargo += ["--features", "automation"]
run(*cargo)
run("javac", "-source", "11", "-target", "11", "-classpath", android, "-d", classes,
    HERE / "android" / "MainActivity.java")
class_files = sorted(classes.rglob("*.class"))
run(tools / "d8", "--lib", android, "--min-api", "31", "--output", dex, *class_files)
unaligned, unsigned, apk = build / "unaligned.apk", build / "unsigned.apk", build / "storybook.apk"
run(tools / "aapt2", "link", "-I", android, "--manifest", HERE / "android" / "AndroidManifest.xml", "-o", unaligned)
with zipfile.ZipFile(unaligned, "a") as archive:
    archive.write(dex / "classes.dex", "classes.dex")
    archive.write(HERE / "android" / "LICENSE-APACHE", "assets/licenses/gpui-mobile/LICENSE-APACHE")
    archive.write(HERE / "android" / "NOTICE", "assets/licenses/gpui-mobile/NOTICE")
    archive.write(ROOT / "target" / target / ("debug" if args.profile == "dev" else args.profile) / "libgpui_storybook_example_mobile.so",
                  f"lib/{args.abi}/libgpui_storybook_example_mobile.so")
run(tools / "zipalign", "-f", "-p", "4", unaligned, unsigned)
key = ROOT / "target" / "mobile-example" / "debug.keystore"
if not key.exists():
    run("keytool", "-genkeypair", "-keystore", key, "-storepass", "android", "-keypass", "android",
        "-alias", "androiddebugkey", "-dname", "CN=Storybook Example", "-keyalg", "RSA", "-validity", "3650")
run(tools / "apksigner", "sign", "--ks", key, "--ks-pass", "pass:android", "--out", apk, unsigned)
run(tools / "apksigner", "verify", apk)
print(apk)
