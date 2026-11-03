"""Native protocol and QML regressions using mocks, without a controller or OS network changes."""
from pathlib import Path
import argparse, json, os, shutil, subprocess
import build
ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--qt',type=Path,default=ROOT/'.tools/Qt')
    parser.add_argument('--output',type=Path,default=ROOT/'.build/ui-regressions')
    args=parser.parse_args();qt=args.qt.resolve();output=args.output.resolve();output.mkdir(parents=True,exist_ok=True)
    env,_=build.environment()
    for name in list(env):
        if name.startswith(('QT_','QML')):env.pop(name)
    env.update(PATH=str(qt/'bin')+os.pathsep+env['PATH'],QT_QPA_PLATFORM='windows',QT_QPA_PLATFORM_PLUGIN_PATH=str(qt/'plugins/platforms'),QML2_IMPORT_PATH=str(qt/'qml'),QT_QUICK_CONTROLS_STYLE='Basic')
    cmake=shutil.which('cmake',path=env['PATH'])
    if not cmake:raise RuntimeError('CMake is required')
    directory=ROOT/'.build/native-tests'
    subprocess.run([cmake,'-S',str(ROOT),'-B',str(directory),'-G','Ninja','-DCMAKE_BUILD_TYPE=Release',f'-DCMAKE_PREFIX_PATH={qt}','-DNETPULSE_BUILD_TESTS=ON'],env=env,check=True)
    subprocess.run([cmake,'--build',str(directory),'--target','NetPulseProtocolTests','--parallel','4'],env=env,check=True)
    checks=[('native',directory/'NetPulseProtocolTests.exe',[]),('qml',qt/'bin/qmltestrunner.exe',['-input',str(ROOT/'tests/qml')])]
    for name,exe,arguments in checks:
        run=subprocess.run([str(exe),*arguments,'-o',str(output/(name+'.results.txt'))+',txt'],env=env,capture_output=True,timeout=90,creationflags=0x08000000)
        (output/(name+'.stdout.txt')).write_bytes(run.stdout)
        (output/(name+'.stderr.txt')).write_bytes(run.stderr)
        if run.returncode:raise RuntimeError(f'{name} regressions failed ({run.returncode}); see {output}')
    result={'result':'passed','native_protocol':True,'qml_activity_requests':True,'production_controller_started':False,'system_network_changed':False}
    (output/'result.json').write_bytes((json.dumps(result,indent=2)+'\n').encode('utf-8'));print(json.dumps(result))
if __name__=='__main__':main()
