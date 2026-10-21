"""Run Cargo with a workspace-local Rust/MSVC SDK, without changing global PATH."""
from pathlib import Path
import os
import subprocess
import sys
import shutil
import hashlib

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / '.local/toolchain'

def core_fingerprint():
    core = ROOT/'apps/desktop-v2/crates/netpulse-core'
    files = sorted(p for p in core.rglob('*') if p.is_file()) + [ROOT/'apps/desktop-v2/Cargo.toml',ROOT/'apps/desktop-v2/Cargo.lock']
    digest = hashlib.sha256()
    for path in files:
        digest.update(path.relative_to(ROOT).as_posix().encode())
        digest.update(b'\0')
        digest.update(path.read_bytes())
    return digest.hexdigest()

def native_env():
    env = os.environ.copy()
    if (TOOLS/'rustup/settings.toml').exists():
        env.update(RUSTUP_HOME=str(TOOLS/'rustup'), CARGO_HOME=str(TOOLS/'cargo'))
    msvc = TOOLS/'msvc/VC/Tools/MSVC/14.44.35207'
    sdk = TOOLS/'msvc/Windows Kits/10'
    node = Path.home()/'.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin'
    extra = [TOOLS/'cargo/bin', msvc/'bin/Hostx64/x64', sdk/'bin/10.0.26100.0/x64', node]
    env['PATH'] = os.pathsep.join(map(str, extra)) + os.pathsep + env['PATH']
    if (msvc/'include').is_dir():
        env['INCLUDE'] = os.pathsep.join(map(str, [msvc/'include'] + [sdk/'Include/10.0.26100.0'/p for p in ('ucrt','shared','um','winrt','cppwinrt')]))
        env['LIB'] = os.pathsep.join(map(str, [msvc/'lib/x64',sdk/'Lib/10.0.26100.0/ucrt/x64',sdk/'Lib/10.0.26100.0/um/x64']))
    return env

if __name__ == '__main__':
    env = native_env()
    args = sys.argv[1:] or ['cargo','test','-p','netpulse-core']
    args[0] = shutil.which(args[0], path=env['PATH']) or args[0]
    raise SystemExit(subprocess.call(args, cwd=ROOT/'apps/desktop-v2', env=env))
