#!/usr/bin/env python3
"""Real raw-stdio MCP proof against an already running, opted-in Android app."""
import argparse
import json
from pathlib import Path
import queue
import threading
import subprocess
import time
from png import read_png

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--serial", required=True)
parser.add_argument("--apk", type=Path, help="Install this APK for the lifecycle-only proof")
parser.add_argument("--lifecycle-only", action="store_true", help="Prove fresh launch, default interaction gating, and owned process cleanup and unchanged forwarding on EOF")
parser.add_argument("--output", type=Path, default=ROOT / "target" / "mobile-evidence")
args = parser.parse_args()
if args.apk and not args.lifecycle_only:
    parser.error("--apk requires --lifecycle-only")
args.output.mkdir(parents=True, exist_ok=True)
transcript = []
captures = []
command = [ROOT / "target/debug/gpui-storybook-mobile-host", "serve", "--serial", args.serial]
forwarding_before = None
if args.lifecycle_only:
    command += ["--config", str(ROOT / "examples/mobile/storybook.toml"), "--launch", "--stop-on-eof"]
    if args.apk:
        command += ["--install", str(args.apk)]
    forwarding_before = subprocess.check_output(["adb", "-s", args.serial, "forward", "--list"], timeout=15)
else:
    command += ["--allow-interaction"]
process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
next_id = 0
responses = queue.Queue(maxsize=32)
def read_responses():
    for line in process.stdout:
        responses.put(json.loads(line))
    responses.put(None)
threading.Thread(target=read_responses, daemon=True).start()

def rpc(method, params=None):
    global next_id
    next_id += 1
    request = {"jsonrpc": "2.0", "id": next_id, "method": method}
    if params is not None:
        request["params"] = params
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    deadline = time.monotonic() + 40
    while time.monotonic() < deadline:
        try:
            response = responses.get(timeout=max(.01, deadline - time.monotonic()))
        except queue.Empty:
            raise TimeoutError(method)
        if response is None:
            raise EOFError("MCP stdout closed")
        transcript.append({"request": request, "response": response})
        if response.get("id") == next_id:
            if "error" in response:
                raise RuntimeError(response["error"])
            return response["result"]
    raise TimeoutError(method)

def call(name, arguments=None, error=False):
    result = rpc("tools/call", {"name": name, "arguments": arguments or {}})
    if bool(result.get("isError", False)) != error:
        raise AssertionError(result)
    print(name, "rejected" if error else "passed", flush=True)
    structured = result.get("structuredContent", result)
    if not error and name in ("storybook_capture_host", "storybook_capture_current_story"):
        observation = structured.get("observation", structured)
        assert observation["provider"] == "adb_compositor_observation"
        assert observation["request_id"] > 0 and observation["host"]["session"]
        assert observation["host"]["active_route"] == observation["host"]["native_route"]
        captures.append(observation)
    return structured

def state():
    snapshot = call("storybook_read_semantic_values")
    return next(value["value"] for value in snapshot["values"] if value["key"] == "public-state")

def compose_state():
    snapshot = call("storybook_read_semantic_values")
    return next(value["value"] for value in snapshot["values"] if value["key"] == "native.compose-counter")

def compose_action(name, arguments=None, error=False):
    return call("storybook_dispatch_host_action", {"action": {"action": "invoke", "name": name,
                "arguments": {} if arguments is None else arguments}}, error=error)

