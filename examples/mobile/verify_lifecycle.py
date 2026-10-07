#!/usr/bin/env python3
"""Check owned startup-failure cleanup and preservation of a restarted process."""
import argparse
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--serial', required=True)
parser.add_argument('--output', type=Path, default=ROOT / 'target/mobile-evidence')
args = parser.parse_args()
if not args.serial.startswith('emulator-'):
    parser.error('requires an exclusively owned disposable emulator')
args.output.mkdir(parents=True, exist_ok=True)
package = 'dev.storybook.mobile'
host = ROOT / 'target/debug/gpui-storybook-mobile-host'

def adb(*command):
    return subprocess.run(['adb', '-s', args.serial, *command], check=False, capture_output=True, timeout=15)

def pid():
    result = adb('shell', 'pidof', package)
    return result.stdout.decode().strip() if result.returncode == 0 else None

failed = subprocess.run([host, 'serve', '--serial', args.serial, '--config', str(ROOT / 'examples/mobile/storybook.toml'), '--launch', '--device-port', '1'],
                        stdin=subprocess.DEVNULL, capture_output=True, timeout=50)
(args.output / 'startup-failure-stderr.txt').write_bytes(failed.stderr)
assert failed.returncode != 0
assert pid() is None, 'failed attachment left its launched example alive'

process = subprocess.Popen([host, 'serve', '--serial', args.serial, '--config', str(ROOT / 'examples/mobile/storybook.toml'), '--launch', '--stop-on-eof'],
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
replacement = None
try:
    process.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
        'params': {'protocolVersion': '2025-03-26', 'capabilities': {},
                   'clientInfo': {'name': 'owned-lifecycle', 'version': '1'}}}) + '\n')
    process.stdin.flush()
    # The host negotiates before emitting initialize, so this is its ready PID.
    response = json.loads(process.stdout.readline())
    assert response.get('id') == 1 and 'result' in response, response
    process.stdin.write(json.dumps({'jsonrpc': '2.0', 'method': 'notifications/initialized'}) + '\n')
    process.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'}) + '\n')
    process.stdin.flush()
    tools = json.loads(process.stdout.readline())
    assert tools.get('id') == 2 and 'result' in tools, tools
    owned = pid()
    assert owned is not None
    started = adb('shell', 'am', 'start', '-S', '-n', package + '/.MainActivity',
                  '--ez', 'storybook_automation', 'true')
    assert started.returncode == 0, started.stderr
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        replacement = pid()
        if replacement and replacement != owned:
            break
        time.sleep(.05)
    assert replacement and replacement != owned
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode == 0, process.stderr.read()
    assert pid() == replacement, 'EOF stopped a process started after host ownership'
    (args.output / 'owned-lifecycle.json').write_text(json.dumps({
        'startup_failure_cleaned': True, 'owned_pid': owned,
        'replacement_pid': replacement, 'replacement_preserved': True}, indent=2) + '\n')
    print('Startup failure cleanup and restarted-process preservation passed', flush=True)
finally:
    if process.poll() is None:
        process.terminate()
        process.wait(timeout=5)
    if replacement and pid() == replacement:
        adb('shell', 'am', 'force-stop', package)
