#!/usr/bin/env python3
"""Native touch/IME and lifecycle proof through the real bounded device endpoint."""
import argparse
import json
from pathlib import Path
import re
import socket
import struct
import subprocess
import time
import xml.etree.ElementTree as ET
from png import read_png

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--serial", required=True)
parser.add_argument("--output", type=Path, default=ROOT / "target/mobile-evidence")
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
transcript = []

def adb(*command):
    return subprocess.check_output(["adb", "-s", args.serial, *command], timeout=20)

port = int(adb("forward", "tcp:0", "tcp:28437").strip())
session = ""
request_id = time.time_ns()

def connect():
    return socket.create_connection(("127.0.0.1", port), timeout=30)

def message(command, version=1, identity=None, id=None):
    global request_id
    request_id += 1
    return {"protocol_version": version, "session": session if identity is None else identity,
            "request_id": request_id if id is None else id, "command": command}

def send(stream, request):
    body = json.dumps(request).encode()
    stream.sendall(struct.pack(">I", len(body)) + body)

def exact(stream, length):
    body = bytearray()
    while len(body) < length:
        data = stream.recv(length - len(body))
        if not data:
            raise EOFError("device response closed")
        body.extend(data)
    return body

def receive(stream):
    length, = struct.unpack(">I", exact(stream, 4))
    assert 0 < length <= 1024 * 1024
    return json.loads(exact(stream, length))

def rpc(command, error=None, version=1, identity=None, id=None):
    request = message(command, version, identity, id)
    with connect() as stream:
        send(stream, request)
        response = receive(stream)
    transcript.append({"request": request, "response": response})
    assert response["request_id"] == request["request_id"]
    outcome = response["outcome"]
    if error:
        assert outcome["Err"]["code"] == error, outcome
        return outcome["Err"]
    if "Err" in outcome:
        raise RuntimeError(outcome["Err"])
    return outcome["Ok"]["value"]

def ready(predicate=lambda host: True, timeout=15):
    global session
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            host = rpc({"operation": "get_host"}, identity="")
            if host["active_route"] == host["native_route"] and predicate(host):
                session = host["session"]
                return host
        except (RuntimeError, EOFError, OSError):
            pass
        time.sleep(.05)
    raise TimeoutError("device rendered readiness")

def state():
    values = rpc({"operation": "read_values"})["values"]
    return next(value["value"] for value in values if value["key"] == "public-state")

