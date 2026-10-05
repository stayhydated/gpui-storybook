#!/usr/bin/env python3
"""Build the Kotlin native qualification APK with pinned Gradle and AndroidX."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import shutil
from urllib.request import urlopen
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--sdk", type=Path, required=True)
parser.add_argument("--write-gradle-locks", action="store_true", help="Refresh Kotlin dependency locks and SHA-256 verification metadata")
args = parser.parse_args()
tools = args.sdk / "build-tools/36.0.0"
build = ROOT / "target/mobile-native-apk"
build.mkdir(parents=True, exist_ok=True)
downloads = build / "downloads"
downloads.mkdir(exist_ok=True)
jars = build / "dependencies"
shutil.rmtree(jars, ignore_errors=True)
jars.mkdir()
dependencies = json.loads((HERE / "androidx-dependencies.json").read_text())
for dependency in dependencies:
    path = downloads / dependency["filename"]
    if not path.exists():
        path.write_bytes(urlopen(dependency["url"], timeout=30).read())
    data = path.read_bytes()
    assert hashlib.sha256(data).hexdigest() == dependency["sha256"], path
    if path.suffix == ".aar":
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for name in archive.namelist():
                if name == "classes.jar" or (name.startswith("libs/") and name.endswith(".jar")):
                    jar = jars / (path.stem + "-" + name.replace("/", "-"))
                    jar.write_bytes(archive.read(name))
    else:
        shutil.copyfile(path, jars / path.name)


env = dict(os.environ)
env["ANDROID_HOME"] = str(args.sdk.resolve())
env["ANDROID_SDK_ROOT"] = str(args.sdk.resolve())


def run(*command, cwd=ROOT):
    subprocess.run([str(value) for value in command], cwd=cwd, env=env, check=True)


assets = build / "assets"
assets.mkdir(exist_ok=True)
shutil.copyfile(HERE / "androidx-dependencies.json", assets / "dependencies.json")
project = HERE.parent / "android"
gradle = [project / "gradlew", "--no-daemon", ":native-qualification:assembleDebug",
          f"-PstorybookBuildRoot={build / 'gradle'}", f"-PstorybookNativeJars={jars}",
          f"-PstorybookNativeAssets={assets}"]
if args.write_gradle_locks:
    gradle += ["--write-locks", "--write-verification-metadata", "sha256"]
run(*gradle, cwd=project)
unsigned = build / "gradle/native-qualification/outputs/apk/debug/native-qualification-debug.apk"
apk = build / "androidx.apk"
key = build / "debug.keystore"
if not key.exists():
    run("keytool", "-genkeypair", "-keystore", key, "-storepass", "android", "-keypass", "android",
        "-alias", "androiddebugkey", "-dname", "CN=Storybook Native Qualification", "-keyalg", "RSA", "-validity", "3650")
run(tools / "apksigner", "sign", "--ks", key, "--ks-pass", "pass:android", "--out", apk, unsigned)
run(tools / "apksigner", "verify", apk)
print(apk)
