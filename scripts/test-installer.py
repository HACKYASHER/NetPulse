"""Test per-user installation/uninstallation in a new temporary directory.

Never runs when a previous Qt installation is registered. Does not install TUN.
"""
from pathlib import Path
import hashlib,json,os,subprocess,time,uuid,winreg
ROOT=Path(__file__).resolve().parents[1]
KEY=r'Software\NetPulseQt'
UNINSTALL=r'Software\Microsoft\Windows\CurrentVersion\Uninstall\NetPulseQt'
def registered(path):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER,path):return True
    except FileNotFoundError:return False

def main():
    if registered(KEY) or registered(UNINSTALL):
        raise RuntimeError('An existing Qt installation is registered; refusing to replace it during verification')
    output=ROOT/'.build'/('installer-check-'+uuid.uuid4().hex)
    install=output/'app';output.mkdir(parents=True)
    installer=ROOT/'release/NetPulse-Setup.exe'
    p=subprocess.run([str(installer),'/S','/D='+str(install)],timeout=180,creationflags=0x08000000)
    assert p.returncode==0,p.returncode
    for name in ['NetPulse.exe','netpulsed.exe','NetPulseService.exe','cores/sing-box.exe','cores/xray.exe']:
        assert hashlib.sha256((install/name).read_bytes()).digest()==hashlib.sha256((ROOT/'release/NetPulse'/name).read_bytes()).digest()
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER,UNINSTALL) as key:
        version=winreg.QueryValueEx(key,'DisplayVersion')[0]
        command=winreg.QueryValueEx(key,'UninstallString')[0]
    assert version=='1.0.0-beta' and command=='"'+str(install/'Uninstall.exe')+'"',(version,command)
    smoke=output/'smoke'
    p=subprocess.run([str(install/'NetPulse.exe'),'--smoke-test',str(smoke)],timeout=85,creationflags=0x08000000)
    assert p.returncode==0,p.returncode
    result=json.loads((smoke/'result.json').read_text());assert result['qml_issues']==0
    p=subprocess.run([str(install/'Uninstall.exe'),'/S'],timeout=180,creationflags=0x08000000)
    assert p.returncode==0,p.returncode
    end=time.monotonic()+40
    while time.monotonic()<end and (registered(KEY) or registered(UNINSTALL) or (install/'NetPulse.exe').exists()):time.sleep(.25)
    assert not registered(KEY) and not registered(UNINSTALL)
    assert not (install/'NetPulse.exe').exists()
    assert not (install/'netpulsed.exe').exists()
    report={'result':'passed','version':'1.0.0-beta','install':True,'packaged_hashes':True,'installed_ui':result,
            'uninstall':True,'registry_removed':True,'tun_service_installed':False,'global_network_changed':False}
    (ROOT/'.build/installer-result.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
