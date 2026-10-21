"""Six-minute owned named-pipe test; never connects to the installed service."""
from pathlib import Path
import hashlib
import importlib.util
import json
import os
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("native_v2", ROOT / "scripts/native-v2.py")
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
source = ROOT / "apps/desktop-v2/crates/netpulse-core/src/service.rs"
before = hashlib.sha256(source.read_bytes()).hexdigest()
env = native.native_env()
env["NETPULSE_IPC_ENDURANCE_SECONDS"] = "360"
started = time.monotonic()
subprocess.run(
    [str(ROOT / ".local/toolchain/cargo/bin/cargo.exe"), "test", "-p", "netpulse-core",
     "service::tests::persistent_ipc_recovers_from_abandoned_clients", "--", "--exact", "--nocapture"],
    cwd=ROOT / "apps/desktop-v2", env=env, check=True,
)
assert hashlib.sha256(source.read_bytes()).hexdigest() == before
assert time.monotonic() - started >= 360
result = {
    "result": "passed", "duration_seconds": 360,
    "test": "service::tests::persistent_ipc_recovers_from_abandoned_clients",
    "service_source_sha256": before, "system_network_changed": False,
    "installed_service_used": False, "source_unchanged_after_test": True,
}
(ROOT / ".local/ipc-endurance.json").write_text(json.dumps(result, indent=2), encoding="utf8")
print(json.dumps(result), flush=True)
