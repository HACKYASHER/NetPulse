"""Exercise the real default-profile GUI startup in an owned runtime copy.
No installation registry writes, production profile access, TUN or proxy changes.
"""
from pathlib import Path
import importlib.util,json,os,shutil,socket,subprocess,time,uuid

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('ipc',ROOT/'scripts/test-ipc.py')
ipc=importlib.util.module_from_spec(spec);spec.loader.exec_module(ipc)

def main():
    output=ROOT/'.build'/('startup-'+uuid.uuid4().hex);output.mkdir()
    app=output/'NetPulse проверка';shutil.copytree(ROOT/'release/NetPulse',app)
    local=output/'Local';local.mkdir()
    env=os.environ.copy();env['LOCALAPPDATA']=str(local);env['QT_QPA_PLATFORM']='windows'
    for name in ['QTDIR','QT_PLUGIN_PATH','QML2_IMPORT_PATH','QML_IMPORT_PATH']:env.pop(name,None)
    def run(exe,*args,cwd=app):
        return subprocess.run([str(app/exe),*map(str,args)],cwd=cwd,env=env,capture_output=True,timeout=35,creationflags=0x08000000)
    resolved=run('netpulsed.exe','--resolve-data','--isolated');assert resolved.returncode==0,resolved.stderr
    profile=Path(json.loads(resolved.stdout)['directory']);assert profile.is_relative_to(app/'data')
    host=ipc.Host(profile)
    try:
        until=time.monotonic()+40
        while host.call('bootstrap')['runtime_preparing']:
            assert time.monotonic()<until;time.sleep(.1)
        host.call('import_text',text='socks5://fixture-user:fixture-secret@127.0.0.1:1080#Startup')
    finally:host.close()
    lookup=run('netpulsed.exe','--profile-paths');assert lookup.returncode==0
    sources=json.loads(lookup.stdout)['sources']
    assert all(Path(p).is_absolute() and Path(p).resolve()!=profile.resolve() for p in sources)
    for cwd in [app,output,app]:
        result=run('NetPulse.exe','--startup-check',cwd=cwd)
        assert result.returncode==0,(result.returncode,result.stderr.decode('utf-8',errors='replace'))
    lock=profile/'client.lock';assert not lock.exists()
    lock.write_bytes(('4294967294\nNetPulse\n'+socket.gethostname()+'\n').encode())
    stale=run('NetPulse.exe','--startup-check');assert stale.returncode==0 and not lock.exists()
    blocked=output/'blocked-profile';blocked.write_bytes(b'owned fixture')
    invalid=run('NetPulse.exe','--startup-check','--data-dir',blocked)
    assert invalid.returncode==12 and 'уже работает' not in invalid.stderr.decode('utf-8',errors='replace')
    lock.mkdir()
    try:
        denied=run('NetPulse.exe','--startup-check')
        assert denied.returncode==12 and 'уже работает' not in denied.stderr.decode('utf-8',errors='replace'),(denied.returncode,denied.stderr.decode('utf-8',errors='replace'))
    finally:lock.rmdir()
    before={p:p.stat().st_size for p in (profile/'logs').rglob('*') if p.is_file()}
    primary=subprocess.Popen([str(app/'NetPulse.exe'),'--background','--isolated'],cwd=app,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=0x08000000)
    try:
        until=time.monotonic()+35
        while True:
            assert primary.poll() is None,'Primary client exited during startup'
            active=lock.exists() and lock.read_text(encoding='utf-8').splitlines()[0]==str(primary.pid)
            controller_started=any(p.stat().st_size>before.get(p,0) for p in (profile/'logs').rglob('*') if p.is_file())
            if active and controller_started:break
            assert time.monotonic()<until,'Default-profile startup did not reach the controller';time.sleep(.1)
        second=run('NetPulse.exe','--startup-check')
        assert second.returncode==1 and primary.poll() is None
        assert lock.read_text(encoding='utf-8').splitlines()[0]==str(primary.pid)
    finally:
        # Own isolated client only: an abrupt exit is part of stale-lock coverage.
        if primary.poll() is None:primary.terminate()
        primary.wait(10)
    after=run('NetPulse.exe','--startup-check');assert after.returncode==0 and not lock.exists()
    host=ipc.Host(profile)
    try:
        boot=host.call('bootstrap');assert len(boot['profiles'])==1 and boot['profiles'][0]['name']=='Startup' and not boot['snapshot']['engine_pid']
    finally:host.close()
    report={'result':'passed','version':'1.0.6','checks':['default profile with existing database starts from multiple working directories','migration list contains only absolute noncurrent profiles','stale lock from a missing process is recovered','directory creation error is not reported as another instance','lock file access error is not reported as another instance','real GUI reaches its isolated controller using default profile resolution','second instance preserves the active primary lock','restart after abrupt isolated client exit recovers its lock','existing profile is preserved'],'production_profile_changed':False,'system_network_changed':False,'registry_modified':False}
    (ROOT/'.build/startup-result.json').write_bytes((json.dumps(report,indent=2)+'\n').encode())
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
