"""Repeat real process-routing tests on loopback without changing Windows routes."""
from pathlib import Path
import importlib.util
import json
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / 'apps/desktop-v2'
spec = importlib.util.spec_from_file_location('native_v2', ROOT / 'scripts/native-v2.py')
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
env = native.native_env()
cargo = shutil.which('cargo', path=env['PATH'])
env.setdefault('NETPULSE_TEST_ENGINE', str(ROOT / '.local/engine-cache/sing-box/sing-box-1.14.3-windows-amd64/sing-box.exe'))
env['NETPULSE_ROUTING_CLIENT'] = str(APP / 'target/debug/examples/routing_client.exe')
fingerprint = native.core_fingerprint()
subprocess.run([cargo, 'build', '-p', 'netpulse-core', '--example', 'routing_client'], cwd=APP, env=env, check=True)
build = subprocess.run([cargo, 'test', '-p', 'netpulse-core', '--lib', '--no-run', '--message-format=json'], cwd=APP, env=env, check=True, capture_output=True, text=True, encoding='utf8')
artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith('{')]
test_exe = next(item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact' and item.get('profile', {}).get('test') and item.get('executable'))
runs = 30
for index in range(runs):
    result = subprocess.run([test_exe, 'app_rules::tests::engine_routes', '--ignored', '--nocapture'], cwd=APP, env=env, capture_output=True, text=True, encoding='utf8')
    print(f'Routing stress {index + 1}/{runs}: {"PASS" if result.returncode == 0 else "FAIL"}', flush=True)
    if result.returncode:
        print(result.stdout + result.stderr)
        raise SystemExit(result.returncode)
assert native.core_fingerprint() == fingerprint, 'Source changed during the routing stress test'
receipt = ROOT / '.local/whitelist-stress.json'
receipt.parent.mkdir(exist_ok=True)
receipt.write_text(json.dumps({'core_sha256': fingerprint, 'runs': runs, 'requests': runs * 24, 'result': 'passed'}, indent=2), encoding='utf8')
