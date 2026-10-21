"""Run native tests including pinned-engine fixtures; never touches system routes."""
from pathlib import Path
import importlib.util
import os
import subprocess
import sys
import json
import re
root=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('native_v2',root/'scripts/native-v2.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
env=module.native_env()
env['NETPULSE_TEST_ENGINE']=str(root/'.local/engine-cache/sing-box/sing-box-1.14.3-windows-amd64/sing-box.exe')
env['NETPULSE_JOB_OWNER']=str(root/'apps/desktop-v2/target/debug/examples/job_owner.exe')
env['NETPULSE_ROUTING_CLIENT']=str(root/'apps/desktop-v2/target/debug/examples/routing_client.exe')
env['NETPULSE_TEST_XRAY']=str(root/'.local/engine-cache/xray/xray.exe')
# Export only the public development fixture into a temporary test directory; no OS trust changes.
from cryptography.hazmat.primitives.serialization import pkcs12, Encoding, PrivateFormat, NoEncryption
fixture=root/'apps/desktop-v2/crates/netpulse-core/tests/fixtures'
key,cert,_=pkcs12.load_key_and_certificates((fixture/'owned-localhost.pfx').read_bytes(),b'fixture-only')
grpc_fixture=root/'.local/owned-grpc-fixture';grpc_fixture.mkdir(exist_ok=True)
(grpc_fixture/'server.pem').write_bytes(cert.public_bytes(Encoding.PEM))
(grpc_fixture/'key.pem').write_bytes(key.private_bytes(Encoding.PEM,PrivateFormat.PKCS8,NoEncryption()))
env['NETPULSE_GRPC_CERT']=str(grpc_fixture/'server.pem');env['NETPULSE_GRPC_KEY']=str(grpc_fixture/'key.pem')
env['NETPULSE_TRANSFER_REPORT']=str(root/'.local/v2-grpc-transfer.json')

cargo=str(root/'.local/toolchain/cargo/bin/cargo.exe')
cwd=root/'apps/desktop-v2'
expected_fingerprint=module.core_fingerprint()
subprocess.run([cargo,'build','-p','netpulse-core','--examples'],cwd=cwd,env=env,check=True)
process=subprocess.Popen([cargo,'test','-p','netpulse-core','--','--include-ignored','--test-threads=1'],cwd=cwd,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,encoding='utf8',errors='replace')
summaries=[]
for line in process.stdout:
    print(line,end='',flush=True)
    found=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',line)
    if found:summaries.append(tuple(map(int,found.groups())))
status=process.wait()
if status==0 and summaries:
    assert module.core_fingerprint()==expected_fingerprint, "Native source changed while tests were running; rerun before packaging"
    counts={'passed':sum(s[0] for s in summaries),'failed':sum(s[1] for s in summaries),'ignored':sum(s[2] for s in summaries)}
    (root/'.local/v2-native-tests.json').write_text(json.dumps({'result':'passed','core_sha256':expected_fingerprint,'counts':counts},indent=2),encoding='utf8')
raise SystemExit(status)
