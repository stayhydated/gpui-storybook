#!/usr/bin/env python3
"""Real-device probes; run only on an exclusively owned disposable emulator."""
import argparse
import base64
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

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--serial", required=True)
parser.add_argument("--apk", type=Path, required=True)
parser.add_argument("--output", type=Path, default=ROOT / "target/android-tools-evidence")
args = parser.parse_args()
if not args.serial.startswith("emulator-"):
    parser.error("these lifecycle probes require a disposable emulator")
args.output.mkdir(parents=True, exist_ok=True)
binary = HERE / "target/debug/gpui-storybook-android-tools"
report = {"serial": args.serial, "candidates": {}, "retry": {}}
owned_forwards = []


def adb(*command):
    return subprocess.check_output(["adb", "-s", args.serial, *command], timeout=30)


def forwards():
    return {tuple(line.split()) for line in adb("forward", "--list").decode().splitlines()
            if line.startswith(args.serial + " ")}


def forward(remote):
    port = int(adb("forward", "tcp:0", f"tcp:{remote}"))
    owned_forwards.append(f"tcp:{port}")
    return port


class Worker:
    def __init__(self, candidate, *flags):
        self.log = open(args.output / (candidate + "-stderr.log"), "a")
        self.process = subprocess.Popen(
            [str(binary), "--candidate", candidate, "--serial", args.serial, *flags],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True,
        )
        self.read()

    def read(self):
        if not select.select([self.process.stdout], [], [], 90)[0]:
            self.process.kill()
            raise TimeoutError("candidate process deadline (outcome may be unknown)")
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f"candidate exited: see {self.log.name}")
        return json.loads(line)

    def call(self, operation, **fields):
        self.process.stdin.write(json.dumps({"operation": operation, **fields}) + "\n")
        self.process.stdin.flush()
        return self.read()

    def ok(self, operation, **fields):
        result = self.call(operation, **fields)
        if "error" in result:
            raise RuntimeError(result["error"])
        return result["ok"]

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
        self.log.close()


class Wire:
    def __init__(self, port):
        self.port, self.session, self.id = port, "", time.time_ns()

    @staticmethod
    def exact(stream, length):
        output = bytearray()
        while len(output) < length:
            chunk = stream.recv(length - len(output))
            if not chunk:
                raise EOFError("device response closed")
            output.extend(chunk)
        return output

    def request(self, operation, **fields):
        self.id += 1
        return {"protocol_version": 1, "session": self.session, "request_id": self.id,
                "command": {"operation": operation, **fields}}

    def call(self, operation, **fields):
        request = self.request(operation, **fields)
        body = json.dumps(request).encode()
        with socket.create_connection(("127.0.0.1", self.port), timeout=15) as stream:
            stream.sendall(struct.pack(">I", len(body)) + body)
            size, = struct.unpack(">I", self.exact(stream, 4))
            assert 0 < size <= 1024 * 1024
            response = json.loads(self.exact(stream, size))
        assert response["request_id"] == request["request_id"]
        if "Err" in response["outcome"]:
            raise RuntimeError(response["outcome"]["Err"])
        return response["outcome"]["Ok"]["value"]

    def ready(self, predicate=lambda h: True):
        def read():
            self.session = ""
            host = self.call("get_host")
            if host["active_route"] == host["native_route"] and predicate(host):
                self.session = host["session"]
                return host
        return eventually(read)

    def state(self):
        return next(v["value"] for v in self.call("read_values")["values"]
                    if v["key"] == "public-state")

    def target(self, key):
        host = self.ready()
        target = next(t for t in self.call("list_targets")["targets"] if t["key"] == key)
        b, g = target["bounds"], host["geometry"]
        return [round(g["x"] + g["scale"] * (b["x"] + b["width"] / 2)),
                round(g["y"] + g["scale"] * (b["y"] + b["height"] / 2))]


def eventually(read, timeout=20):
    deadline, last = time.monotonic() + timeout, None
    while time.monotonic() < deadline:
        try:
            value = read()
            if value:
                return value
        except (RuntimeError, OSError, EOFError) as error:
            last = error
        time.sleep(.05)
    raise TimeoutError(f"observed state deadline: {last}")


def start_app():
    adb("shell", "am", "start", "-S", "-n", "dev.storybook.mobile/.MainActivity",
        "--ez", "storybook_automation", "true")


def stop_uia():
    result = subprocess.run(["adb", "-s", args.serial, "shell", "pkill", "-f", "com.wetest.uia2.Main"], timeout=30)
    assert result.returncode in (0, 1), result


