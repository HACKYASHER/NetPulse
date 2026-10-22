"""Exercise the packaged controller with isolated data, without changing Windows networking."""
from pathlib import Path
import argparse, copy, json, os, queue, socket, subprocess, sys, threading, time, uuid, winreg

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
        assert boot['preferences']['applications']==[] and boot['preferences']['bypass_domains']==[]
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
        preferences=checked['preferences']
        assert preferences['ping_method']=='tcp_connect_rtt' and preferences['ping_proxy_mode']=='keepalive'
        preferences.update(dns={'mode':'udp','server':'1.1.1.1'},applications=[r'C:\Apps\Steam.EXE',r'D:\Other\steam.exe'],bypass_domains=['.ru','*.su'])
        host.call('save_preferences',preferences=preferences)
        checked=host.call('bootstrap')['preferences']
        assert checked['dns']=={'mode':'udp','server':'1.1.1.1'}
        assert checked['applications']==['steam.exe'] and checked['bypass_domains']==['ru','su']
        routing=json.loads((directory/'routing-apps.json').read_text(encoding='utf-8'))
        assert routing['applications']==['steam.exe'] and routing['bypass_domains']==['ru','su']
        host.call('save_application_catalog',applications=[{'name':'NetPulse','path':str(ROOT/'release/NetPulse/NetPulse.exe'),'source':'test'}])
        assert len(host.call('read_application_catalog'))==1
        checks.append('DNS persists, EXE rules migrate and deduplicate, routing has its own JSON file')

        # Exercise the same controller entry point as each settings form. The
        # isolated host persists autostart intent without editing Windows Run.
        run_key=r'Software\Microsoft\Windows\CurrentVersion\Run'
        def autorun():
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER,run_key) as key:
                try:return winreg.QueryValueEx(key,'NetPulseQt')
                except FileNotFoundError:return None
        before_autorun=autorun()
        baseline=host.call('bootstrap')['preferences']
        settings_cases=[]
        for field,values in {
            'theme':['dark','light','system'],'language':['ru','en'],
            'transport_engine':['auto','sing-box','xray'],
            'route_mode':['global','whitelist','blacklist'],
            'ping_method':['proxy_https_rtt','proxy_https_head_rtt','tcp_connect_rtt','icmp_rtt'],
            'ping_proxy_mode':['double','keepalive'],'mtu':[1280,1400,1420,1500,9000],
            'proxy_port':[1024,2080,65535],'network_preset':['balanced','compatibility','games','custom'],
            'logging_level':['DEBUG','INFO','WARNING','ERROR'],
            'fake_ip':[True,False],'autostart':[True,False],
            'close_to_tray':[False,True],'minimize_to_tray':[True,False],
            'auto_reconnect':[True,False],'auto_failover':[True,False]
        }.items():
            settings_cases.extend((field,{field:value}) for value in values)
        for dns in [{'mode':'system'},{'mode':'udp','server':'9.9.9.9'},{'mode':'tls','server':'one.one.one.one'},{'mode':'https','server':'cloudflare-dns.com','path':'/dns-query'}]:
            for route in ['direct','vpn']:
                if dns['mode']=='system' and route=='vpn':continue
                settings_cases.append(('dns',{'dns':dns,'dns_route':route}))
        for mode in ['inherit','auto','custom','disabled']:
            settings_cases.append(('tls_defaults',{'tls_defaults':{'sni_mode':mode,'server_name':'example.invalid','http_host':None,'fingerprint':None}}))
        settings_cases += [
            ('fragmentation',{'fragmentation':dict(baseline['fragmentation'],enabled=True)}),
            ('multiplex',{'multiplex':dict(baseline['multiplex'],enabled=True)}),
            ('routing_rules',{'routing_rules':[{'enabled':True,'kind':'domain','value':'.ru','action':action} for action in ['direct','proxy','reject']]})
        ]
        for field,patch in settings_cases:
            candidate=copy.deepcopy(baseline);candidate.update(patch)
            host.call('save_preferences',preferences=candidate)
            saved=host.call('bootstrap')['preferences']
            for key,value in patch.items():assert saved[key]==value,(key,saved[key],value)
            assert host.call('snapshot')['engine_pid'] is None
        for patch in [{'mtu':1279},{'mtu':9001},{'proxy_port':1023},{'dns':{'mode':'udp','server':'invalid'}},{'probe_url':'http://example.com'},{'applications':['not-executable']},{'bypass_domains':['https://example.com']}]:
            previous=host.call('bootstrap')['preferences'];candidate=dict(previous,**patch)
            assert 'error' in host.wait(host.send('save_preferences',preferences=candidate)),patch
            assert host.call('bootstrap')['preferences']==previous
        host.call('save_preferences',preferences=baseline)
        assert before_autorun==autorun()
        checks.append(f'{len(settings_cases)} settings form variants persist; invalid saves are atomic; isolated tests preserve Windows autorun')
        host.call('quick_ui',patch={'favorite_id':profile['id']})
        assert profile['id'] in host.call('bootstrap')['preferences']['favorites']
        host.call('quick_ui',patch={'favorite_id':profile['id']})
        assert profile['id'] not in host.call('bootstrap')['preferences']['favorites']
        checks.append('pin and unpin actions persist')

        # A real socket owned by this test process must appear in both PID and
        # EXE-group analyses, even with no active engine connection.
        processes=host.call('playground_processes')
        process=next(p for p in processes if p['pid']==os.getpid())
        # A Python venv redirector can run a different physical executable than
        # sys.executable; use the path actually returned by Windows inventory.
        process_path=process['path']
        with socket.socket() as listener:
            listener.bind(('127.0.0.1',0));listener.listen()
            with socket.create_connection(listener.getsockname()) as client:
                accepted,_=listener.accept()
                with accepted:
                    endpoint='127.0.0.1:'+str(listener.getsockname()[1])
                    for pid in [os.getpid(),None]:
                        analysis=host.call('connection_analysis',pid=pid,path=process_path)
                        assert any(c.get('destination')==endpoint for c in analysis['connections']),analysis
        processes=host.call('playground_processes')
        assert any(p['pid']==os.getpid() for p in processes)
        assert all('cpu_percent' not in p and 'memory_bytes' not in p for p in processes)
        checks.append('Playground exposes owned TCP connections by PID and EXE group, with network-focused process data')

        for engine in ['sing-box','xray','auto']:
            host.call('quick_network',transport_engine=engine)
            state=host.call('bootstrap')
            assert state['preferences']['transport_engine']==engine and not state['snapshot']['engine_pid']
        assert host.wait(host.send('quick_network',transport_engine='unknown'))['error']
        assert host.call('bootstrap')['preferences']['transport_engine']=='auto'
        current=host.call('bootstrap')['preferences']
        current['sniffing']={'enabled':True,'http':True,'tls':True,'quic':False,'timeout_ms':125}
        current['tun']=True;current['system_proxy']=True
        host.call('save_preferences',preferences=current)
        state=host.call('bootstrap');assert state['preferences']['tun'] and state['preferences']['system_proxy']
        assert state['preferences']['sniffing']==current['sniffing'] and not state['snapshot']['engine_pid']
        invalid=copy.deepcopy(current);invalid['sniffing']['timeout_ms']=0
        assert host.wait(host.send('save_preferences',preferences=invalid))['error']['code']=='InvalidSniffing'
        assert host.call('bootstrap')['preferences']['sniffing']==current['sniffing']
        host.call('quick_network',tun=False,system_proxy=False)
        checks.append('engine choices persist; invalid choice rolls back; independent modes and Sniffing validate without starting VPN')
        assert host.wait(host.send('start_mtu_tuning',target='https://www.gstatic.com/generate_204',method='https',minimum=1440,maximum=1472,samples=5))['error']['code']=='TunRequired'
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
        assert host.call('diagnostic_history',kind='latency')
        ping_summary=next(p for p in host.call('bootstrap')['profiles'] if p['id']==profile['id'])
        assert ping_summary['latency_method']=='tcp_connect_rtt' and ping_summary['latency_at']

        for category in ['main','core','tunnel','subscriptions','service']:
            assert isinstance(host.call('log_read',day=host.call('log_days')[0],category=category),str)
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
        retained=next(p for p in boot['profiles'] if p['id']==profile['id'])
        assert (retained['latency_ms'],retained['latency_method'],retained['latency_at'])==(ping_summary['latency_ms'],ping_summary['latency_method'],ping_summary['latency_at'])
        checks.append('last per-server ping survives controller restart')
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
