"""Verify installer update/repair/uninstall using a separate test namespace.
The generated installer script is used with faster test compression, scoped registry/shortcut
names, isolated lock checks and disabled service hooks. No live service changes.
"""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,time,uuid,winreg
ROOT=Path(__file__).resolve().parents[1]
def exists(key):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER,key):return True
    except FileNotFoundError:return False

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--prepared-test',type=Path)
    options=parser.parse_args()
    token=options.prepared_test.name if options.prepared_test else 'NetPulseInstallTest'+uuid.uuid4().hex
    assert token.startswith('NetPulseInstallTest') and len(token)==51
    output=(options.prepared_test or ROOT/'.build'/token).resolve()
    assert output.resolve().is_relative_to((ROOT/'.build').resolve())
    output.mkdir(exist_ok=bool(options.prepared_test));install=output/'app';data=output/'data';data.mkdir(exist_ok=bool(options.prepared_test))
    source=(ROOT/'.build/installer.nsi').read_text(encoding='utf-8-sig')
    source=source.replace('NetPulseQt',token).replace('NetPulse Qt',token).replace('$DESKTOP\\NetPulse.lnk','$DESKTOP\\'+token+'.lnk').replace('$SMPROGRAMS\\NetPulse\\','$SMPROGRAMS\\'+token+'\\').replace('$SMPROGRAMS\\NetPulse"','$SMPROGRAMS\\'+token+'"')
    source=source.replace('OutFile "'+str(ROOT/'release/NetPulse-Setup.exe')+'"','OutFile "'+str(output/'Setup.exe')+'"')
    source=source.replace('--installer-check', '--installer-check --data-dir "'+str(data)+'"')
    source=source.replace('  nsExec::ExecToStack \'"$INSTDIR\\netpulsed.exe" --installer-service-remove\'','  Push ""\n  Push 0')
    # The install hook remains unreachable with /NOSERVICE; also stub it as a
    # second barrier so a test cannot launch Windows elevation accidentally.
    source=source.replace('  nsExec::ExecToStack \'"$INSTDIR\\netpulsed.exe" --installer-service-install\'','  Push ""\n  Push 0')
    assert '--installer-service-' not in source
    source=source.replace('SetCompressor /SOLID lzma\nSetCompressorDictSize 32','SetCompressor zlib')
    script=output/'installer.nsi'
    if options.prepared_test:
        assert script.read_text(encoding='utf-8-sig')==source and (output/'Setup.exe').is_file()
    else:
        script.write_text(source,encoding='utf-8-sig')
        subprocess.run([os.environ.get('NSIS_EXE',str(ROOT.parent/'ONE/.local/tools/nsis-3.13/makensis.exe')),'/V2',str(script)],check=True,timeout=600)
    key='Software\\'+token;uninstall='Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\'+token
    assert not exists(key) and not exists(uninstall)
    command=[str(output/'Setup.exe'),'/S','/NOSERVICE','/D='+str(install)]
    subprocess.run(command,check=True,timeout=180,creationflags=0x08000000)
    for name in ['NetPulse.exe','netpulsed.exe','NetPulseService.exe','cores/sing-box.exe','cores/xray.exe']:
        assert hashlib.sha256((install/name).read_bytes()).digest()==hashlib.sha256((ROOT/'release/NetPulse'/name).read_bytes()).digest()
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER,uninstall) as handle:
        assert winreg.QueryValueEx(handle,'DisplayName')[0]=='NetPulse'
        assert winreg.QueryValueEx(handle,'ModifyPath')[0]=='"'+str(install/'NetPulse-Maintenance.exe')+'"'
    sentinel=data/'settings-sentinel.txt';sentinel.write_text('keep',encoding='ascii')
    (install/'netpulsed.exe').write_bytes(b'damaged test controller')
    subprocess.run(command,check=True,timeout=180,creationflags=0x08000000)
    assert sentinel.read_text()=='keep'
    assert hashlib.sha256((install/'netpulsed.exe').read_bytes()).digest()==hashlib.sha256((ROOT/'release/NetPulse/netpulsed.exe').read_bytes()).digest()
    subprocess.run([str(install/'NetPulse.exe'),'--smoke-test',str(output/'smoke')],check=True,timeout=85,creationflags=0x08000000)
    result=json.loads((output/'smoke/result.json').read_text());assert result['qml_issues']==0
    subprocess.run([str(install/'Uninstall.exe'),'/S'],check=True,timeout=180,creationflags=0x08000000)
    until=time.monotonic()+45
    while time.monotonic()<until and (exists(key) or (install/'NetPulse.exe').exists()):time.sleep(.25)
    assert not exists(key) and not exists(uninstall) and not (install/'NetPulse.exe').exists()
    assert sentinel.read_text()=='keep'
    report={'result':'passed','install':True,'update_repair':True,'uninstall':True,'data_preserved':True,'registry_removed':True,'installed_ui':result,'tun_service_installed':False,'global_network_changed':False,'verification_namespace':token}
    (ROOT/'.build/installer-result.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