def wait_state(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            value = state()
            if predicate(value):
                return value
        except RuntimeError:
            pass
        time.sleep(.025)
    raise TimeoutError("rendered public state")

def nodes():
    adb("shell", "uiautomator", "dump", "/data/local/tmp/storybook-window.xml")
    return list(ET.fromstring(adb("shell", "cat", "/data/local/tmp/storybook-window.xml")).iter("node"))

def tap_node(node):
    left, top, right, bottom = map(int, re.findall(r"\d+", node.attrib["bounds"]))
    adb("shell", "input", "tap", str((left + right) // 2), str((top + bottom) // 2))

def native_tab(label):
    tap_node(next(node for node in nodes() if node.attrib.get("resource-id") == "storybook." + label.lower()))

def target_touch(key):
    host = ready()
    target = next(target for target in rpc({"operation": "list_targets"})["targets"] if target["key"] == key)
    bounds, geometry = target["bounds"], host["geometry"]
    x = geometry["x"] + geometry["scale"] * (bounds["x"] + bounds["width"] / 2)
    y = geometry["y"] + geometry["scale"] * (bounds["y"] + bounds["height"] / 2)
    assert geometry["x"] <= x < geometry["x"] + geometry["width"]
    assert geometry["y"] <= y < geometry["y"] + geometry["height"]
    adb("shell", "input", "tap", str(round(x)), str(round(y)))
    transcript.append({"native_touch": {"key": key, "x": x, "y": y, "host": host}})

try:
    ready()
    adb("shell", "cmd", "window", "user-rotation", "lock", "0")
    ready(lambda host: host["orientation"] == "portrait")
    pid = adb("shell", "pidof", "dev.storybook.mobile").strip()
    rpc({"operation": "open_story", "key": "embedded-counter"})
    original = ready(lambda host: host["geometry"]["height"] > host["geometry"]["display_height"] * .65)
    # Presentation is a typed device-protocol field. Native and GPUI appearance
    # must agree even when a batch keeps the current route.
    background_pixels = {}
    for background, expected in [("light", False), ("dark", True), ("light", False)]:
        rpc({"operation": "run_steps", "request": {
            "presentation": {"background": background, "viewport": "responsive"},
            "steps": [{"type": "wait_frames", "count": 1}]}})
        assert ready()["dark"] is expected
        ticket = rpc({"operation": "prepare_capture"})
        try:
            path = args.output / ("counter-direct-" + background + ".png")
            path.write_bytes(adb("exec-out", "screencap", "-p"))
            width, height, channels, rows = read_png(path)
            geometry = ticket["host"]["geometry"]
            assert (width, height) == (geometry["display_width"], geometry["display_height"])
            x, y = 4, geometry["y"] + geometry["height"] - 40
            pixel = rows[y][x * channels:(x + 1) * channels]
            if background in background_pixels:
                assert pixel == background_pixels[background]
            background_pixels[background] = pixel
        finally:
            rpc({"operation": "finish_capture", "ticket": ticket["ticket"]})
    assert background_pixels["dark"] != background_pixels["light"]
    transcript.append({"presentation_pixels": {
        key: list(value) for key, value in background_pixels.items()}})
    print("native/device presentation and rendered pixels passed", flush=True)
    before = state()["count"]
    target_touch("increment")
    wait_state(lambda value: value["count"] == before + 1)
    print("native surface touch passed", flush=True)
    native_tab("NOTES")
    ready(lambda host: host["active_route"] == "embedded-notes" and host["geometry"]["height"] < original["geometry"]["height"])
    rpc({"operation": "set_control", "key": "note", "value": {"type": "text", "value": ""}})
    target_touch("note-input")
    # The declared AOSP en-US keyboard draws keys without accessibility nodes.
    # Tap its observed physical layout, separately from GPUI text insertion.
    ime = adb("shell", "settings", "get", "secure", "default_input_method").decode().strip()
    assert ime == "com.android.inputmethod.latin/.LatinIME", f"qualify this keyboard layout separately: {ime}"
    host = ready(lambda host: host["geometry"]["height"] < original["geometry"]["height"])
    keyboard_top = host["geometry"]["y"] + host["geometry"]["height"]
    keyboard_bottom = original["geometry"]["y"] + original["geometry"]["height"]
    width = host["geometry"]["display_width"]
    scale = host["geometry"]["scale"]
    # LatinIME can hide its suggestion strip without moving its key rows.
    # The qualified portrait layout places row/space centers relative to its
    # observed bottom, in density-independent pixels.
    assert keyboard_bottom - keyboard_top >= 240 * scale
    adb("shell", "input", "tap", str(round(width * .1)), str(round(keyboard_bottom - 160 * scale)))
    wait_state(lambda value: value["note"] == "a")
    adb("shell", "input", "tap", str(round(width * .5)), str(round(keyboard_bottom - 40 * scale)))
    wait_state(lambda value: value["note"] == "a ")
    transcript.append({"native_ime": "AOSP soft-keyboard a and space committed through InputConnection", "keyboard_top": keyboard_top, "host": host})
    print("native IME commit and insets passed", flush=True)
    adb("shell", "input", "keyevent", "4")
    native_tab("COUNTER")
    ready(lambda host: host["active_route"] == "embedded-counter")
    before = state()["count"]
    long_request = message({"operation": "run_steps", "request": {"steps": [
        {"type": "wait_frames", "count": 60}, {"type": "click_target", "target_key": "increment"}]}})
    stream = connect()
    send(stream, long_request)
    # A read acknowledgment follows owner admission; no fixed timing assumption.
    rpc({"operation": "current_story"})
    rpc({"operation": "open_story", "key": "embedded-notes"}, error="automation_busy")
    stream.shutdown(socket.SHUT_RDWR)
    stream.close()
    rpc(long_request["command"], error="stale_host", id=long_request["request_id"])
    wait_state(lambda value: value["count"] == before + 1, timeout=40)
    # Admission of another operation proves the disconnected batch released its lease.
    rpc({"operation": "open_story", "key": "embedded-counter"})
    assert state()["count"] == before + 1
    print("disconnect, busy, and duplicate admission passed", flush=True)
    failed = rpc({"operation": "run_steps", "request": {"steps": [
        {"type": "click_target", "target_key": "increment"},
        {"type": "click_target", "target_key": "missing-runtime-target"}]}}, error="interaction_failed")
    assert failed["steps_dispatched"] == 1
    assert state()["count"] == before + 2
    print("partial dispatch without retry passed", flush=True)
    rpc({"operation": "open_story", "key": "embedded-counter"}, version=2, error="protocol_mismatch")
    ticket = rpc({"operation": "prepare_capture"})
    rpc({"operation": "open_story", "key": "embedded-notes"}, error="automation_busy")
    native_tab("NOTES")
    ready(lambda host: host["active_route"] == "embedded-notes")
    rpc({"operation": "finish_capture", "ticket": ticket["ticket"]}, error="stale_host")
    rpc({"operation": "open_story", "key": "embedded-counter"})
    print("capture lease and stale interval passed", flush=True)
    before = state()["count"]
    old_session = session
    adb("shell", "input", "keyevent", "3")
    rpc({"operation": "get_host"}, error="no_live_host")
    adb("shell", "am", "start", "-n", "dev.storybook.mobile/.MainActivity", "--ez", "storybook_automation", "true")
    resumed = ready()
    assert adb("shell", "pidof", "dev.storybook.mobile").strip() == pid
    assert state()["count"] == before
    print("pause/resume retained runtime passed", flush=True)
    old_session = session
    adb("shell", "cmd", "window", "user-rotation", "lock", "1")
    rotated = ready(lambda host: host["orientation"] == "landscape" and host["session"] != old_session)
    assert rotated["surface_revision"] > resumed["surface_revision"]
    assert adb("shell", "pidof", "dev.storybook.mobile").strip() == pid
    assert state()["count"] == before
    rpc({"operation": "open_story", "key": "embedded-notes"}, identity=old_session, error="stale_host")
    rpc({"operation": "list_targets"})
    print("activity recreation and stale session passed", flush=True)
    print("Android native lifecycle proof passed", flush=True)
finally:
    adb("shell", "cmd", "window", "user-rotation", "lock", "0")
    adb("shell", "cmd", "window", "user-rotation", "free")
    adb("forward", "--remove", f"tcp:{port}")
    (args.output / "native-transcript.json").write_text(json.dumps(transcript, indent=2) + "\n")
