#!/usr/bin/env python3
"""Build a test-only APK from hash-pinned AndroidX artifacts without Gradle."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import shutil
from urllib.request import urlopen
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--sdk", type=Path, required=True)
args = parser.parse_args()
tools = args.sdk / "build-tools/36.0.0"
android = args.sdk / "platforms/android-36/android.jar"
build = ROOT / "target/mobile-native-apk"
build.mkdir(parents=True, exist_ok=True)
jars = []
dependencies = json.loads((HERE / "androidx-dependencies.json").read_text())
for dependency in dependencies:
    path = build / dependency["filename"]
    if not path.exists():
        path.write_bytes(urlopen(dependency["url"], timeout=30).read())
    data = path.read_bytes()
    assert hashlib.sha256(data).hexdigest() == dependency["sha256"], path
    if path.suffix == ".aar":
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for name in archive.namelist():
                if name == "classes.jar" or (name.startswith("libs/") and name.endswith(".jar")):
                    jar = build / (path.stem + "-" + name.replace("/", "-"))
                    jar.write_bytes(archive.read(name))
                    jars.append(jar)
    else:
        jars.append(path)


def run(*command):
    subprocess.run([str(value) for value in command], check=True)


classes, dex = build / "classes", build / "dex"
shutil.rmtree(classes, ignore_errors=True)
shutil.rmtree(dex, ignore_errors=True)
classes.mkdir()
dex.mkdir()
run("javac", "-source", "8", "-target", "8", "-Xlint:-options", "-bootclasspath",
    str(tools / "core-lambda-stubs.jar") + ":" + str(android), "-classpath",
    ":".join(str(p) for p in jars), "-d", classes, HERE / "androidx/NativeQualification.java")
compiled = build / "probe.jar"
run("jar", "cf", compiled, "-C", classes, ".")
run(tools / "d8", "--lib", android, "--min-api", "31", "--output", dex, compiled, *jars)
unaligned, unsigned, apk = [build / name for name in ("unaligned.apk", "unsigned.apk", "androidx.apk")]
run(tools / "aapt2", "link", "-I", android, "--manifest", HERE / "androidx/AndroidManifest.xml",
    "-o", unaligned)
with zipfile.ZipFile(unaligned, "a") as archive:
    archive.write(dex / "classes.dex", "classes.dex")
    archive.write(HERE / "androidx-dependencies.json", "assets/dependencies.json")
run(tools / "zipalign", "-f", "-p", "4", unaligned, unsigned)
key = build / "debug.keystore"
if not key.exists():
    run("keytool", "-genkeypair", "-keystore", key, "-storepass", "android", "-keypass", "android",
        "-alias", "androiddebugkey", "-dname", "CN=Storybook Native Qualification", "-keyalg", "RSA", "-validity", "3650")
run(tools / "apksigner", "sign", "--ks", key, "--ks-pass", "pass:android", "--out", apk, unsigned)
run(tools / "apksigner", "verify", apk)
print(apk)