try:
    rpc("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                       "clientInfo": {"name": "mobile-proof", "version": "1"}})
    process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    process.stdin.flush()
    tools = rpc("tools/list")["tools"]
    names = {tool["name"] for tool in tools}
    if args.lifecycle_only:
        assert {"storybook_get_host", "storybook_list_host_actions"} <= names
        assert not {"storybook_dispatch_host_action", "storybook_run_steps", "storybook_click_target"} & names
        host = call("storybook_get_host")
        assert host["active_route"] == host["native_route"] == "embedded-counter"
    else:
        assert {"storybook_get_host", "storybook_list_host_actions", "storybook_dispatch_host_action",
                "storybook_capture_host", "storybook_run_scenario", "storybook_click_target"} <= names
        stories = call("storybook_list_stories")["stories"]
        assert {story["key"] for story in stories} == {"embedded-counter", "embedded-notes"}
        host = call("storybook_get_host")
        assert host["active_route"] == host["native_route"]
        host_actions = call("storybook_list_host_actions")["actions"]
        assert any(action["name"] == "set_appearance" for action in host_actions)
        assert {"compose.increment", "compose.reset"} <= {action["name"] for action in host_actions}
        call("storybook_open_story", {"story_key": "embedded-counter"})
        before = state()["count"]
        native_before = compose_state()["count"]
        compose_action("compose.increment")
        assert compose_state()["count"] == native_before + 1
        native_value = call("storybook_read_value", {"value_key": "native.compose-counter"})
        assert native_value["semantic_value"]["value"]["count"] == native_before + 1
        call("storybook_wait_for_value", {"value_key": "native.compose-counter", "json_pointer": "/count",
             "expected": native_before + 1, "max_frames": 60})
        assert state()["count"] == before
        compose_action("compose.increment", {"unexpected": True}, error=True)
        compose_action("compose.increment", [], error=True)
        compose_action("compose.unknown", error=True)
        assert compose_state()["count"] == native_before + 1
        compose_action("compose.reset")
        assert compose_state()["count"] == 0
        compose_action("compose.increment")
        call("storybook_click_target", {"target_key": "increment"})
        assert state()["count"] == before + 1
        actions = call("storybook_list_actions")["actions"]
        assert any(action["name"] == "embedded_demo::Increment" for action in actions)
        result = call("storybook_run_steps", {"story_key": "embedded-counter", "steps": [
            {"type": "dispatch_action", "name": "embedded_demo::Increment"}]})
        assert result["steps_dispatched"] == 1
        assert state()["count"] == before + 2
        assert compose_state()["count"] == 1
        call("storybook_wait_for_value", {"value_key": "public-state", "json_pointer": "/count", "expected": before + 2, "max_frames": 60})
        call("storybook_capture_host", {"scope": "display", "output_path": str(args.output / "counter-display.png")})
        call("storybook_capture_current_story", {"output_path": str(args.output / "counter-gpui.png")})
        call("storybook_dispatch_host_action", {"action": {"action": "set_appearance", "id": "dark"}})
        assert call("storybook_get_host")["appearance"]["id"] == "dark"
        call("storybook_open_story", {"story_key": "embedded-notes"})
        host = call("storybook_get_host")
        assert host["native_route"] == host["active_route"] == "embedded-notes"
        for _ in range(2):
            call("storybook_run_scenario", {"story_key": "embedded-notes", "scenario_key": "write-note"})
            assert state()["note"] == "hello"
        call("storybook_capture_host", {"scope": "display", "output_path": str(args.output / "notes-display.png")})
        call("storybook_capture_host", {"scope": "gpui", "output_path": str(args.output / "notes-gpui.png")})
        call("storybook_capture_current_story", {"width": 390, "height": 844}, error=True)
        call("storybook_run_steps", {"steps": [{"type": "focus_next", "unexpected": 1}]}, error=True)
        call("storybook_open_story", {"story_key": "embedded-counter"})
        assert state()["count"] == before + 2
        for _ in range(2):
            call("storybook_run_scenario", {"story_key": "embedded-counter", "scenario_key": "increment-twice"})
            assert state()["count"] == 2
        assert compose_state()["count"] == 1
        call("storybook_capture_host", {"scope": "display", "output_path": str(args.output / "counter-dark-display.png")})
        decoded = {}
        for capture in captures:
            width, height, channels, rows = read_png(Path(capture["path"]))
            geometry = capture["host"]["geometry"]
            expected = (geometry["display_width"], geometry["display_height"]) if capture["scope"] == "display" else (geometry["width"], geometry["height"])
            assert (width, height) == expected == (capture["pixel_width"], capture["pixel_height"])
            # Text and control edges must contain rendered variation within the GPUI region.
            top = geometry["y"] if capture["scope"] == "display" else 0
            colors = {rows[y][x:x + 3] for y in range(top, min(top + 400, height), 3)
                      for x in range(0, width * channels, channels * 3)}
            assert len(colors) > 16, "capture has no rendered text/control variation"
            decoded[Path(capture["path"]).name] = (width, height, channels, rows)
        full, cropped = decoded["notes-display.png"], decoded["notes-gpui.png"]
        assert full[0] == cropped[0] and full[2] == cropped[2]
        crop = next(capture for capture in captures if Path(capture["path"]).name == "notes-gpui.png")
        geometry = crop["host"]["geometry"]
        assert cropped[1] < full[1] - geometry["y"], "GPUI includes the keyboard/system bars"
        different = sum(a != b for y, row in enumerate(cropped[3])
                        for a, b in zip(row, full[3][y + geometry["y"]]))
        assert different < cropped[0] * cropped[1] * cropped[2] * .02, "GPUI crop disagrees with display content"
        print("Decoded capture scopes, dimensions, content, and crop passed", flush=True)
        print("Android raw stdio proof passed", flush=True)

finally:
    transcript_path = args.output / ("eof-transcript.json" if args.lifecycle_only else "mcp-transcript.json")
    transcript_path.write_text(json.dumps(transcript, indent=2) + "\n")
    process.stdin.close()
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        process.terminate()
        process.wait(timeout=5)

if args.lifecycle_only:
    assert process.returncode == 0
    stopped = subprocess.run(["adb", "-s", args.serial, "shell", "pidof", "dev.storybook.mobile"], capture_output=True, timeout=15)
    assert stopped.returncode != 0
    assert subprocess.check_output(["adb", "-s", args.serial, "forward", "--list"], timeout=15) == forwarding_before
    transcript.append({"eof": "owned example stopped; forwarding unchanged; default interaction tools omitted"})
    transcript_path.write_text(json.dumps(transcript, indent=2) + "\n")
    print("Explicit launch, interaction gating, stop-on-EOF, and unchanged forwarding passed", flush=True)