class Relay:
    """A real HTTP relay that discards one response after native input completes.

    Its supervised upstream is the actual u2.jar service on the emulator.
    The oracle is the example's rendered public count, not a call expectation.
    """
    def __init__(self):
        self.upstream = forward(9010)
        self.backend = None
        self.log = open(args.output / "relay-backend.log", "a")
        self.lock, self.drop, self.events = threading.Lock(), False, []
        relay = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.handle_request()

            def do_POST(self):
                self.handle_request()

            def log_message(self, *_):
                pass

            def handle_request(self):
                size = int(self.headers.get("Content-Length", 0))
                assert 0 <= size <= 1024 * 1024
                body = self.rfile.read(size)
                with relay.lock:
                    relay.ensure_upstream()
                    connection = http.client.HTTPConnection("127.0.0.1", relay.upstream, timeout=15)
                    connection.request(self.command, self.path, body,
                                       {"Content-Type": "application/json"})
                    response = connection.getresponse()
                    payload = response.read(1024 * 1024 + 1)
                    assert len(payload) <= 1024 * 1024
                    connection.close()
                    method = json.loads(body).get("method") if body else None
                    discarded = method == "click" and relay.drop
                    if discarded:
                        relay.drop = False
                    relay.events.append({"method": method, "discarded_response": discarded})
                if discarded:
                    self.close_connection = True
                    return
                self.send_response(response.status)
                self.send_header("Content-Length", str(len(payload)))
                self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.write(payload)

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def ensure_upstream(self):
        def ping():
            try:
                conn = http.client.HTTPConnection("127.0.0.1", self.upstream, timeout=1)
                conn.request("GET", "/ping")
                value = conn.getresponse().read() == b"pong"
                conn.close()
                return value
            except (OSError, http.client.HTTPException):
                return False
        if ping():
            return
        if self.backend:
            self.backend.wait(timeout=10)
        self.backend = subprocess.Popen(
            ["adb", "-s", args.serial, "shell",
             "CLASSPATH=/data/local/tmp/u2.jar app_process / com.wetest.uia2.Main -p 9010"],
            stdout=self.log, stderr=self.log,
        )
        eventually(ping)

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        stop_uia()
        if self.backend:
            self.backend.wait(timeout=10)
        self.log.close()


def adb_probe(candidate):
    worker = Worker(candidate)
    value = report["candidates"].setdefault(candidate, {})
    try:
        value["install"] = worker.ok("install", path=str(args.apk.resolve()))
        result = worker.ok("shell", command="printf probe-out; printf probe-err >&2; exit 7")
        assert result["stdout"] == "probe-out" and result["stderr"] == "probe-err", result
        assert result["exit"] in (7, None), result
        value["shell_v2"] = result
        if result["exit"] is None:
            value["limitation"] = "shell API lost the observed exit status (expected 7)"
        worker.ok("shell", command="am start -S -n dev.storybook.mobile/.MainActivity --ez storybook_automation true")
        before = forwards()
        value["forward"] = worker.call("forward", local="tcp:0", remote="tcp:28437")
        added = forwards() - before
        value["allocated_forwards"] = sorted(added)
        for _, local, _ in added:
            owned_forwards.append(local)
        if added:
            assert len(added) == 1
            _, local, _ = added.pop()
            wire = Wire(int(local.split(":")[1]))
        else:
            wire = Wire(forward(28437))
        wire.ready()
        if candidate == "adbutils":
            response = worker.ok("wire", request=wire.request("list_targets"))
            assert response["outcome"]["Ok"]["value"]["targets"]
            value["direct_device_socket"] = "bounded Storybook list_targets decoded"
        before_count = wire.state()["count"]
        x, y = wire.target("increment")
        worker.ok("shell", command=f"input tap {x} {y}")
        eventually(lambda: wire.state()["count"] == before_count + 1)
        value["count_delta"] = wire.state()["count"] - before_count
        value["screenshot"] = worker.ok("screenshot", path=str(args.output / (candidate + ".png")))
        host = wire.ready()
        assert value["screenshot"] == {"width": host["geometry"]["display_width"],
                                        "height": host["geometry"]["display_height"]}
        if "local" in locals():
            value["remove_forward"] = worker.call("remove_forward", local=local)
            assert all(row[1] != local for row in forwards()), value
            owned_forwards.remove(local)
        value["passed"] = True
    finally:
        worker.close()


