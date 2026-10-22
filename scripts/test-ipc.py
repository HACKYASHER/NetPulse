"""Exercise the packaged controller with isolated data, without changing Windows networking."""
from pathlib import Path
import argparse, json, queue, subprocess, threading, time, uuid

ROOT=Path(__file__).resolve().parents[1]
class Host:
    def __init__(self,directory):
        self.process=subprocess.Popen([str(ROOT/'release/NetPulse/netpulsed.exe'),'--stdio',str(directory),'--isolated'],
            stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=0x08000000)
        self.messages=queue.Queue();self.serial=0;self.events=[];self.saved={}
        def reader():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
        threading.Thread(target=reader,daemon=True).start()
    def send(self,_operation,**params):
        self.serial+=1
        self.process.stdin.write(json.dumps({'id':self.serial,'method':_operation,'params':params},ensure_ascii=False).encode()+b'\n')
        self.process.stdin.flush();return self.serial
    def wait(self,id,timeout=35):
        end=time.monotonic()+timeout
        while id not in self.saved:
            response=self.messages.get(timeout=max(.01,end-time.monotonic()))
            if 'event' in response:self.events.append(response)
            else:self.saved[response.get('id')]=response
        return self.saved.pop(id)
    def call(self,_operation,**params):
        response=self.wait(self.send(_operation,**params))
        if 'error' in response:raise AssertionError((_operation,response['error']))
        return response.get('result')
    def close(self):
        if self.process.poll() is None:
            self.call('quit_application')
            self.process.stdin.close()
            assert self.process.wait(20)==0

def run(directory):
    directory.mkdir(parents=True,exist_ok=True);host=Host(directory);checks=[]
    try:
        end=time.monotonic()+30
        while True:
            boot=host.call('bootstrap')
            if not boot['runtime_preparing']:break
            assert time.monotonic()<end
            time.sleep(.2)
        assert boot['engine_ready'] and boot['xray_ready']
        assert not boot['snapshot']['engine_pid']
        checks.append('offline engines verified; no connection on startup')
        unknown=host.wait(host.send('arbitrary_shell',command='never executed'))
        assert unknown['error']['code']=='UnknownCommand'
        host.process.stdin.write(b'{invalid JSON}\n');host.process.stdin.flush()
        assert host.call('snapshot')['state']=='disconnected'
        checks.append('unknown and malformed requests rejected; channel remains usable')
        secret='85d01a47-8957-4f0a-afb2-c7896d30792b'
        report=host.call('import_text',text=f'vless://{secret}@127.0.0.1:443?security=tls&sni=fixture.invalid#Local%20QA')
        assert report['imported']==1
        boot=host.call('bootstrap');profile=next(p for p in boot['profiles'] if p['name']=='Local QA')
        assert secret not in json.dumps(boot)
        host.call('select_profile',id=profile['id'])
        preferences=boot['preferences'];preferences['theme']='light';preferences['language']='en'
        host.call('save_preferences',preferences=preferences)
        bad=dict(preferences,proxy_port=0)
        rejected=host.wait(host.send('save_preferences',preferences=bad))
        assert rejected['error']['code']=='InvalidSettings'
        checked=host.call('bootstrap')
        assert checked['preferences']['theme']=='light' and checked['preferences']['selected_profile']==profile['id']
        assert secret.encode() not in (directory/'netpulse.db').read_bytes()
        checks.append('profile import, selection, encrypted persistence, settings validation')
        assert host.wait(host.send('connect',id=profile['id']))['error']['code']=='ConnectionModeRequired'
        checks.append('connection cannot start with both modes disabled')
        ids=[host.send('snapshot') for _ in range(10)]
        assert all(host.wait(id)['result']['state']=='disconnected' for id in ids)
        checks.append('10 concurrent responses correctly correlated')
        job=host.call('start_latency',ids=[profile['id']],method='tcp_connect_rtt')
        end=time.monotonic()+30
        while True:
            status=host.call('diagnostic_status',id=job)
            if not status['running']:break
            assert time.monotonic()<end
            time.sleep(.1)
        assert len(status['results'])==1
        checks.append('real diagnostic job completes over owned loopback target')
        days=host.call('log_days');assert days
        assert isinstance(host.call('log_read',day=days[0],category='main'),str)
        rejected=host.wait(host.send('log_read',day='../outside',category='main'))
        assert 'error' in rejected
        checks.append('audit reads work and path traversal is rejected')
        host.close()
        host=Host(directory)
        boot=host.call('bootstrap');assert boot['preferences']['theme']=='light'
        assert boot['preferences']['selected_profile']==profile['id']
        assert any(p['id']==profile['id'] for p in boot['profiles'])
        checks.append('settings and profile persist after controller restart')
        return {'result':'passed','checks':checks,'system_network_changed':False,'data_directory':str(directory)}
    finally:
        host.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--directory',type=Path)
    args=parser.parse_args();directory=args.directory or ROOT/'.build'/('ipc-'+uuid.uuid4().hex)
    result=run(directory.resolve())
    destination=ROOT/'.build/ipc-result.json';destination.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(result,ensure_ascii=True,indent=2))
