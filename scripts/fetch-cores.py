"""Download the exact official network engines pinned by the NetPulse core."""
from pathlib import Path
import hashlib,io,urllib.request,zipfile
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'resources/cores';DEST.mkdir(parents=True,exist_ok=True)
COMPONENTS=[
    ('SagerNet/sing-box','v1.14.3','sing-box-1.14.3-windows-amd64.zip',{
        'sing-box.exe':'5c19bb3f8c12d4e85b3d47fe63123a7624b314b58dd7a8e32f51004f1c4d2faf',
        'libcronet.dll':'3217c6260fbca5f16072e0b79735742f40109a63bb0ff88fd6b96dd6b54a2928'}),
    ('XTLS/Xray-core','v26.10.17','Xray-windows-64.zip',{
        'xray.exe':'a743b8c2ce49cb33d6c1cf764c780bf1a475d746ad7485e7c3fcfe6458511c92'})]
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
print('Official runtime components verified.')
