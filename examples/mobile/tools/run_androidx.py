#!/usr/bin/env python3
"""Install/run the isolated AndroidX instrumentation and retain its artifacts."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--serial", required=True)
parser.add_argument("--apk", type=Path, default=ROOT / "target/android-tools-apk/androidx.apk")
parser.add_argument("--output", type=Path, default=ROOT / "target/android-tools-evidence")
args = parser.parse_args()
if not args.serial.startswith("emulator-"):
    parser.error("this proof requires a disposable emulator with AOSP en-US LatinIME")
args.output.mkdir(parents=True, exist_ok=True)


def adb(*command, timeout=30):
    return subprocess.check_output(["adb", "-s", args.serial, *command], timeout=timeout)


try:
    print(adb("install", "--no-incremental", "-r", str(args.apk)).decode(), end="")
    result = adb("shell", "am", "instrument", "-w", "-r", "dev.storybook.tools/.Probe", timeout=180)
    (args.output / "androidx-instrumentation.txt").write_bytes(result)
    print(result.decode(), end="")
    data = adb("exec-out", "run-as", "dev.storybook.tools", "cat", "files/report.json")
    if data.startswith(b"{"):
        (args.output / "androidx-report.json").write_bytes(data)
    assert b"INSTRUMENTATION_CODE: -1" in result, result.decode()
    for remote, local in [("hierarchy.xml", "androidx.xml"), ("androidx.png", "androidx.png")]:
        data = adb("exec-out", "run-as", "dev.storybook.tools", "cat", "files/" + remote)
        (args.output / local).write_bytes(data)
    report = json.loads((args.output / "androidx-report.json").read_bytes())
    assert report.get("passed") is True, report
    print("AndroidX UI Automator 2.4.0 native proof passed")
finally:
    adb("shell", "am", "force-stop", "dev.storybook.tools")