def uia_probe(candidate):
    stop_uia()
    start_app()
    wire = Wire(forward(28437))
    wire.ready()
    worker = Worker(candidate, *( ["--single-attempt"] if candidate == "uiautomator2" else []))
    value = report["candidates"].setdefault(candidate, {})
    try:
        rpc = lambda method, params: worker.ok("rpc", method=method, params=params)
        value["device_info"] = rpc("deviceInfo", [])
        xml = rpc("dumpWindowHierarchy", [False, 50])
        ET.fromstring(xml)
        (args.output / (candidate + ".xml")).write_text(xml)
        assert "android.view.SurfaceView" in xml
        assert "increment" not in xml
        for label, route in [("NOTES", "embedded-notes"), ("COUNTER", "embedded-counter")]:
            selector = {"mask": 8, "textStartsWith": label}
            assert rpc("exist", [selector]) is True
            assert rpc("click", [selector]) is True
            wire.ready(lambda host: host["active_route"] == route)
        before = wire.state()["count"]
        assert rpc("click", wire.target("increment")) is True
        eventually(lambda: wire.state()["count"] == before + 1)
        value["count_delta"] = wire.state()["count"] - before
        value["selector_routes"] = ["embedded-notes", "embedded-counter"]
        image = rpc("takeScreenshot", [1.0, 90])
        if image.startswith("data:image/"):
            prefix, image = image.split(",", 1)
            assert prefix in ("data:image/jpeg;base64", "data:image/png;base64"), prefix
        data = base64.b64decode("".join(image.split()), validate=True)
        assert data.startswith(b"\xff\xd8") or data.startswith(b"\x89PNG")
        (args.output / (candidate + "-capture.jpg")).write_bytes(data)
        value["capture_bytes"] = len(data)
        value["passed"] = True
    finally:
        worker.close()
        stop_uia()


def retry_probe():
    relay = Relay()
    reverse = "tcp:9009"
    adb("reverse", "--no-rebind", reverse, f"tcp:{relay.server.server_port}")
    try:
        for candidate in ("uiautomator", "uiautomator2"):
            for single in (False, True):
                start_app()
                wire = Wire(forward(28437))
                wire.ready()
                flags = (["--rpc-url", f"http://127.0.0.1:{relay.server.server_port}/jsonrpc/0",
                          "--attempts", "1" if single else "3"] if candidate == "uiautomator"
                         else ["--port", "9009"] + (["--single-attempt"] if single else []))
                worker = Worker(candidate, *flags)
                try:
                    before = wire.state()["count"]
                    relay.drop = True
                    result = worker.call("rpc", method="click", params=wire.target("increment"))
                    delta = eventually(lambda: wire.state()["count"] - before)
                    # Observe the final value after the completed client call.
                    delta = wire.state()["count"] - before
                    assert delta == (1 if single else 2), (candidate, single, delta, result)
                    assert ("error" in result) == single, result
                    report["retry"][f"{candidate}-{'single' if single else 'default'}"] = {
                        "count_delta": delta, "result": result,
                        "input_outcome": "unknown after dropped reply" if single else "input replayed",
                    }
                finally:
                    worker.close()
                    stop_uia()
    finally:
        adb("reverse", "--remove", reverse)
        relay.close()
        report["relay_events"] = relay.events


try:
    adb("shell", "cmd", "window", "user-rotation", "lock", "0")
    report["android"] = {"sdk": adb("shell", "getprop", "ro.build.version.sdk").decode().strip(),
                         "abi": adb("shell", "getprop", "ro.product.cpu.abi").decode().strip()}
    for candidate in ("adb-client", "adbutils", "uiautomator", "uiautomator2"):
        try:
            (adb_probe if candidate.startswith("adb") else uia_probe)(candidate)
            print(candidate, "passed", flush=True)
        except Exception as error:
            report["candidates"].setdefault(candidate, {})["failure"] = str(error)
            print(candidate, "failed:", error, flush=True)
    retry_probe()
finally:
    stop_uia()
    for local in owned_forwards:
        with contextlib.suppress(subprocess.CalledProcessError):
            adb("forward", "--remove", local)
    adb("shell", "cmd", "window", "user-rotation", "free")
    (args.output / "rust-report.json").write_text(json.dumps(report, indent=2) + "\n")
assert all(v.get("passed") for v in report["candidates"].values()), report["candidates"]
print("all Rust candidate probes and native dropped-reply oracles passed", flush=True)
