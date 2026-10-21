"""Stage verified offline runtime resources and original redistribution notices."""
from pathlib import Path
import hashlib,json,shutil,tarfile,zipfile
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'apps/desktop-v2/resources/cores'
DEST.mkdir(parents=True,exist_ok=True)
FILES=[('.local/engine-cache/sing-box/sing-box-1.14.3-windows-amd64/sing-box.exe','sing-box.exe','5c19bb3f8c12d4e85b3d47fe63123a7624b314b58dd7a8e32f51004f1c4d2faf'),('.local/engine-cache/sing-box/sing-box-1.14.3-windows-amd64/libcronet.dll','libcronet.dll','3217c6260fbca5f16072e0b79735742f40109a63bb0ff88fd6b96dd6b54a2928'),('.local/engine-cache/xray/xray.exe','xray.exe','a743b8c2ce49cb33d6c1cf764c780bf1a475d746ad7485e7c3fcfe6458511c92')]
for relative,name,digest in FILES:
    path=ROOT/relative
    if not path.is_file():path=DEST/name
    with path.open('rb') as file:assert hashlib.file_digest(file,'sha256').hexdigest()==digest,name
    if path.resolve()!=(DEST/name).resolve():shutil.copy2(path,DEST/name)
source=ROOT/'.local/engine-sources'
if not (source/'manifest.json').is_file() and (DEST/'source-manifest.json').is_file() and any((DEST/'licenses').glob('*')):
    print(json.dumps({'staged':str(DEST),'runtime_files':len(FILES),'original_notices':'preserved from source package'}))
    raise SystemExit(0)
manifest=json.loads((source/'manifest.json').read_text(encoding='utf8'))
notices=DEST/'licenses';notices.mkdir(exist_ok=True)
for item in manifest['roots']:
    with tarfile.open(source/item['file']) as archive:
        for member in archive.getmembers():
            if member.isfile() and member.name.rsplit('/',1)[-1].upper().startswith(('LICENSE','NOTICE','COPYING')):
                name=item['name']+'-'+member.name.replace('/','-')
                (notices/name).write_bytes(archive.extractfile(member).read())
for item in manifest['modules']:
    with zipfile.ZipFile(source/item['archive']) as archive:
        for name in archive.namelist():
            if name.rsplit('/',1)[-1].upper().startswith(('LICENSE','LICENCE','NOTICE','COPYING','UNLICENSE')) and not name.endswith('/'):
                (notices/(hashlib.sha256(name.encode()).hexdigest()[:16]+'-'+name.rsplit('/',1)[-1])).write_bytes(archive.read(name))
gpl=ROOT/'apps/desktop/licenses/GPL-3.0-only.txt'
if not gpl.is_file():gpl=ROOT/'LICENSE'
shutil.copy2(gpl,notices/'GPL-3.0-only.txt')
shutil.copy2(source/'manifest.json',DEST/'source-manifest.json')
(DEST/'README.txt').write_text('NetPulse bundled runtimes: sing-box 1.14.3; Xray 26.10.17.\nOriginal licenses are in licenses/. Corresponding application and engine/dependency sources are available at https://github.com/HACKYASHER/NetPulse (apps/desktop-v2 and third-party/sources).\nNo third-party engine branding or original license notices have been removed.\n',encoding='utf8')
print(json.dumps({'staged':str(DEST),'runtime_files':len(FILES),'source_modules':len(manifest['modules'])}))
