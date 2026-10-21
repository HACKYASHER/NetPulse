"""Prepare exact official runtime inputs for a source checkout; no EXE is run."""
from pathlib import Path
import hashlib, io, json, shutil, urllib.request, zipfile
ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT/'apps/desktop-v2/resources/cores'
DEST.mkdir(parents=True, exist_ok=True)
items = [
 ('SagerNet/sing-box','v1.14.3','sing-box-1.14.3-windows-amd64.zip',{
  'sing-box.exe':'5c19bb3f8c12d4e85b3d47fe63123a7624b314b58dd7a8e32f51004f1c4d2faf',
  'libcronet.dll':'3217c6260fbca5f16072e0b79735742f40109a63bb0ff88fd6b96dd6b54a2928'}),
 ('XTLS/Xray-core','v26.10.17','Xray-windows-64.zip',{
  'xray.exe':'a743b8c2ce49cb33d6c1cf764c780bf1a475d746ad7485e7c3fcfe6458511c92'})]
for repo, tag, name, components in items:
 if all((DEST/n).is_file() and hashlib.sha256((DEST/n).read_bytes()).hexdigest()==sha for n,sha in components.items()):continue
 request=urllib.request.Request(f'https://github.com/{repo}/releases/download/{tag}/{name}',headers={'User-Agent':'NetPulse-source-build'})
 with urllib.request.urlopen(request,timeout=120) as r:raw=r.read(160*1024*1024+1)
 if len(raw)>160*1024*1024:raise RuntimeError('Runtime archive exceeds size limit')
 with zipfile.ZipFile(io.BytesIO(raw)) as z:
  for component, expected in components.items():
   candidates=[n for n in z.namelist() if n.rsplit('/',1)[-1]==component]
   if len(candidates)!=1:raise RuntimeError('Ambiguous runtime component')
   data=z.read(candidates[0]);assert hashlib.sha256(data).hexdigest()==expected,component
   (DEST/component).write_bytes(data)
source=ROOT/'third-party/sources/engine'
if source.is_dir():shutil.copytree(source,ROOT/'.local/engine-sources',dirs_exist_ok=True)
licenses=ROOT/'third-party/licenses'
if licenses.is_dir():shutil.copytree(licenses,ROOT/'release-v2/licenses',dirs_exist_ok=True)
if (ROOT/'LICENSE').is_file():shutil.copy2(ROOT/'LICENSE',ROOT/'LICENSE.txt')
# Tests resolve exact pinned binaries through their established cache locations.
cache=ROOT/'.local/engine-cache'
for name in ['sing-box.exe','libcronet.dll']:
 target=cache/'sing-box/sing-box-1.14.3-windows-amd64'/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(DEST/name,target)
target=cache/'xray/xray.exe';target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(DEST/'xray.exe',target)
print(json.dumps({'runtime_inputs':'verified','executables_run':False,'source_inputs':source.is_dir()}))
