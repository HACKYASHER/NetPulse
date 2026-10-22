"""Run isolated packaged UI checks; no VPN or global proxy is enabled."""
from pathlib import Path
import importlib.util,json,os,shutil,subprocess,uuid
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('ipc',ROOT/'scripts/test-ipc.py')
ipc=importlib.util.module_from_spec(spec);spec.loader.exec_module(ipc)

def main():
    suite=ROOT/'.build'/('release-verification-'+uuid.uuid4().hex)
    suite.mkdir(parents=True)
    results=[]
    clean=os.environ.copy()
    clean['PATH']=os.pathsep.join([str(Path(clean['WINDIR'])/'System32'),clean['WINDIR']])
    for name in ['QTDIR','QT_PLUGIN_PATH','QML2_IMPORT_PATH','QML_IMPORT_PATH']:clean.pop(name,None)
    for language,theme in [('ru','dark'),('ru','light'),('en','light'),('en','dark')]:
        output=suite/(language+'-'+theme);directory=output/'data'
        output.mkdir(parents=True)
        fixture=Path(os.environ.get('NETPULSE_UI_FIXTURE',ROOT.parent/'ONE/apps/desktop-v2/target/debug/examples/ui_fixture.exe'))
        if fixture.is_file():subprocess.run([str(fixture),str(directory)],cwd=ROOT/'backend',check=True,capture_output=True)
        else:directory.mkdir(parents=True)
        host=ipc.Host(directory)
        try:
            preferences=host.call('bootstrap')['preferences']
            preferences.update(language=language,theme=theme)
            host.call('save_preferences',preferences=preferences)
        finally:host.close()
        run=subprocess.run([str(ROOT/'release/NetPulse/NetPulse.exe'),'--smoke-test',str(output)],
                           capture_output=True,env=clean,timeout=85,creationflags=0x08000000)
        assert run.returncode==0,(run.returncode,run.stderr.decode(errors='replace'))
        result=json.loads((output/'result.json').read_text())
        assert result['result']=='passed' and result['version']=='1.0.1-beta'
        assert result['pages']==8 and result['qml_issues']==0 and result['frames']==18
        assert all((output/f'page-{i}.png').is_file() for i in range(8))
        results.append(dict(language=language,theme=theme,**result))
        if language=='en' and theme=='dark':
            target=ROOT/'docs/images';target.mkdir(exist_ok=True,parents=True)
            shutil.copy2(output/'frame-overview.png',target/'overview.png')
        print(language+'/'+theme+': eight pages rendered, no QML issues',flush=True)
    report={'result':'passed','version':'1.0.1-beta','ui':results,'system_network_changed':False,'output_directory':str(suite)}
    (ROOT/'.build/ui-result.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print('Packaged UI verification passed.',flush=True)

if __name__=='__main__':main()
