"""Build the C++/QML client and standalone Rust controller and deploy Qt DLLs."""
from pathlib import Path
import argparse, hashlib, json, os, re, shutil, subprocess, sys

ROOT=Path(__file__).resolve().parents[1]
def environment():
    env=os.environ.copy()
    tools=Path(env.get('NETPULSE_TOOLCHAIN',ROOT.parent/'ONE/.local/toolchain'))
    if (tools/'rustup/settings.toml').is_file():
        env.update(RUSTUP_HOME=str(tools/'rustup'),CARGO_HOME=str(tools/'cargo'))
    msvc=tools/'msvc/VC/Tools/MSVC/14.44.35207'
    sdk=tools/'msvc/Windows Kits/10'
    additions=[Path(sys.executable).parent,tools/'cargo/bin']
    if (msvc/'include').is_dir():
        additions += [msvc/'bin/Hostx64/x64',sdk/'bin/10.0.26100.0/x64']
        env['INCLUDE']=os.pathsep.join(map(str,[msvc/'include']+[sdk/'Include/10.0.26100.0'/p for p in ['ucrt','shared','um','winrt','cppwinrt']]))
        env['LIB']=os.pathsep.join(map(str,[msvc/'lib/x64',sdk/'Lib/10.0.26100.0/ucrt/x64',sdk/'Lib/10.0.26100.0/um/x64']))
    env['PATH']=os.pathsep.join(map(str,additions))+os.pathsep+env['PATH']
    return env,msvc

def run(args,env,cwd=ROOT):
    print('Running '+str(args[0]),flush=True)
    args=list(args)
    args[0]=shutil.which(str(args[0]),path=env['PATH']) or str(args[0])
    subprocess.run([str(a) for a in args],env=env,cwd=cwd,check=True)

def validate_bundled_cores():
    for source,constant,name in [('engine.rs','ENGINE_SHA256','sing-box.exe'),('engine.rs','CRONET_SHA256','libcronet.dll'),('runtime.rs','XRAY_SHA256','xray.exe')]:
        code=(ROOT/'backend/crates/netpulse-core/src'/source).read_text(encoding='utf-8')
        expected=re.search(r'pub const '+constant+r':\s*&str\s*=\s*"([0-9a-f]{64})"',code)
        path=ROOT/'resources/cores'/name
        if expected is None or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected.group(1):
            raise RuntimeError('Bundled component is missing or differs from the pinned build: '+str(path))

def main():
    validate_bundled_cores()
    parser=argparse.ArgumentParser()
    parser.add_argument('--skip-backend',action='store_true')
    parser.add_argument('--cargo-target',type=Path,default=ROOT/'.build/rust')
    parser.add_argument('--qt',type=Path,default=Path(os.environ.get('QTDIR',ROOT/'.tools/Qt')))
    args=parser.parse_args();env,msvc=environment()
    qt=args.qt.resolve();env['PATH']=str(qt/'bin')+os.pathsep+env['PATH']
    target=args.cargo_target.resolve();env['CARGO_TARGET_DIR']=str(target)
    out=ROOT/'release/NetPulse';out.mkdir(parents=True,exist_ok=True)
    if not args.skip_backend:run(['cargo','build','--release','-p','netpulse-host','-p','netpulse-service'],env,ROOT/'backend')
    for exe in ['netpulsed.exe','NetPulseService.exe']:
        shutil.copy2(target/'release'/exe,out/exe)
    run(['cmake','-S',ROOT,'-B',ROOT/'.build/qt','-G','Ninja','-DCMAKE_BUILD_TYPE=Release','-DCMAKE_CXX_FLAGS_DEBUG=/Z7 /Od /Ob0 /RTC1',f'-DCMAKE_PREFIX_PATH={qt}'],env)
    run(['cmake','--build',ROOT/'.build/qt','--parallel','4'],env)
    shutil.copy2(ROOT/'.build/qt/NetPulse.exe',out/'NetPulse.exe')
    run([qt/'bin/windeployqt.exe','--release','--verbose','0','--no-translations','--no-compiler-runtime','--qmldir',ROOT/'qml',out/'NetPulse.exe'],env)
    # Portable distribution also includes the Microsoft C++ runtime, licensed
    # for redistribution with applications built using these tools.
    runtime=msvc.parents[2]/'Redist/MSVC'
    if runtime.is_dir():
        versions=sorted(runtime.iterdir())
        for dll in (versions[-1]/'x64/Microsoft.VC143.CRT').glob('*.dll'):shutil.copy2(dll,out/dll.name)
    else:
        for name in ['msvcp140.dll','vcruntime140.dll','vcruntime140_1.dll','msvcp140_1.dll','msvcp140_2.dll','concrt140.dll']:
            source=Path(os.environ['WINDIR'])/'System32'/name
            if source.is_file():shutil.copy2(source,out/name)
    shutil.copytree(ROOT/'resources/cores',out/'cores',dirs_exist_ok=True)
    shutil.copytree(ROOT/'licenses',out/'licenses',dirs_exist_ok=True)
    shutil.copy2(ROOT/'LICENSE.txt',out/'LICENSE.txt')
    for name in ['README.md','ARCHITECTURE.md']:
        if (ROOT/name).is_file():shutil.copy2(ROOT/name,out/name)
    print('Portable application: '+str(out/'NetPulse.exe'),flush=True)

if __name__=='__main__':main()
