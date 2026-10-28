"""Fetch official Xray/companion DLL and build the marked NetPulse sing-box."""
from pathlib import Path
import hashlib,io,json,os,subprocess,sys,urllib.request,zipfile
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'resources/cores';DEST.mkdir(parents=True,exist_ok=True)
COMPONENTS=[
    ('SagerNet/sing-box','v1.14.4','sing-box-1.14.4-windows-amd64.zip',{
        'libcronet.dll':'3217c6260fbca5f16072e0b79735742f40109a63bb0ff88fd6b96dd6b54a2928'}),
    ('XTLS/Xray-core','v26.10.21','Xray-windows-64.zip',{
        'xray.exe':'c77a00eb84fb93ae044c2d771a1cabcc033b81e487253fbc35b12df6879c1e15'})]
for repo,tag,archive,files in COMPONENTS:
    if all((DEST/name).is_file() and hashlib.sha256((DEST/name).read_bytes()).hexdigest()==checksum for name,checksum in files.items()):continue
    request=urllib.request.Request(f'https://github.com/{repo}/releases/download/{tag}/{archive}',headers={'User-Agent':'NetPulse-source-build'})
    with urllib.request.urlopen(request,timeout=120) as response:data=response.read(160*1024*1024+1)
    if len(data)>160*1024*1024:raise RuntimeError('Runtime archive exceeds the limit')
    with zipfile.ZipFile(io.BytesIO(data)) as source:
        for name,checksum in files.items():
            candidates=[n for n in source.namelist() if n.rsplit('/',1)[-1]==name]
            if len(candidates)!=1:raise RuntimeError('Ambiguous runtime component')
            payload=source.read(candidates[0])
            if hashlib.sha256(payload).hexdigest()!=checksum:raise RuntimeError('Runtime checksum mismatch')
            (DEST/name).write_bytes(payload)
manifest=json.loads((DEST/'netpulse-build/manifest.json').read_text())
if not (DEST/'sing-box.exe').is_file() or hashlib.sha256((DEST/'sing-box.exe').read_bytes()).hexdigest()!=manifest['sha256']:
    command=[sys.executable,ROOT/'scripts/build-sing-box.py']
    if os.environ.get('NETPULSE_GO_EXE'):command+=['--go',os.environ['NETPULSE_GO_EXE']]
    subprocess.run(command,check=True)
print('Pinned runtime components verified; sing-box carries the NetPulse build suffix.')
