"""Exercise packaged profile relocation and engine repair with synthetic data.
No production profile, Windows service, proxy or routes are changed.
"""
from pathlib import Path
import hashlib, importlib.util, json, os, shutil, subprocess, time, uuid
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('ipc',ROOT/'scripts/test-ipc.py')
ipc=importlib.util.module_from_spec(spec);spec.loader.exec_module(ipc)

def main():
    output=ROOT/'.build'/('layout-test-'+uuid.uuid4().hex);output.mkdir()
    local=output/'Local';legacy=local/'NetPulseQt';app=output/'NetPulse';app.mkdir()
    old_program=local/'Programs/NetPulse';old_program.mkdir(parents=True)
    for name in ['NetPulse.exe','netpulsed.exe','keep.txt']:(old_program/name).write_bytes(b'old fixture')
    shutil.copy2(ROOT/'release/NetPulse/netpulsed.exe',app/'netpulsed.exe')
    (app/'installation-files.txt').write_text('NetPulse.exe\nnetpulsed.exe\n',encoding='utf-8')
    host=ipc.Host(legacy)
    try:
        until=time.monotonic()+35
        while host.call('bootstrap')['runtime_preparing']:
            assert time.monotonic()<until;time.sleep(.2)
        host.call('import_text',text='socks5://layout-user:layout-secret@127.0.0.1:1080#Migrated')
        prefs=host.call('bootstrap')['preferences'];prefs.update(language='en',theme='light')
        host.call('save_preferences',preferences=prefs)
    finally:host.close()
    env=os.environ.copy();env['LOCALAPPDATA']=str(local)
    command=[str(app/'netpulsed.exe'),'--resolve-data','--isolated']
    def resolve():
        result=subprocess.run(command,env=env,capture_output=True,timeout=120,creationflags=0x08000000)
        assert result.returncode==0,result.stdout.decode(errors='replace')
        return Path(json.loads(result.stdout)['directory'])
    profile=resolve();assert profile.is_relative_to(app/'data')
    assert not (legacy/'netpulse.db').exists() and (legacy/'migrated-to.txt').exists()
    assert (old_program/'keep.txt').exists() and not (old_program/'NetPulse.exe').exists()
    host=ipc.Host(profile)
    try:
        boot=host.call('bootstrap');assert len(boot['profiles'])==1 and boot['profiles'][0]['name']=='Migrated'
        assert boot['preferences']['language']=='en' and boot['preferences']['theme']=='light'
        assert not boot['snapshot']['engine_pid']
        # Missing installed engine remains absent until explicit repair.
        until=time.monotonic()+35
        while host.call('bootstrap')['runtime_preparing']:
            assert time.monotonic()<until;time.sleep(.2)
    finally:host.close()
    missing=profile/'engine/sing-box.exe';missing.unlink();host=ipc.Host(profile)
    try:
        until=time.monotonic()+35
        while True:
            boot=host.call('bootstrap')
            if not boot['runtime_preparing']:break
            assert time.monotonic()<until;time.sleep(.2)
        assert not missing.exists(), 'startup silently restored a missing engine'
        assert boot['runtime_error']['code']=='RuntimeComponentUnavailable',boot['runtime_error']
        assert str(missing).replace('\\','/') in boot['runtime_error']['message'].replace('\\','/'),boot['runtime_error']
        assert host.call('activate_core_bundle',rollback=False) is True
        assert host.call('bootstrap')['engine_ready']
        assert hashlib.sha256(missing.read_bytes()).digest()==hashlib.sha256((ROOT/'resources/cores/sing-box.exe').read_bytes()).digest()
        assert len(host.call('bootstrap')['profiles'])==1
    finally:host.close()
    assert resolve()==profile
    report={'result':'passed','packaged_profile_migration':True,'encrypted_profile_preserved':True,'preferences_preserved':True,'legacy_known_files_cleaned':True,'unknown_files_preserved':True,'missing_engine_not_silently_recreated':True,'explicit_offline_repair':True,'all_data_under_application':True,'system_network_changed':False}
    (ROOT/'.build/layout-result.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
