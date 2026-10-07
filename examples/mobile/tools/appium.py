#!/usr/bin/env python3
"""Qualify appium-client against an owned Android emulator and loopback Appium."""
import argparse
import contextlib
import http.client
import http.server
import json
from pathlib import Path
import select
import socket
import struct
import subprocess
import threading
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[3]
BINARY = ROOT / "examples/mobile/tools/target/debug/gpui-storybook-android-tools"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--server-port", type=int, default=5783)
    parser.add_argument("--output", type=Path, default=ROOT / "target/android-tools-evidence/appium")
    args = parser.parse_args()
    if not args.serial.startswith("emulator-"):
        parser.error("qualification requires an exclusively owned disposable emulator")
    args.output.mkdir(parents=True, exist_ok=True)

    def adb(*command):
        return subprocess.check_output(["adb", "-s", args.serial, *command], timeout=30)

    def forwards():
        return {line for line in adb("forward", "--list").decode().splitlines() if line.startswith(args.serial + " ")}

    baseline = forwards()
    wire_port = int(adb("forward", "tcp:0", "tcp:28437"))
    owned_sessions = set()
    workers = []
    report = {"serial": args.serial, "client": "appium-client 0.2.2", "server": "3.8.0", "driver": "uiautomator2 8.7.0"}

    def appium(method, path):
        connection = http.client.HTTPConnection("127.0.0.1", args.server_port, timeout=30)
        try:
            connection.request(method, path)
            response = connection.getresponse()
            return response.status, json.loads(response.read(64 * 1024 * 1024))
        finally:
            connection.close()

    class Relay:
        def __init__(self):
            self.session = None
            self.lose_click = False
            self.hold_delete = False
            self.release_delete = threading.Event()
            relay = self

            class Handler(http.server.BaseHTTPRequestHandler):
                def log_message(self, *_):
                    pass

                def handle_request(self):
                    length = int(self.headers.get("Content-Length", 0))
                    assert 0 <= length <= 1024 * 1024
                    body = self.rfile.read(length)
                    if self.command == "DELETE" and relay.hold_delete:
                        if not relay.release_delete.wait(30):
                            return
                    connection = http.client.HTTPConnection("127.0.0.1", args.server_port, timeout=90)
                    try:
                        connection.request(self.command, self.path, body=body, headers={"Content-Type": "application/json"})
                        response = connection.getresponse()
                        data = response.read(64 * 1024 * 1024 + 1)
                        assert len(data) <= 64 * 1024 * 1024
                        if self.path == "/session" and self.command == "POST" and response.status == 200:
                            relay.session = json.loads(data)["value"]["sessionId"]
                            owned_sessions.add(relay.session)
                        if relay.lose_click and self.path.endswith("/execute/sync") and b"clickGesture" in body:
                            relay.lose_click = False
                            self.close_connection = True
                            self.connection.shutdown(socket.SHUT_RDWR)
                            return
                        self.send_response(response.status)
                        self.send_header("Content-Type", "application/json")
                        self.send_header("Content-Length", str(len(data)))
                        self.end_headers()
                        with contextlib.suppress(BrokenPipeError, ConnectionResetError):
                            self.wfile.write(data)
                    finally:
                        connection.close()

                do_GET = do_POST = do_DELETE = handle_request

            self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            self.thread = threading.Thread(target=self.server.serve_forever)
            self.thread.start()

        def close(self):
            self.release_delete.set()
            self.server.shutdown()
            self.server.server_close()
            self.thread.join()

    class Worker:
        def __init__(self, relay, drop_only=False):
            self.log = open(args.output / ("drop-stderr.log" if drop_only else "client-stderr.log"), "w")
            flags = ["--appium-drop-only"] if drop_only else []
            self.process = subprocess.Popen([str(BINARY), "--candidate", "appium", "--serial", args.serial,
                                             "--rpc-url", f"http://127.0.0.1:{relay.server.server_port}/", *flags],
                                            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True)
            workers.append(self)
            assert self.read()["ok"]["ready"]

        def read(self):
            if not select.select([self.process.stdout], [], [], 90)[0]:
                raise TimeoutError("Appium worker deadline; mutation outcome may be unknown")
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError(f"Appium worker exited: {self.log.name}")
            return json.loads(line)

        def call(self, operation, **fields):
            self.process.stdin.write(json.dumps({"operation": operation, **fields}) + "\n")
            self.process.stdin.flush()
            return self.read()

        def ok(self, operation, **fields):
            result = self.call(operation, **fields)
            assert "ok" in result, result
            return result["ok"]

        def close(self):
            if not self.process.stdin.closed:
                self.process.stdin.close()
            status = self.process.wait(timeout=15)
            assert status == 0, status
            self.log.close()

    class Wire:
        session = ""
        id = time.time_ns()

        def call(self, operation, **fields):
            self.id += 1
            body = json.dumps({"protocol_version": 2, "session": self.session, "request_id": self.id,
                               "command": {"operation": operation, **fields}}).encode()
            def exact(stream, count):
                result = bytearray()
                while len(result) < count:
                    part = stream.recv(count - len(result))
                    if not part:
                        raise EOFError("Storybook reply lost")
                    result.extend(part)
                return result
            with socket.create_connection(("127.0.0.1", wire_port), timeout=15) as stream:
                stream.sendall(struct.pack(">I", len(body)) + body)
                size, = struct.unpack(">I", exact(stream, 4))
                assert 0 < size <= 1024 * 1024
                reply = json.loads(exact(stream, size))
            assert reply["request_id"] == self.id
            if "Err" in reply["outcome"]:
                raise RuntimeError(reply["outcome"]["Err"])
            return reply["outcome"]["Ok"]["value"]

        def ready(self, route=None):
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                self.session = ""
                try:
                    host = self.call("get_host")
                    if host["active_route"] == host["native_route"] and (route is None or host["active_route"] == route):
                        self.session = host["session"]
                        return host
                except RuntimeError:
                    pass
                time.sleep(.05)
            raise TimeoutError("Storybook readiness")

        def state(self):
            return next(value["value"] for value in self.call("read_values")["values"] if value["key"] == "public-state")

    relays = []
    try:
        wire = Wire()
        wire.ready()
        relay = Relay()
        relays.append(relay)
        client = Worker(relay)
        source = client.ok("appium_source")
        ET.fromstring(source)
        assert "SurfaceView" in source
        (args.output / "hierarchy.xml").write_text(source)
        client.ok("appium_select", prefix="Notes")
        notes = wire.ready("embedded-notes")
        client.ok("appium_select", prefix="Counter")
        host = wire.ready("embedded-counter")
        before = wire.state()["count"]
        target = next(target for target in wire.call("list_targets")["targets"] if target["key"] == "increment")
        bounds, geometry = target["bounds"], host["geometry"]
        x = round(geometry["x"] + geometry["scale"] * (bounds["x"] + bounds["width"] / 2))
        y = round(geometry["y"] + geometry["scale"] * (bounds["y"] + bounds["height"] / 2))
        client.ok("appium_click", x=x, y=y)
        wire.ready("embedded-counter")
        assert wire.state()["count"] == before + 1
        relay.lose_click = True
        lost = client.call("appium_click", x=x, y=y)
        wire.ready("embedded-counter")
        after = wire.state()["count"]
        assert "error" in lost, lost
        assert after == before + 2, (before, after)
        ticket = wire.call("prepare_capture")
        try:
            image = client.ok("screenshot", path=str(args.output / "display.png"))
        finally:
            finished = wire.call("finish_capture", ticket=ticket["ticket"])
        assert finished == ticket["host"]
        assert (image["width"], image["height"]) == (geometry["display_width"], geometry["display_height"])
        client.close()
        closed, _ = appium("GET", f"/session/{relay.session}/source")
        assert closed == 404, closed
        report.update({"native_selectors": True, "notes_route": notes["active_route"], "click_delta": 1,
                       "lost_reply_delta": 1, "lost_reply_outcome": lost, "png": image, "awaited_close_status": closed})
        drop_relay = Relay()
        relays.append(drop_relay)
        drop_client = Worker(drop_relay, drop_only=True)
        drop_relay.hold_delete = True
        drop_client.close()
        live, _ = appium("GET", f"/session/{drop_relay.session}/source")
        assert live == 200, live
        report["drop_returned_with_live_session"] = True
        drop_relay.release_delete.set()
        # Explicitly settle the known owned session, even if Drop's task was cancelled.
        status, _ = appium("DELETE", f"/session/{drop_relay.session}")
        assert status in (200, 404), status
        gone, _ = appium("GET", f"/session/{drop_relay.session}/source")
        assert gone == 404, gone
        report["drop_session_cleanup_status"] = gone
        report["passed"] = True
    finally:
        for relay in relays:
            relay.release_delete.set()
        for worker in workers:
            if worker.process.poll() is None:
                worker.process.kill()
                worker.process.wait(timeout=10)
            worker.log.close()
        for session in owned_sessions:
            with contextlib.suppress(OSError):
                appium("DELETE", f"/session/{session}")
        for relay in relays:
            relay.close()
        adb("forward", "--remove", f"tcp:{wire_port}")
        report["forward_inventory_preserved"] = forwards() == baseline
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    assert report["forward_inventory_preserved"], report
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
